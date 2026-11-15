//! Autonomous native PTY staging endpoint. No shipping session selects it.

use std::fmt;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};

use oneterm_terminal::backend::{AsyncGhosttyPump, AsyncGhosttyPumpError};
use oneterm_terminal::ghostty::colors::DefaultColors;
use oneterm_terminal::ghostty::input::{KeyEvent, MouseEvent};
use oneterm_terminal::ghostty::selection::Update as SelectionUpdate;
use oneterm_terminal::ghostty::state::Scroll;
use oneterm_terminal::ghostty_model::SharedModel;
use oneterm_terminal::{
    ClipboardOrigin, OscRouter, PtyTransport, SessionEvent, SessionEventSink, SharedSessionState,
    SharedState, TerminalError, TerminalSecurityPolicy,
};
use oneterm_vt::pty::EventedReadWrite;

use super::*;

const CONTROL_QUEUE_CAPACITY: usize = 64;
const PASTE_BYTE_BUDGET: usize = 4 * 1024 * 1024;

struct PasteReservation {
    budget: Arc<ByteBudget<PASTE_BYTE_BUDGET>>,
    bytes: usize,
}

impl Drop for PasteReservation {
    fn drop(&mut self) {
        self.budget.release(self.bytes);
    }
}

struct Paste {
    reveal: bool,
    text: String,
    result: async_channel::Sender<Result<(), AsyncGhosttyPumpError>>,
    _reservation: PasteReservation,
}

struct ControlWait<'a> {
    endpoint: &'a NativeLocal,
    complete: bool,
}

impl Drop for ControlWait<'_> {
    fn drop(&mut self) {
        if !self.complete
            && let Err(error) = self.endpoint.close()
        {
            log::warn!("Native local cancelled control close: {error}");
        }
    }
}

#[derive(Debug)]
pub enum InputError {
    Invalid(oneterm_terminal::ghostty::Error),
    Admission(TerminalError),
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => error.fmt(f),
            Self::Admission(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for InputError {}
impl From<oneterm_terminal::ghostty::Error> for InputError {
    fn from(error: oneterm_terminal::ghostty::Error) -> Self {
        Self::Invalid(error)
    }
}

enum Input {
    Raw(
        Vec<u8>,
        async_channel::Sender<Result<(), AsyncGhosttyPumpError>>,
        PasteReservation,
    ),
    KeyReceipt(
        KeyEvent,
        async_channel::Sender<Result<Vec<u8>, AsyncGhosttyPumpError>>,
    ),
    ClearCommand(async_channel::Sender<Result<(), AsyncGhosttyPumpError>>),
    Search(
        oneterm_terminal::ghostty::search::Request,
        async_channel::Sender<
            Result<oneterm_terminal::ghostty::search::Snapshot, AsyncGhosttyPumpError>,
        >,
    ),
    Wheel(oneterm_terminal::ghostty::wheel::WheelEvent),
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(WindowSize),
    Paste(Paste),
    Scroll(Scroll),
    DefaultColors(
        Box<DefaultColors>,
        async_channel::Sender<Result<(), AsyncGhosttyPumpError>>,
    ),
    Selection(
        SelectionUpdate,
        async_channel::Sender<Result<(), AsyncGhosttyPumpError>>,
    ),
    ScrollNow(
        Scroll,
        async_channel::Sender<Result<(), AsyncGhosttyPumpError>>,
    ),
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Native(AsyncGhosttyPumpError),
    OwnerLost,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Native local PTY: {error}"),
            Self::Native(error) => error.fmt(f),
            Self::OwnerLost => f.write_str("Native local PTY owner ended without a result"),
        }
    }
}

impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<AsyncGhosttyPumpError> for Error {
    fn from(error: AsyncGhosttyPumpError) -> Self {
        Self::Native(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    Requested,
    ChildExit,
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleDelivery {
    Complete,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    pub stop: Stop,
    pub exit_code: Option<i32>,
    pub lifecycle: LifecycleDelivery,
}

struct Admission {
    gate: Mutex<()>,
    requested: AtomicBool,
    sealed: AtomicBool,
}

/// Explicit native endpoint with raw writes and bounded physical controls.
pub struct NativeLocal {
    logging: oneterm_terminal::TerminalLogController,
    transport: LocalTransport,
    admission: Arc<Admission>,
    inputs: async_channel::Sender<Input>,
    poll: Arc<Poller>,
    model: SharedModel,
    state: SharedState,
    result: async_channel::Receiver<Result<Report, Error>>,
    completed: bool,
    control: Arc<ShellControl>,
    paste_budget: Arc<ByteBudget<PASTE_BYTE_BUDGET>>,
}

impl NativeLocal {
    /// Starts asynchronous initialization on a dedicated PTY owner thread.
    /// Startup failures are reported by wait; no live PTY is dropped here.
    pub fn spawn(
        opts: Options,
        size: WindowSize,
        events: async_channel::Sender<SessionEvent>,
    ) -> Result<Self, Error> {
        Self::spawn_with_security(opts, size, events, TerminalSecurityPolicy::default())
    }

    pub fn spawn_with_security(
        opts: Options,
        size: WindowSize,
        events: async_channel::Sender<SessionEvent>,
        security: TerminalSecurityPolicy,
    ) -> Result<Self, Error> {
        Self::spawn_with_scrollback(opts, size, events, security, None)
    }

    pub fn spawn_with_scrollback(
        opts: Options,
        size: WindowSize,
        events: async_channel::Sender<SessionEvent>,
        security: TerminalSecurityPolicy,
        scrollback: Option<usize>,
    ) -> Result<Self, Error> {
        Self::spawn_with_logging(
            opts,
            size,
            events,
            security,
            scrollback,
            (Default::default(), "terminal".into()),
        )
    }

    pub(crate) fn spawn_with_logging(
        opts: Options,
        size: WindowSize,
        events: async_channel::Sender<SessionEvent>,
        security: TerminalSecurityPolicy,
        scrollback: Option<usize>,
        logging_setup: (oneterm_core::TerminalLogConfig, std::path::PathBuf),
    ) -> Result<Self, Error> {
        let poll = Arc::new(Poller::new()?);
        let control = Arc::new(ShellControl::default());
        let (sender, input) = mpsc::sync_channel(LOCAL_COMMAND_QUEUE_CAPACITY);
        let notifier = ShellNotifier {
            sender,
            poller: poll.clone(),
            control: control.clone(),
        };
        let transport = LocalTransport::new();
        transport.set_notifier(notifier.clone());
        let state = SharedSessionState::new_alive();
        let events = SessionEventSink::new(events);
        let router = OscRouter::with_security(
            transport.clone(),
            events.clone(),
            state.clone(),
            ClipboardOrigin::Local,
            security,
        );
        let pump =
            AsyncGhosttyPump::spawn_with_scrollback(router, size.cols, size.rows, scrollback)?;
        let logging = pump.logging();
        let owner_logging = logging.clone();
        let model = pump.model();
        let admission = Arc::new(Admission {
            gate: Mutex::new(()),
            requested: AtomicBool::new(false),
            sealed: AtomicBool::new(false),
        });
        let (done, result) = async_channel::bounded(1);
        let (inputs, controls) = async_channel::bounded(CONTROL_QUEUE_CAPACITY);
        let endpoint_poll = poll.clone();
        let mut owner = OwnerIo {
            pty: None,
            registered: false,
            input,
            pending: None,
            offset: 0,
            poll,
            control: control.clone(),
            admission: admission.clone(),
            exit: None,
            cancelled: false,
            controls,
        };
        let owner_state = state.clone();
        std::thread::Builder::new()
            .name("Native PTY owner".into())
            .spawn(move || {
                let result = run(
                    &mut owner,
                    pump,
                    opts,
                    size,
                    owner_state,
                    events,
                    owner_logging,
                    logging_setup,
                );
                // The endpoint may have been dropped/cancelled; teardown already ran.
                if let Err(error) = done.send_blocking(result) {
                    log::debug!("Native local result receiver closed: {error}");
                }
            })?;
        Ok(Self {
            logging,
            transport,
            admission,
            inputs,
            poll: endpoint_poll,
            model,
            state,
            result,
            completed: false,
            control,
            paste_budget: Arc::default(),
        })
    }

    pub fn write(&self, bytes: &[u8]) -> Result<(), TerminalError> {
        let _gate = self
            .admission
            .gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.admission.sealed.load(Ordering::Acquire) {
            return Err(TerminalError::Closed);
        }
        self.transport.pty_write(bytes)
    }

    pub(crate) fn logging(&self) -> oneterm_terminal::TerminalLogController {
        self.logging.clone()
    }

    /// Seal new input without cancelling active native publication. The owner
    /// drains admitted controls before taking its ordinary teardown path.
    /// Completion and the caller's deadline remain separate obligations.
    pub fn request_shutdown(&self) -> Result<(), TerminalError> {
        let _gate = self
            .admission
            .gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.admission.sealed.store(true, Ordering::Release);
        self.poll
            .notify()
            .map_err(|error| TerminalError::Transport(error.to_string()))
    }

    pub fn close(&self) -> Result<(), TerminalError> {
        let _gate = self
            .admission
            .gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.admission.requested.store(true, Ordering::Release);
        self.transport.pty_close()
    }

    /// Queue controls, not layout-derived guesses or physical delivery proof.
    pub fn key(&self, key: KeyEvent) -> Result<(), InputError> {
        key.validate()?;
        self.admit(Input::Key(key))
    }

    /// Acknowledges exact native encoding and original FIFO admission once.
    pub async fn key_receipt(&self, key: KeyEvent) -> Result<Vec<u8>, AsyncGhosttyPumpError> {
        key.validate().map_err(|error| {
            AsyncGhosttyPumpError::Engine(
                oneterm_terminal::ghostty::owner::Error::Native(error).into(),
            )
        })?;
        let (tx, rx) = async_channel::bounded(1);
        self.enqueue(Input::KeyReceipt(key, tx))
            .map_err(|error| match error {
                InputError::Admission(error) => AsyncGhosttyPumpError::Transport(error),
                InputError::Invalid(_) => unreachable!("enqueue only checks admission"),
            })?;
        let mut guard = ControlWait {
            endpoint: self,
            complete: false,
        };
        self.poll.notify().map_err(|error| {
            AsyncGhosttyPumpError::Transport(TerminalError::Transport(error.to_string()))
        })?;
        let result = rx.recv().await.map_err(|_| AsyncGhosttyPumpError::Closed)?;
        guard.complete = true;
        result
    }

    pub fn mouse(&self, mouse: MouseEvent) -> Result<(), InputError> {
        mouse.validate()?;
        self.admit(Input::Mouse(mouse))
    }

    pub fn wheel(
        &self,
        wheel: oneterm_terminal::ghostty::wheel::WheelEvent,
    ) -> Result<(), InputError> {
        wheel.validate()?;
        self.admit(Input::Wheel(wheel))
    }

    /// Native then physical resize, followed by capture, at a batch boundary.
    /// Successful return means admission only, not child acknowledgment.
    pub fn resize(&self, size: WindowSize) -> Result<(), InputError> {
        oneterm_terminal::ghostty::render::check_budget(size.cols, size.rows)?;
        self.admit(Input::Resize(size))
    }

    /// Queue a native viewport change and publication without requiring more
    /// child output. Success means admission only; negative delta moves older.
    pub fn scroll(&self, scroll: Scroll) -> Result<(), InputError> {
        self.admit(Input::Scroll(scroll))
    }

    pub async fn scroll_viewport(&self, scroll: Scroll) -> Result<(), AsyncGhosttyPumpError> {
        let (tx, rx) = async_channel::bounded(1);
        self.enqueue(Input::ScrollNow(scroll, tx))
            .map_err(|error| match error {
                InputError::Admission(error) => AsyncGhosttyPumpError::Transport(error),
                InputError::Invalid(_) => {
                    unreachable!("scroll admission does not validate native state")
                }
            })?;
        let mut guard = ControlWait {
            endpoint: self,
            complete: false,
        };
        self.poll.notify().map_err(|error| {
            AsyncGhosttyPumpError::Transport(TerminalError::Transport(error.to_string()))
        })?;
        let result = rx.recv().await.map_err(|_| AsyncGhosttyPumpError::Closed)?;
        guard.complete = true;
        result
    }

    pub async fn search_request(
        &self,
        request: oneterm_terminal::ghostty::search::Request,
    ) -> Result<oneterm_terminal::ghostty::search::Snapshot, AsyncGhosttyPumpError> {
        request.validate().map_err(|error| {
            AsyncGhosttyPumpError::Engine(
                oneterm_terminal::ghostty::owner::Error::Search(error).into(),
            )
        })?;
        let (tx, rx) = async_channel::bounded(1);
        self.enqueue(Input::Search(request, tx))
            .map_err(|error| match error {
                InputError::Admission(error) => AsyncGhosttyPumpError::Transport(error),
                InputError::Invalid(_) => unreachable!("search enqueue only checks admission"),
            })?;
        let mut guard = ControlWait {
            endpoint: self,
            complete: false,
        };
        self.poll.notify().map_err(|error| {
            AsyncGhosttyPumpError::Transport(TerminalError::Transport(error.to_string()))
        })?;
        let result = rx.recv().await.map_err(|_| AsyncGhosttyPumpError::Closed)?;
        guard.complete = true;
        result
    }

    fn admit(&self, input: Input) -> Result<(), InputError> {
        self.enqueue(input)?;
        self.poll
            .notify()
            .map_err(|error| InputError::Admission(TerminalError::Transport(error.to_string())))
    }

    fn enqueue(&self, input: Input) -> Result<(), InputError> {
        let _gate = self
            .admission
            .gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.admission.sealed.load(Ordering::Acquire)
            || self.control.shutdown.load(Ordering::Acquire)
            || self.inputs.is_closed()
        {
            return Err(InputError::Admission(TerminalError::Closed));
        }
        self.inputs.try_send(input).map_err(|error| {
            InputError::Admission(match error {
                async_channel::TrySendError::Full(_) => TerminalError::QueueFull,
                async_channel::TrySendError::Closed(_) => TerminalError::Closed,
            })
        })?;
        Ok(())
    }

    /// Executes in control order using live native modes. Success means original
    /// FIFO admission only. Dropping an admitted future independently closes.
    pub async fn paste(&self, text: &str) -> Result<(), AsyncGhosttyPumpError> {
        self.paste_request(text, false).await
    }

    pub async fn paste_visible(&self, text: &str) -> Result<(), AsyncGhosttyPumpError> {
        self.paste_request(text, true).await
    }

    async fn paste_request(&self, text: &str, reveal: bool) -> Result<(), AsyncGhosttyPumpError> {
        AsyncGhosttyPump::<LocalTransport>::validate_paste(text)
            .map_err(AsyncGhosttyPumpError::Paste)?;
        if !self.paste_budget.reserve(text.len()) {
            return Err(AsyncGhosttyPumpError::Transport(TerminalError::QueueFull));
        }
        let reservation = PasteReservation {
            budget: self.paste_budget.clone(),
            bytes: text.len(),
        };
        let (tx, rx) = async_channel::bounded(1);
        self.enqueue(Input::Paste(Paste {
            reveal,
            text: text.to_owned(),
            result: tx,
            _reservation: reservation,
        }))
        .map_err(|error| match error {
            InputError::Admission(error) => AsyncGhosttyPumpError::Transport(error),
            InputError::Invalid(_) => unreachable!("enqueue only checks admission"),
        })?;
        let mut guard = ControlWait {
            endpoint: self,
            complete: false,
        };
        self.poll.notify().map_err(|error| {
            AsyncGhosttyPumpError::Transport(TerminalError::Transport(error.to_string()))
        })?;
        let result = rx.recv().await.map_err(|_| AsyncGhosttyPumpError::Closed)?;
        guard.complete = true;
        result
    }

    pub fn model(&self) -> SharedModel {
        self.model.clone()
    }

    pub async fn write_visible(&self, bytes: &[u8]) -> Result<(), AsyncGhosttyPumpError> {
        oneterm_terminal::ghostty_session::validate_input_size(bytes.len())
            .map_err(AsyncGhosttyPumpError::Paste)?;
        if !self.paste_budget.reserve(bytes.len()) {
            return Err(AsyncGhosttyPumpError::Transport(TerminalError::QueueFull));
        }
        let reservation = PasteReservation {
            budget: self.paste_budget.clone(),
            bytes: bytes.len(),
        };
        let (tx, rx) = async_channel::bounded(1);
        self.enqueue(Input::Raw(bytes.to_vec(), tx, reservation))
            .map_err(|error| match error {
                InputError::Admission(error) => AsyncGhosttyPumpError::Transport(error),
                InputError::Invalid(_) => unreachable!("enqueue only checks admission"),
            })?;
        let mut guard = ControlWait {
            endpoint: self,
            complete: false,
        };
        self.poll.notify().map_err(|error| {
            AsyncGhosttyPumpError::Transport(TerminalError::Transport(error.to_string()))
        })?;
        let result = rx.recv().await.map_err(|_| AsyncGhosttyPumpError::Closed)?;
        guard.complete = true;
        result
    }

    pub async fn clear_command(&self) -> Result<(), AsyncGhosttyPumpError> {
        let (tx, rx) = async_channel::bounded(1);
        self.enqueue(Input::ClearCommand(tx))
            .map_err(|error| match error {
                InputError::Admission(error) => AsyncGhosttyPumpError::Transport(error),
                InputError::Invalid(_) => unreachable!("enqueue only checks admission"),
            })?;
        let mut guard = ControlWait {
            endpoint: self,
            complete: false,
        };
        self.poll.notify().map_err(|error| {
            AsyncGhosttyPumpError::Transport(TerminalError::Transport(error.to_string()))
        })?;
        let result = rx.recv().await.map_err(|_| AsyncGhosttyPumpError::Closed)?;
        guard.complete = true;
        result
    }

    /// Success acknowledges a selected-cell publication, not physical paint.
    /// Invalid native anchors reject recoverably; cancellation after admission
    /// requests independent close and cannot undo an already applied selection.
    pub async fn update_selection(
        &self,
        update: SelectionUpdate,
    ) -> Result<(), AsyncGhosttyPumpError> {
        let (tx, rx) = async_channel::bounded(1);
        self.enqueue(Input::Selection(update, tx))
            .map_err(|error| match error {
                InputError::Admission(error) => AsyncGhosttyPumpError::Transport(error),
                InputError::Invalid(_) => unreachable!("enqueue only checks admission"),
            })?;
        let mut guard = ControlWait {
            endpoint: self,
            complete: false,
        };
        self.poll.notify().map_err(|error| {
            AsyncGhosttyPumpError::Transport(TerminalError::Transport(error.to_string()))
        })?;
        let result = rx.recv().await.map_err(|_| AsyncGhosttyPumpError::Closed)?;
        guard.complete = true;
        result
    }
    pub fn state(&self) -> SharedState {
        self.state.clone()
    }

    /// Apply native defaults in control order and acknowledge frame publication.
    /// Program OSC overrides survive; admitted cancellation independently closes.
    pub async fn set_default_colors(
        &self,
        colors: DefaultColors,
    ) -> Result<(), AsyncGhosttyPumpError> {
        let (tx, rx) = async_channel::bounded(1);
        self.enqueue(Input::DefaultColors(Box::new(colors), tx))
            .map_err(|error| match error {
                InputError::Admission(error) => AsyncGhosttyPumpError::Transport(error),
                InputError::Invalid(_) => unreachable!("enqueue only checks admission"),
            })?;
        let mut guard = ControlWait {
            endpoint: self,
            complete: false,
        };
        self.poll.notify().map_err(|error| {
            AsyncGhosttyPumpError::Transport(TerminalError::Transport(error.to_string()))
        })?;
        let result = rx.recv().await.map_err(|_| AsyncGhosttyPumpError::Closed)?;
        guard.complete = true;
        result
    }

    /// Cancellation of a polled wait drops this endpoint and requests shutdown.
    /// Success observes PTY/native destruction, not an OS-thread join.
    pub async fn wait(mut self) -> Result<Report, Error> {
        let result = self.result.recv().await.map_err(|_| Error::OwnerLost)?;
        self.completed = true;
        result
    }
}

impl Drop for NativeLocal {
    fn drop(&mut self) {
        if !self.completed {
            if let Err(error) = self.close() {
                log::warn!("Native local endpoint drop close: {error}");
            }
        }
    }
}

struct PollWake(Arc<Poller>);
impl Wake for PollWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        if let Err(error) = self.0.notify() {
            log::warn!("Native local future wake: {error}");
        }
    }
}

struct OwnerIo<P: EventedPty + OnResize> {
    pty: Option<P>,
    registered: bool,
    input: mpsc::Receiver<Cow<'static, [u8]>>,
    pending: Option<Cow<'static, [u8]>>,
    offset: usize,
    poll: Arc<Poller>,
    control: Arc<ShellControl>,
    admission: Arc<Admission>,
    // Outer Option distinguishes unseen status from an observed unknown code.
    exit: Option<Option<i32>>,
    cancelled: bool,
    controls: async_channel::Receiver<Input>,
}

impl<P: EventedPty + OnResize> OwnerIo<P> {
    fn stopped(&self) -> bool {
        self.control.shutdown.load(Ordering::Acquire)
    }

    fn observe_child(&mut self) {
        if let Some(pty) = &mut self.pty
            && let Some(ChildEvent::Exited(status)) = pty.next_child_event()
        {
            self.exit = Some(status.and_then(|status| status.code()));
        }
    }

    /// One bounded service turn; accepted bytes alone release budget. Never
    /// drains the bounded channel into a second growing owner-side queue.
    fn writes(&mut self) -> io::Result<()> {
        let Some(pty) = &mut self.pty else {
            return Ok(());
        };
        let mut remaining = MAX_LOCKED_READ;
        while remaining > 0 {
            if self.pending.is_none() {
                self.pending = self.input.try_recv().ok();
                self.offset = 0;
            }
            let Some(bytes) = &self.pending else {
                break;
            };
            if self.offset == bytes.len() {
                self.pending = None;
                continue;
            }
            let end = bytes.len().min(self.offset + remaining);
            match pty.writer().write(&bytes[self.offset..end]) {
                Ok(0) => break,
                Ok(n) => {
                    self.offset += n;
                    remaining -= n;
                    self.control.queued_input_bytes.release(n);
                    if self.offset == bytes.len() {
                        self.pending = None;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        if self.pending.is_none() {
            self.pending = self.input.try_recv().ok();
            self.offset = 0;
        }
        Ok(())
    }

    fn wait_poll(&mut self, readable: bool) -> io::Result<()> {
        if let Some(pty) = &mut self.pty {
            let mut interest = PollEvent::none(PTY_READ_WRITE_TOKEN);
            interest.readable = readable;
            interest.writable = self.pending.is_some();
            pty.reregister(&self.poll, interest, PollMode::Level)?;
        }
        let mut events = Events::with_capacity(POLL_EVENT_CAPACITY);
        loop {
            match self.poll.wait(&mut events, None) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => {
                    result?;
                    break;
                }
            }
        }
        self.observe_child();
        Ok(())
    }

    /// While native/UI awaits, service writes and close but read no more output.
    fn drive<F: Future>(
        &mut self,
        future: F,
        cancellable: bool,
    ) -> Result<Option<F::Output>, Error> {
        let waker = Waker::from(Arc::new(PollWake(self.poll.clone())));
        let mut cx = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            if cancellable && self.stopped() {
                return Ok(None);
            }
            if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
                return Ok(Some(value));
            }
            self.writes()?;
            self.wait_poll(false)?;
        }
    }

    fn dispose(&mut self) -> io::Result<()> {
        let _gate = self
            .admission
            .gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.control.shutdown.store(true, Ordering::Release);
        self.controls.close();
        while self.controls.try_recv().is_ok() {}
        if let Some(bytes) = self.pending.take() {
            self.control
                .queued_input_bytes
                .release(bytes.len() - self.offset);
        }
        while let Ok(bytes) = self.input.try_recv() {
            self.control.queued_input_bytes.release(bytes.len());
        }
        drop(_gate);
        let deregister = if self.registered {
            self.registered = false;
            self.pty
                .as_mut()
                .expect("registered PTY exists")
                .deregister(&self.poll)
        } else {
            Ok(())
        };
        // Windows drop can block and terminate the child: exclusively owner-side.
        self.pty.take();
        deregister
    }

    fn operation<T>(
        &mut self,
        future: impl Future<Output = Result<T, AsyncGhosttyPumpError>>,
    ) -> Result<bool, Error> {
        match self.drive(future, true)? {
            Some(result) => {
                result?;
                Ok(true)
            }
            None => {
                self.cancelled = true;
                Ok(false)
            }
        }
    }

    fn input(
        &mut self,
        pump: &mut AsyncGhosttyPump<LocalTransport>,
        input: Input,
    ) -> Result<bool, Error> {
        let done = match input {
            Input::Search(request, reply) => {
                if reply.is_closed() || self.stopped() {
                    self.cancelled = true;
                    return Ok(false);
                }
                let Some(result) = self.drive(
                    async {
                        use oneterm_terminal::ghostty::search::Request;
                        match request {
                            Request::Set(needle, options) => pump.search(needle, options).await,
                            Request::Step => pump.search_step().await,
                            Request::Reveal(navigation) => pump.navigate_search(navigation).await,
                        }
                    },
                    true,
                )?
                else {
                    self.cancelled = true;
                    return Ok(false);
                };
                let rejected = result
                    .as_ref()
                    .is_err_and(AsyncGhosttyPumpError::is_search_rejection);
                let failure = result.as_ref().err().cloned();
                if let Err(error) = reply.try_send(result) {
                    log::debug!("Native local search acknowledgment receiver closed: {error}");
                }
                if !rejected && let Some(error) = failure {
                    return Err(error.into());
                }
                return Ok(true);
            }
            Input::Key(key) => self.operation(pump.key(key))?,
            Input::Raw(bytes, reply, reservation) => {
                if reply.is_closed() || self.stopped() {
                    self.cancelled = true;
                    return Ok(false);
                }
                let Some(result) = self.drive(pump.write_visible(&bytes), true)? else {
                    self.cancelled = true;
                    return Ok(false);
                };
                drop(bytes);
                drop(reservation);
                if let Err(error) = reply.try_send(result.clone()) {
                    log::debug!("Native local raw input acknowledgment receiver closed: {error}");
                }
                result?;
                return Ok(true);
            }
            Input::KeyReceipt(key, reply) => {
                if reply.is_closed() || self.stopped() {
                    self.cancelled = true;
                    return Ok(false);
                }
                let Some(result) = self.drive(pump.key_receipt(key), true)? else {
                    self.cancelled = true;
                    return Ok(false);
                };
                let failure = result.as_ref().err().cloned();
                if let Err(error) = reply.try_send(result) {
                    log::debug!("Native local key receipt receiver closed: {error}");
                }
                if let Some(error) = failure {
                    return Err(error.into());
                }
                return Ok(true);
            }
            Input::Mouse(mouse) => self.operation(pump.mouse(mouse))?,
            Input::Wheel(wheel) => self.operation(pump.wheel(wheel))?,
            Input::Scroll(scroll) => return self.operation(pump.scroll_viewport(scroll)),
            Input::ScrollNow(scroll, reply) => {
                if reply.is_closed() || self.stopped() {
                    self.cancelled = true;
                    return Ok(false);
                }
                let Some(result) = self.drive(pump.scroll_viewport(scroll), true)? else {
                    self.cancelled = true;
                    return Ok(false);
                };
                let result = result.map(|_| ());
                if let Err(error) = reply.try_send(result.clone()) {
                    log::debug!(
                        "Native local search navigation acknowledgment receiver closed: {error}"
                    );
                }
                result?;
                return Ok(true);
            }
            Input::DefaultColors(colors, reply) => {
                if reply.is_closed() || self.stopped() {
                    self.cancelled = true;
                    return Ok(false);
                }
                let Some(result) = self.drive(pump.set_default_colors(colors), true)? else {
                    self.cancelled = true;
                    return Ok(false);
                };
                let result = result.map(|_| ());
                if let Err(error) = reply.try_send(result.clone()) {
                    log::debug!(
                        "Native local default-colors acknowledgment receiver closed: {error}"
                    );
                }
                result?;
                return Ok(true);
            }
            Input::ClearCommand(reply) => {
                if reply.is_closed() || self.stopped() {
                    self.cancelled = true;
                    return Ok(false);
                }
                let Some(result) = self.drive(pump.clear_command(), true)? else {
                    self.cancelled = true;
                    return Ok(false);
                };
                if let Err(error) = reply.try_send(result.clone()) {
                    log::debug!("Native local clear acknowledgment receiver closed: {error}");
                }
                result?;
                return Ok(true);
            }
            Input::Selection(update, reply) => {
                if reply.is_closed() || self.stopped() {
                    self.cancelled = true;
                    return Ok(false);
                }
                let Some(result) = self.drive(pump.update_selection(update), true)? else {
                    self.cancelled = true;
                    return Ok(false);
                };
                let result = result.map(|_| ());
                let rejected = result
                    .as_ref()
                    .is_err_and(AsyncGhosttyPumpError::is_selection_rejection);
                if let Err(error) = reply.try_send(result.clone()) {
                    log::debug!("Native local selection acknowledgment receiver closed: {error}");
                }
                if !rejected {
                    result?;
                }
                return Ok(true);
            }
            Input::Paste(paste) => {
                if paste.result.is_closed() || self.stopped() {
                    self.cancelled = true;
                    return Ok(false);
                }
                let Some(result) = self.drive(
                    async {
                        if paste.reveal {
                            pump.paste_visible(&paste.text).await
                        } else {
                            pump.paste(&paste.text).await
                        }
                    },
                    true,
                )?
                else {
                    self.cancelled = true;
                    return Ok(false);
                };
                // One-slot reply cannot backpressure the owner. A cancelled
                // receiver has its independent close guard, never a retry.
                if let Err(error) = paste.result.try_send(result.clone()) {
                    log::debug!("Native local paste acknowledgment receiver closed: {error}");
                }
                result?;
                return Ok(true);
            }
            Input::Resize(size) => {
                if !self.operation(pump.resize(
                    size.cols,
                    size.rows,
                    u32::from(size.cell_width),
                    u32::from(size.cell_height),
                ))? {
                    return Ok(false);
                }
                self.physical_resize(size)?;
                true
            }
        };
        if !done {
            return Ok(false);
        }
        self.operation(pump.finish_batch())
    }

    fn physical_resize(&mut self, size: WindowSize) -> io::Result<()> {
        let pending = self
            .control
            .pending_resize
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if !pending.is_some_and(|pending| pending.cols == size.cols && pending.rows == size.rows) {
            return Err(io::Error::other(
                "native local pending resize does not match active control",
            ));
        }
        self.pty
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "native local PTY absent"))?
            .on_resize(size)
    }
}

impl<P: EventedPty + OnResize> Drop for OwnerIo<P> {
    fn drop(&mut self) {
        if let Err(error) = self.dispose() {
            log::warn!("Native local PTY cleanup: {error}");
        }
    }
}

fn batches<P: EventedPty + OnResize>(
    io: &mut OwnerIo<P>,
    pump: &mut AsyncGhosttyPump<LocalTransport>,
) -> Result<Stop, Error> {
    let mut bytes = vec![0; MAX_LOCKED_READ];
    loop {
        if io.stopped() {
            return Ok(Stop::Requested);
        }
        io.writes()?;
        io.observe_child();
        if let Ok(input) = io.controls.try_recv()
            && !io.input(pump, input)?
        {
            return Ok(Stop::Requested);
        }
        if io.admission.sealed.load(Ordering::Acquire) {
            // Admission and the seal share one mutex. Once observed, no later
            // control can enter; preserve FIFO and read no more child output.
            if io.controls.is_empty() {
                return Ok(Stop::Requested);
            }
            continue;
        }
        let mut eof = false;
        let read = io
            .pty
            .as_mut()
            .expect("started PTY exists")
            .reader()
            .read(&mut bytes);
        match read {
            Ok(0) => {
                // Windows PipeReader uses zero for an empty live ring;
                // only the independent child watcher establishes exit.
                #[cfg(not(windows))]
                {
                    eof = true;
                }
            }
            Ok(n) => {
                if !io.operation(pump.process_chunk(&bytes[..n]))? {
                    return Ok(Stop::Requested);
                }
                // Bound reliable-event accumulation even under continuous output.
                if !io.operation(pump.finish_batch())? {
                    return Ok(Stop::Requested);
                }
                // Service one control between bounded read batches, even
                // when output is continuous. Read again without parking;
                // only an empty read/WouldBlock rearms readable interest.
                continue;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {
                eof = true;
            }
            Err(error) => return Err(error.into()),
        }
        io.observe_child();
        if let Some(code) = io.exit {
            if !io.operation(pump.publish_exit(code))? {
                return Ok(Stop::Requested);
            }
            return Ok(Stop::ChildExit);
        }
        if eof {
            return Ok(Stop::Eof);
        }
        if io.controls.is_empty() {
            io.wait_poll(true)?;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run(
    io: &mut OwnerIo<PseudoConsole>,
    mut pump: AsyncGhosttyPump<LocalTransport>,
    opts: Options,
    size: WindowSize,
    state: SharedState,
    events: SessionEventSink,
    logging: oneterm_terminal::TerminalLogController,
    logging_setup: (oneterm_core::TerminalLogConfig, std::path::PathBuf),
) -> Result<Report, Error> {
    let result = (|| {
        if io.stopped() || (io.admission.sealed.load(Ordering::Acquire) && io.controls.is_empty()) {
            return Ok(Stop::Requested);
        }
        io.pty = Some(PseudoConsole::spawn(&opts, size)?);
        if let Some(pid) = io.pty.as_ref().and_then(|pty| pty.child_pid()) {
            logging.set_identity(oneterm_terminal::local_log_identity(&logging_setup.1, pid));
        }
        if logging_setup.0.enabled
            && let Err(error) = logging.start(&logging_setup.0)
        {
            log::warn!("Native local automatic logging did not start: {error}");
        }
        // SAFETY: owner retains source and poller and deregisters before drop,
        // including unwinding via OwnerIo::drop, all on this same thread.
        unsafe {
            io.pty.as_mut().expect("spawned PTY exists").register(
                &io.poll,
                PollEvent::readable(PTY_READ_WRITE_TOKEN),
                PollMode::Level,
            )?;
        }
        io.registered = true;
        batches(io, &mut pump)
    })();
    // Close native admission synchronously before draining transport reservations.
    let teardown_started = std::time::Instant::now();
    log::debug!("Native local owner entering teardown: {result:?}");
    let native = pump.shutdown();
    state.set_alive(false);
    let physical = io.dispose();
    log::debug!(
        "Native local PTY disposed after {:?}: {physical:?}",
        teardown_started.elapsed()
    );
    let native = io.drive(native, false);
    log::debug!(
        "Native local engine disposed after {:?}",
        teardown_started.elapsed()
    );
    let admission = io.admission.clone();
    let closed = io.drive(
        async {
            // Immediate capacity wins over an already requested close.
            let send = events.send_checked(SessionEvent::Closed);
            let mut send = std::pin::pin!(send);
            std::future::poll_fn(|cx| {
                if let Poll::Ready(sent) = send.as_mut().poll(cx) {
                    return Poll::Ready(if sent {
                        Ok(LifecycleDelivery::Complete)
                    } else {
                        Err(Error::Native(AsyncGhosttyPumpError::EventsClosed))
                    });
                }
                if admission.requested.load(Ordering::Acquire) {
                    return Poll::Ready(Ok(LifecycleDelivery::Cancelled));
                }
                Poll::Pending
            })
            .await
        },
        false,
    );
    let stop = result?;
    physical?;
    let native = native?.expect("uncancellable native teardown");
    match native {
        Err(AsyncGhosttyPumpError::Engine(
            oneterm_terminal::ghostty_bridge::Error::AbandonedDelivery,
        )) if stop == Stop::Requested => {}
        other => {
            other?;
        }
    }
    let mut lifecycle = closed?.expect("uncancellable lifecycle driver")?;
    if io.cancelled {
        lifecycle = LifecycleDelivery::Cancelled;
    }
    Ok(Report {
        stop,
        exit_code: io.exit.flatten(),
        lifecycle,
    })
}

#[cfg(all(test, windows))]
#[path = "ghostty_local_tests.rs"]
mod tests;
