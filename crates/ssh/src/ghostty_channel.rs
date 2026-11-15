//! Opt-in native direct-channel owner. Not a production TerminalSession.
//! Caller supplies an authenticated direct connection and opened shell channel.

use std::future::Future;
use std::sync::Arc;

use oneterm_terminal::backend::{AsyncGhosttyPump, AsyncGhosttyPumpError};
use oneterm_terminal::ghostty::colors::DefaultColors;
use oneterm_terminal::ghostty::input::{KeyEvent, MouseEvent};
use oneterm_terminal::ghostty::selection::Update as SelectionUpdate;
use oneterm_terminal::ghostty::state::Scroll;
use oneterm_terminal::ghostty_model::SharedModel;
use oneterm_terminal::{
    ByteBudget, ClipboardOrigin, OscRouter, PtyTransport, SessionEvent, SessionEventSink,
    SharedSessionState, SharedState, TerminalError,
};
use russh::{ChannelMsg, ChannelReadHalf, ChannelWriteHalf, client};
use tokio_util::sync::CancellationToken;

use crate::transport::{Cmd, SSH_COMMAND_QUEUE_CAPACITY, SshTransport};

const INPUT_QUEUE_CAPACITY: usize = 64;
const PASTE_BYTE_BUDGET: usize = 4 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct Services {
    pub(crate) carriers: crate::route::JumpHandles,
    pub(crate) shutdown: CancellationToken,
    pub(crate) tasks: tokio_util::task::TaskTracker,
    pub(crate) opens: Option<tokio::sync::mpsc::Receiver<crate::tunnel::HandleRequest>>,
    pub(crate) logging: oneterm_core::TerminalLogConfig,
    pub(crate) identity: String,
}

impl Drop for Services {
    fn drop(&mut self) {
        self.shutdown.cancel();
        self.tasks.close();
    }
}

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
    endpoint: &'a NativeChannel,
    complete: bool,
}

impl Drop for ControlWait<'_> {
    fn drop(&mut self) {
        if !self.complete
            && let Err(error) = self.endpoint.close()
        {
            log::warn!("Native SSH cancelled control close: {error}");
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InputError {
    #[error(transparent)]
    Invalid(#[from] oneterm_terminal::ghostty::Error),
    #[error(transparent)]
    Admission(#[from] TerminalError),
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
    Resize(Resize),
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

struct Resize {
    cols: u16,
    rows: u16,
    cell_width_px: u32,
    cell_height_px: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Native(#[from] AsyncGhosttyPumpError),
    #[error(transparent)]
    Ssh(#[from] russh::Error),
    #[error("Native SSH owner task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("Native SSH command channel closed unexpectedly")]
    CommandsClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    Requested,
    Eof,
    PeerClosed,
    Disconnected,
}

/// Normal delivery waits for UI capacity. Explicit endpoint close may cancel
/// pending lifecycle delivery; it must not be mistaken for successful delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleDelivery {
    Complete,
    Cancelled,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub stop: Stop,
    pub exit_code: Option<i32>,
    pub lifecycle: LifecycleDelivery,
}

/// An owned experimental stream, deliberately not a TerminalSession adapter.
/// Drop requests close; wait observes physical teardown and native destruction.
pub struct NativeChannel {
    logging: oneterm_terminal::TerminalLogController,
    sftp: Option<Arc<crate::sftp::SftpSession>>,
    sftp_shutdown: Option<CancellationToken>,
    admission: std::sync::Mutex<()>,
    transport: SshTransport,
    inputs: async_channel::Sender<Input>,
    close: CancellationToken,
    model: SharedModel,
    state: SharedState,
    task: tokio::task::JoinHandle<Result<Report, Error>>,
    paste_budget: Arc<ByteBudget<PASTE_BYTE_BUDGET>>,
    composition: oneterm_terminal::ghostty_ime::Composition,
}

impl oneterm_terminal::ghostty_content::RenderSource for NativeChannel {
    fn render_model(&self) -> SharedModel {
        self.model.clone()
    }
}

impl oneterm_terminal::ghostty_session::SurfaceSession for NativeChannel {
    fn clear_command(&self) -> oneterm_terminal::ghostty_session::PasteFuture<'_> {
        Box::pin(self.clear_command())
    }
    fn set_default_colors(
        &self,
        colors: oneterm_terminal::ghostty::colors::DefaultColors,
    ) -> oneterm_terminal::ghostty_session::ColorFuture<'_> {
        Box::pin(NativeChannel::set_default_colors(self, colors))
    }
    fn reveal_search(
        &self,
        navigation: oneterm_terminal::ghostty::search::Navigation,
    ) -> oneterm_terminal::ghostty_session::SearchFuture<'_> {
        Box::pin(
            self.search_request(oneterm_terminal::ghostty::search::Request::Reveal(
                navigation,
            )),
        )
    }
    fn search(
        &self,
        needle: String,
        options: oneterm_terminal::ghostty::search::Options,
    ) -> oneterm_terminal::ghostty_session::SearchFuture<'_> {
        Box::pin(
            self.search_request(oneterm_terminal::ghostty::search::Request::Set(
                needle, options,
            )),
        )
    }
    fn search_step(&self) -> oneterm_terminal::ghostty_session::SearchFuture<'_> {
        Box::pin(self.search_request(oneterm_terminal::ghostty::search::Request::Step))
    }
    fn scroll_viewport(
        &self,
        scroll: Scroll,
    ) -> oneterm_terminal::ghostty_session::PasteFuture<'_> {
        Box::pin(self.scroll_viewport(scroll))
    }
    fn selection(
        &self,
        update: oneterm_terminal::ghostty::selection::Update,
    ) -> oneterm_terminal::ghostty_session::PasteFuture<'_> {
        Box::pin(self.update_selection(update))
    }
    fn paste_visible<'a>(
        &'a self,
        text: &'a str,
    ) -> oneterm_terminal::ghostty_session::PasteFuture<'a> {
        Box::pin(self.paste_visible(text))
    }
    fn wheel(
        &self,
        wheel: oneterm_terminal::ghostty::wheel::WheelEvent,
    ) -> Result<(), oneterm_terminal::ghostty_session::InputError> {
        self.wheel(wheel).map_err(surface_error)
    }
    fn mouse(
        &self,
        mouse: MouseEvent,
    ) -> Result<(), oneterm_terminal::ghostty_session::InputError> {
        self.mouse(mouse).map_err(surface_error)
    }
    fn resize(
        &self,
        size: oneterm_terminal::ghostty_session::Size,
    ) -> Result<(), oneterm_terminal::ghostty_session::InputError> {
        self.resize(
            size.cols,
            size.rows,
            u32::from(size.cell_width_px),
            u32::from(size.cell_height_px),
        )
        .map_err(surface_error)
    }
    fn key(&self, key: KeyEvent) -> Result<(), oneterm_terminal::ghostty_session::InputError> {
        self.key(key).map_err(surface_error)
    }
    fn key_receipt(
        &self,
        key: KeyEvent,
    ) -> oneterm_terminal::ghostty_session::KeyReceiptFuture<'_> {
        Box::pin(self.key_receipt(key))
    }
    fn write_visible<'a>(
        &'a self,
        bytes: &'a [u8],
    ) -> oneterm_terminal::ghostty_session::PasteFuture<'a> {
        Box::pin(self.write_visible(bytes))
    }
    fn release_repaint_hint(&self) {
        self.state.release_repaint_hint();
    }
    fn set_marked_text(&self, text: String) -> Result<(), oneterm_terminal::ghostty_ime::Error> {
        self.set_marked_text(text)
    }
    fn marked_text(&self) -> Option<String> {
        self.marked_text()
    }
    fn clear_marked_text(&self) {
        self.clear_marked_text();
    }
    fn commit_text(&self, text: &str) -> Result<(), oneterm_terminal::ghostty_ime::Error> {
        self.commit_text(text)
    }
}

fn surface_error(error: InputError) -> oneterm_terminal::ghostty_session::InputError {
    match error {
        InputError::Invalid(error) => oneterm_terminal::ghostty_session::InputError::Invalid(error),
        InputError::Admission(error) => {
            oneterm_terminal::ghostty_session::InputError::Admission(error)
        }
    }
}

impl NativeChannel {
    /// Session-local preedit only; callers request redraw themselves.
    pub fn set_marked_text(
        &self,
        text: String,
    ) -> Result<(), oneterm_terminal::ghostty_ime::Error> {
        self.composition.set_marked_text(text)
    }

    pub fn marked_text(&self) -> Option<String> {
        self.composition.marked_text()
    }

    pub fn clear_marked_text(&self) {
        self.composition.clear_marked_text();
    }

    /// Clear preedit and enqueue exact UTF-8, not paste. Success is admission.
    pub fn commit_text(&self, text: &str) -> Result<(), oneterm_terminal::ghostty_ime::Error> {
        self.composition
            .commit_text(text, |bytes| self.write(bytes))
    }

    /// Must run inside a tokio runtime. No new authentication or trust policy.
    /// This low-level entry owns a direct handle; the application factory passes
    /// its jump, forwarding and SFTP owners through spawn_with_sftp instead.
    pub fn spawn<H: client::Handler + Send + 'static>(
        handle: client::Handle<H>,
        channel: russh::Channel<client::Msg>,
        cols: u16,
        rows: u16,
        events: async_channel::Sender<SessionEvent>,
    ) -> Result<Self, Error> {
        Self::spawn_configured(
            handle,
            channel,
            cols,
            rows,
            events,
            Default::default(),
            None,
        )
    }

    /// Application-owned direct connection with explicit remote OSC policy and history.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_configured<H: client::Handler + Send + 'static>(
        handle: client::Handle<H>,
        channel: russh::Channel<client::Msg>,
        cols: u16,
        rows: u16,
        events: async_channel::Sender<SessionEvent>,
        security: oneterm_terminal::TerminalSecurityPolicy,
        scrollback: Option<usize>,
    ) -> Result<Self, Error> {
        Self::spawn_with_sftp(
            handle,
            channel,
            cols,
            rows,
            events,
            security,
            scrollback,
            SharedSessionState::new_alive(),
            None,
            Services::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn spawn_with_sftp<H: client::Handler + Send + 'static>(
        handle: client::Handle<H>,
        channel: russh::Channel<client::Msg>,
        cols: u16,
        rows: u16,
        events: async_channel::Sender<SessionEvent>,
        security: oneterm_terminal::TerminalSecurityPolicy,
        scrollback: Option<usize>,
        state: SharedState,
        sftp_channel: Option<crate::native_sftp::Prepared>,
        mut services: Services,
    ) -> Result<Self, Error> {
        let (tx, commands) = async_channel::bounded(SSH_COMMAND_QUEUE_CAPACITY);
        let transport = SshTransport::new(tx);
        let (inputs, input_rx) = async_channel::bounded(INPUT_QUEUE_CAPACITY);
        let close = CancellationToken::new();
        let events = SessionEventSink::new(events);
        let router = OscRouter::with_security(
            transport.clone(),
            events.clone(),
            state.clone(),
            ClipboardOrigin::Remote,
            security,
        );
        let pump = AsyncGhosttyPump::spawn_with_scrollback(router, cols, rows, scrollback)?;
        let logging = pump.logging();
        logging.set_identity(services.identity.clone());
        if services.logging.enabled
            && let Err(error) = logging.start(&services.logging)
        {
            log::warn!("Native SSH automatic logging did not start: {error}");
        }
        let model = pump.model();
        let io = OwnerIo {
            transport: transport.clone(),
            commands,
            inputs: input_rx,
            close: close.clone(),
            state: state.clone(),
            events,
        };
        let sftp_owner = sftp_channel.map(crate::native_sftp::Owner::spawn);
        let sftp = sftp_owner.as_ref().map(|owner| owner.backend.clone());
        let sftp_shutdown = sftp_owner.as_ref().map(|owner| owner.shutdown.clone());
        let task = tokio::spawn(async move {
            let operation = run(channel, pump, io);
            let requests = async {
                if let Some(opens) = &mut services.opens {
                    while let Some(request) = opens.recv().await {
                        tokio::select! {
                            _ = services.shutdown.cancelled() => break,
                            _ = crate::tunnel::serve_native_handle_request(&handle, request) => {}
                        }
                    }
                }
                std::future::pending::<()>().await;
            };
            let result = tokio::select! {
                result = operation => result,
                _ = requests => unreachable!("request service remains pending"),
            };
            services.shutdown.cancel();
            services.opens.take();
            services.tasks.close();
            services.tasks.wait().await;
            let sftp_result = if let Some(owner) = sftp_owner {
                owner.complete().await
            } else {
                Ok(())
            };
            // The connection stays alive through channel/native teardown.
            drop(handle);
            drop(services);
            if let Err(error) = sftp_result {
                log::error!("Native SFTP owner failed: {error}");
                return Err(Error::Join(error));
            }
            result
        });
        Ok(Self {
            logging,
            sftp,
            sftp_shutdown,
            admission: std::sync::Mutex::new(()),
            transport,
            composition: Default::default(),
            inputs,
            close,
            model,
            state,
            task,
            paste_budget: Arc::default(),
        })
    }

    pub fn model(&self) -> SharedModel {
        self.model.clone()
    }

    pub fn state(&self) -> SharedState {
        self.state.clone()
    }

    pub(crate) fn logging(&self) -> oneterm_terminal::TerminalLogController {
        self.logging.clone()
    }

    pub(crate) fn sftp(&self) -> Option<Arc<dyn oneterm_core::SftpBackend>> {
        self.sftp
            .as_ref()
            .map(|backend| backend.clone() as Arc<dyn oneterm_core::SftpBackend>)
    }

    pub fn write(&self, bytes: &[u8]) -> Result<(), TerminalError> {
        let _admission = self
            .admission
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.inputs.is_closed() {
            return Err(TerminalError::Closed);
        }
        self.transport.pty_write(bytes)
    }

    /// Reply to an admitted request on this channel, never through paste/fan-out.
    pub fn reply_clipboard(&self, text: &str) -> Result<(), TerminalError> {
        self.write(&oneterm_terminal::ghostty_session::clipboard_reply(text)?)
    }

    /// Admit physical input, not layout-to-key guessing or delivery confirmation.
    /// Raw writes may overtake queued inputs while a batch awaits UI delivery.
    pub fn key(&self, key: KeyEvent) -> Result<(), InputError> {
        key.validate()?;
        self.admit_input(Input::Key(key))
    }

    /// Return exact native bytes after original FIFO admission, never resend.
    pub async fn key_receipt(&self, key: KeyEvent) -> Result<Vec<u8>, AsyncGhosttyPumpError> {
        key.validate().map_err(|error| {
            AsyncGhosttyPumpError::Engine(
                oneterm_terminal::ghostty::owner::Error::Native(error).into(),
            )
        })?;
        let (tx, rx) = async_channel::bounded(1);
        self.admit_input(Input::KeyReceipt(key, tx))
            .map_err(|error| match error {
                InputError::Admission(error) => AsyncGhosttyPumpError::Transport(error),
                InputError::Invalid(_) => unreachable!("admission only checks the queue"),
            })?;
        let mut guard = ControlWait {
            endpoint: self,
            complete: false,
        };
        let result = rx.recv().await.map_err(|_| AsyncGhosttyPumpError::Closed)?;
        guard.complete = true;
        result
    }

    pub fn mouse(&self, mouse: MouseEvent) -> Result<(), InputError> {
        mouse.validate()?;
        self.admit_input(Input::Mouse(mouse))
    }

    pub fn wheel(
        &self,
        wheel: oneterm_terminal::ghostty::wheel::WheelEvent,
    ) -> Result<(), InputError> {
        wheel.validate().map_err(InputError::Invalid)?;
        self.admit_input(Input::Wheel(wheel))
    }

    /// Queue a coordinated native/peer resize at a batch boundary. Success is
    /// admission, not remote acknowledgment or resized-frame publication.
    pub fn resize(
        &self,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result<(), InputError> {
        oneterm_terminal::ghostty::render::check_budget(cols, rows)?;
        self.admit_input(Input::Resize(Resize {
            cols,
            rows,
            cell_width_px,
            cell_height_px,
        }))
    }

    fn admit_input(&self, input: Input) -> Result<(), InputError> {
        if self.transport.is_closing() {
            return Err(TerminalError::Closed.into());
        }
        self.inputs.try_send(input).map_err(|error| {
            match error {
                async_channel::TrySendError::Full(_) => TerminalError::QueueFull,
                async_channel::TrySendError::Closed(_) => TerminalError::Closed,
            }
            .into()
        })
    }

    /// Queue a native viewport change/publication, not a physical peer request.
    /// Success means admission; negative delta moves toward older history.
    pub fn scroll(&self, scroll: Scroll) -> Result<(), InputError> {
        self.admit_input(Input::Scroll(scroll))
    }

    /// Executes in control order with live native mode. Success acknowledges
    /// original transport FIFO admission, not peer receipt. Dropping an admitted
    /// future requests independent close; already enqueued bytes cannot retract.
    pub async fn paste(&self, text: &str) -> Result<(), AsyncGhosttyPumpError> {
        self.paste_request(text, false).await
    }

    pub async fn paste_visible(&self, text: &str) -> Result<(), AsyncGhosttyPumpError> {
        self.paste_request(text, true).await
    }

    async fn paste_request(&self, text: &str, reveal: bool) -> Result<(), AsyncGhosttyPumpError> {
        AsyncGhosttyPump::<SshTransport>::validate_paste(text)
            .map_err(AsyncGhosttyPumpError::Paste)?;
        if !self.paste_budget.reserve(text.len()) {
            return Err(AsyncGhosttyPumpError::Transport(TerminalError::QueueFull));
        }
        let reservation = PasteReservation {
            budget: self.paste_budget.clone(),
            bytes: text.len(),
        };
        let (tx, rx) = async_channel::bounded(1);
        self.admit_input(Input::Paste(Paste {
            reveal,
            text: text.to_owned(),
            result: tx,
            _reservation: reservation,
        }))
        .map_err(|error| match error {
            InputError::Admission(error) => AsyncGhosttyPumpError::Transport(error),
            InputError::Invalid(_) => unreachable!("admission does not validate physical input"),
        })?;
        let mut guard = ControlWait {
            endpoint: self,
            complete: false,
        };
        let result = rx.recv().await.map_err(|_| AsyncGhosttyPumpError::Closed)?;
        guard.complete = true;
        result
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
        self.admit_input(Input::Raw(bytes.to_vec(), tx, reservation))
            .map_err(|error| match error {
                InputError::Admission(error) => AsyncGhosttyPumpError::Transport(error),
                InputError::Invalid(_) => unreachable!("admission only checks the queue"),
            })?;
        let mut guard = ControlWait {
            endpoint: self,
            complete: false,
        };
        let result = rx.recv().await.map_err(|_| AsyncGhosttyPumpError::Closed)?;
        guard.complete = true;
        result
    }

    /// Send the shell command and publish cleared selection; no paste encoding.
    /// Post-admission cancellation independently requests close.
    pub async fn clear_command(&self) -> Result<(), AsyncGhosttyPumpError> {
        let (tx, rx) = async_channel::bounded(1);
        self.admit_input(Input::ClearCommand(tx))
            .map_err(|error| match error {
                InputError::Admission(error) => AsyncGhosttyPumpError::Transport(error),
                InputError::Invalid(_) => unreachable!("admission does not validate clear"),
            })?;
        let mut guard = ControlWait {
            endpoint: self,
            complete: false,
        };
        let result = rx.recv().await.map_err(|_| AsyncGhosttyPumpError::Closed)?;
        guard.complete = true;
        result
    }

    /// Acknowledge selected-cell publication, not paint or clipboard I/O.
    pub async fn update_selection(
        &self,
        update: SelectionUpdate,
    ) -> Result<(), AsyncGhosttyPumpError> {
        let (tx, rx) = async_channel::bounded(1);
        self.admit_input(Input::Selection(update, tx))
            .map_err(|error| match error {
                InputError::Admission(error) => AsyncGhosttyPumpError::Transport(error),
                InputError::Invalid(_) => {
                    unreachable!("admission does not validate native selection")
                }
            })?;
        let mut guard = ControlWait {
            endpoint: self,
            complete: false,
        };
        let result = rx.recv().await.map_err(|_| AsyncGhosttyPumpError::Closed)?;
        guard.complete = true;
        result
    }

    pub async fn scroll_viewport(&self, scroll: Scroll) -> Result<(), AsyncGhosttyPumpError> {
        let (tx, rx) = async_channel::bounded(1);
        self.admit_input(Input::ScrollNow(scroll, tx))
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
        self.admit_input(Input::Search(request, tx))
            .map_err(|error| match error {
                InputError::Admission(error) => AsyncGhosttyPumpError::Transport(error),
                InputError::Invalid(_) => unreachable!("search admission only checks the queue"),
            })?;
        let mut guard = ControlWait {
            endpoint: self,
            complete: false,
        };
        let result = rx.recv().await.map_err(|_| AsyncGhosttyPumpError::Closed)?;
        guard.complete = true;
        result
    }

    /// Apply caller defaults and acknowledge publication, not physical paint.
    /// Program OSC overrides survive; admitted cancellation independently closes.
    pub async fn set_default_colors(
        &self,
        colors: DefaultColors,
    ) -> Result<(), AsyncGhosttyPumpError> {
        let (tx, rx) = async_channel::bounded(1);
        self.admit_input(Input::DefaultColors(Box::new(colors), tx))
            .map_err(|error| match error {
                InputError::Admission(error) => AsyncGhosttyPumpError::Transport(error),
                InputError::Invalid(_) => {
                    unreachable!("admission does not validate default colors")
                }
            })?;
        let mut guard = ControlWait {
            endpoint: self,
            complete: false,
        };
        let result = rx.recv().await.map_err(|_| AsyncGhosttyPumpError::Closed)?;
        guard.complete = true;
        result
    }

    /// Seal input admission and finish admitted controls before owner teardown.
    /// Hard close remains independent and can interrupt an unresponsive peer.
    pub fn request_shutdown(&self) -> Result<(), TerminalError> {
        if let Some(stop) = &self.sftp_shutdown {
            stop.cancel();
        }
        let _admission = self
            .admission
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.inputs.close();
        Ok(())
    }

    /// Close is independent of FIFO/UI capacity, and does not wait for native.
    pub fn close(&self) -> Result<(), TerminalError> {
        if let Some(stop) = &self.sftp_shutdown {
            stop.cancel();
        }
        // Teardown may already have closed transport admission while final
        // lifecycle delivery waits for UI. Its token must still be signalled.
        let result = if self.transport.is_closing() {
            Ok(())
        } else {
            self.transport.pty_close()
        };
        self.close.cancel();
        result
    }

    pub async fn wait(self) -> Result<Report, Error> {
        // Borrow the JoinHandle so cancelled wait still drops this endpoint and
        // requests close rather than silently detaching a live connection.
        let mut endpoint = self;
        (&mut endpoint.task).await?
    }
}

impl Drop for NativeChannel {
    fn drop(&mut self) {
        if !self.transport.is_closing()
            && let Err(error) = self.close()
        {
            log::warn!("native SSH endpoint drop close failed: {error}");
        }
        self.close.cancel();
    }
}

struct OwnerIo {
    transport: SshTransport,
    commands: async_channel::Receiver<Cmd>,
    inputs: async_channel::Receiver<Input>,
    close: CancellationToken,
    state: SharedState,
    events: SessionEventSink,
}

enum Driver<T> {
    Done(T),
    Close,
}

impl OwnerIo {
    async fn default_colors(
        &self,
        pump: &mut AsyncGhosttyPump<SshTransport>,
        writer: &ChannelWriteHalf<client::Msg>,
        colors: Box<DefaultColors>,
        reply: async_channel::Sender<Result<(), AsyncGhosttyPumpError>>,
    ) -> Result<Driver<()>, Error> {
        if reply.is_closed() {
            self.close.cancel();
            return Ok(Driver::Close);
        }
        let Driver::Done(result) = self
            .drive(async { Ok(pump.set_default_colors(colors).await) }, writer)
            .await?
        else {
            return Ok(Driver::Close);
        };
        let result = result.map(|_| ());
        if let Err(error) = reply.try_send(result.clone()) {
            log::debug!("Native SSH default-colors acknowledgment receiver closed: {error}");
        }
        result?;
        Ok(Driver::Done(()))
    }

    async fn clear_command(
        &self,
        pump: &mut AsyncGhosttyPump<SshTransport>,
        writer: &ChannelWriteHalf<client::Msg>,
        reply: async_channel::Sender<Result<(), AsyncGhosttyPumpError>>,
    ) -> Result<Driver<()>, Error> {
        if reply.is_closed() {
            self.close.cancel();
            return Ok(Driver::Close);
        }
        let Driver::Done(result) = self
            .drive(async { Ok(pump.clear_command().await) }, writer)
            .await?
        else {
            return Ok(Driver::Close);
        };
        if let Err(error) = reply.try_send(result.clone()) {
            log::debug!("Native SSH clear acknowledgment receiver closed: {error}");
        }
        result?;
        Ok(Driver::Done(()))
    }

    async fn key_receipt(
        &self,
        pump: &mut AsyncGhosttyPump<SshTransport>,
        writer: &ChannelWriteHalf<client::Msg>,
        key: KeyEvent,
        reply: async_channel::Sender<Result<Vec<u8>, AsyncGhosttyPumpError>>,
    ) -> Result<Driver<()>, Error> {
        if reply.is_closed() {
            self.close.cancel();
            return Ok(Driver::Close);
        }
        let Driver::Done(result) = self
            .drive(async { Ok(pump.key_receipt(key).await) }, writer)
            .await?
        else {
            return Ok(Driver::Close);
        };
        let failure = result.as_ref().err().cloned();
        if let Err(error) = reply.try_send(result) {
            log::debug!("Native SSH key receipt receiver closed: {error}");
        }
        if let Some(error) = failure {
            return Err(error.into());
        }
        Ok(Driver::Done(()))
    }

    async fn raw_input(
        &self,
        pump: &mut AsyncGhosttyPump<SshTransport>,
        writer: &ChannelWriteHalf<client::Msg>,
        bytes: Vec<u8>,
        reply: async_channel::Sender<Result<(), AsyncGhosttyPumpError>>,
        reservation: PasteReservation,
    ) -> Result<Driver<()>, Error> {
        if reply.is_closed() {
            self.close.cancel();
            return Ok(Driver::Close);
        }
        let Driver::Done(result) = self
            .drive(async { Ok(pump.write_visible(&bytes).await) }, writer)
            .await?
        else {
            return Ok(Driver::Close);
        };
        drop(bytes);
        drop(reservation);
        if let Err(error) = reply.try_send(result.clone()) {
            log::debug!("Native SSH raw input acknowledgment receiver closed: {error}");
        }
        result?;
        Ok(Driver::Done(()))
    }

    async fn selection(
        &self,
        pump: &mut AsyncGhosttyPump<SshTransport>,
        writer: &ChannelWriteHalf<client::Msg>,
        update: SelectionUpdate,
        reply: async_channel::Sender<Result<(), AsyncGhosttyPumpError>>,
    ) -> Result<Driver<()>, Error> {
        if reply.is_closed() {
            self.close.cancel();
            return Ok(Driver::Close);
        }
        let Driver::Done(result) = self
            .drive(async { Ok(pump.update_selection(update).await) }, writer)
            .await?
        else {
            return Ok(Driver::Close);
        };
        let result = result.map(|_| ());
        let rejected = result
            .as_ref()
            .is_err_and(AsyncGhosttyPumpError::is_selection_rejection);
        if let Err(error) = reply.try_send(result.clone()) {
            log::debug!("Native SSH selection acknowledgment receiver closed: {error}");
        }
        if !rejected {
            result?;
        }
        Ok(Driver::Done(()))
    }

    async fn scroll_now(
        &self,
        pump: &mut AsyncGhosttyPump<SshTransport>,
        writer: &ChannelWriteHalf<client::Msg>,
        scroll: Scroll,
        reply: async_channel::Sender<Result<(), AsyncGhosttyPumpError>>,
    ) -> Result<Driver<()>, Error> {
        if reply.is_closed() {
            self.close.cancel();
            return Ok(Driver::Close);
        }
        let Driver::Done(result) = self
            .drive(async { Ok(pump.scroll_viewport(scroll).await) }, writer)
            .await?
        else {
            return Ok(Driver::Close);
        };
        let result = result.map(|_| ());
        if let Err(error) = reply.try_send(result.clone()) {
            log::debug!("Native SSH search navigation acknowledgment receiver closed: {error}");
        }
        result?;
        Ok(Driver::Done(()))
    }

    async fn search(
        &self,
        pump: &mut AsyncGhosttyPump<SshTransport>,
        writer: &ChannelWriteHalf<client::Msg>,
        request: oneterm_terminal::ghostty::search::Request,
        reply: async_channel::Sender<
            Result<oneterm_terminal::ghostty::search::Snapshot, AsyncGhosttyPumpError>,
        >,
    ) -> Result<Driver<()>, Error> {
        if reply.is_closed() {
            self.close.cancel();
            return Ok(Driver::Close);
        }
        let Driver::Done(result) = self
            .drive(
                async {
                    use oneterm_terminal::ghostty::search::Request;
                    Ok(match request {
                        Request::Set(needle, options) => pump.search(needle, options).await,
                        Request::Step => pump.search_step().await,
                        Request::Reveal(navigation) => pump.navigate_search(navigation).await,
                    })
                },
                writer,
            )
            .await?
        else {
            return Ok(Driver::Close);
        };
        let rejected = result
            .as_ref()
            .is_err_and(AsyncGhosttyPumpError::is_search_rejection);
        let failure = result.as_ref().err().cloned();
        if let Err(error) = reply.try_send(result) {
            log::debug!("Native SSH search acknowledgment receiver closed: {error}");
        }
        if !rejected && let Some(error) = failure {
            return Err(error.into());
        }
        Ok(Driver::Done(()))
    }

    async fn paste(
        &self,
        pump: &mut AsyncGhosttyPump<SshTransport>,
        writer: &ChannelWriteHalf<client::Msg>,
        paste: Paste,
    ) -> Result<Driver<()>, Error> {
        if paste.result.is_closed() {
            self.close.cancel();
            return Ok(Driver::Close);
        }
        // Preserve the native result for the caller before propagating it to
        // owner teardown. drive continues servicing the original physical FIFO.
        let Driver::Done(result) = self
            .drive(
                async {
                    Ok(if paste.reveal {
                        pump.paste_visible(&paste.text).await
                    } else {
                        pump.paste(&paste.text).await
                    })
                },
                writer,
            )
            .await?
        else {
            return Ok(Driver::Close);
        };
        let Paste {
            text,
            result: reply,
            _reservation: reservation,
            ..
        } = paste;
        drop(text);
        drop(reservation);
        // The one-slot acknowledgment never backpressures the owner. A dropped
        // receiver requests independent close via ControlWait, never a retry.
        if let Err(error) = reply.try_send(result.clone()) {
            log::debug!("Native SSH paste acknowledgment receiver closed: {error}");
        }
        result?;
        Ok(Driver::Done(()))
    }

    async fn publish_closed(&self) -> Result<LifecycleDelivery, Error> {
        tokio::select! {
            biased;
            delivered = self.events.send_checked(SessionEvent::Closed) => {
                if !delivered {
                    return Err(AsyncGhosttyPumpError::EventsClosed.into());
                }
                Ok(LifecycleDelivery::Complete)
            }
            _ = self.close.cancelled() => Ok(LifecycleDelivery::Cancelled),
        }
    }

    fn command<'a>(
        &'a self,
        command: Result<Cmd, async_channel::RecvError>,
        writer: &'a ChannelWriteHalf<client::Msg>,
    ) -> impl Future<Output = Result<(), Error>> + 'a {
        // Arm before constructing the future: a close-ready select can drop it
        // without ever polling the write after the receiver consumed its bytes.
        let budget = match &command {
            Ok(Cmd::Write(bytes)) => Some(WriteBudget {
                transport: &self.transport,
                bytes: bytes.len(),
            }),
            _ => None,
        };
        async move {
            let _budget = budget;
            match command {
                Ok(Cmd::Write(bytes)) => {
                    // Even if cancellation drops this write future, release the
                    // reservation exactly once. Never replay a partially sent write.
                    writer.data(&bytes[..]).await?;
                    self.state.add_tx_bytes(bytes.len() as u64);
                    Ok(())
                }
                Ok(Cmd::Close) => {
                    self.close.cancel();
                    Ok(())
                }
                Ok(Cmd::Resize) => self.resize_channel(writer).await,
                Err(_) => Err(Error::CommandsClosed),
            }
        }
    }

    async fn resize_channel(&self, writer: &ChannelWriteHalf<client::Msg>) -> Result<(), Error> {
        if let Some((rows, cols)) = self.transport.take_pending_resize() {
            writer
                .window_change(u32::from(cols), u32::from(rows), 0, 0)
                .await?;
        }
        Ok(())
    }

    async fn flush_resize(
        &self,
        writer: &ChannelWriteHalf<client::Msg>,
    ) -> Result<Driver<()>, Error> {
        // Bound this drain to commands already queued at pump completion. A
        // direct pending-size drain must not overtake earlier encoded input.
        // A full queue may contain no resize marker, so also drain the pending
        // size after those commands, without waiting for another remote read.
        for _ in 0..self.commands.len() {
            let Ok(command) = self.commands.try_recv() else {
                break;
            };
            tokio::select! {
                biased;
                _ = self.close.cancelled() => return Ok(Driver::Close),
                result = self.command(Ok(command), writer) => result?,
            }
        }
        tokio::select! {
            biased;
            _ = self.close.cancelled() => Ok(Driver::Close),
            result = self.resize_channel(writer) => {
                result?;
                Ok(Driver::Done(()))
            }
        }
    }

    async fn resize(
        &self,
        pump: &mut AsyncGhosttyPump<SshTransport>,
        writer: &ChannelWriteHalf<client::Msg>,
        size: Resize,
    ) -> Result<Driver<()>, Error> {
        let Resize {
            cols,
            rows,
            cell_width_px,
            cell_height_px,
        } = size;
        if matches!(
            self.drive(
                pump.resize(cols, rows, cell_width_px, cell_height_px),
                writer
            )
            .await?,
            Driver::Close
        ) {
            return Ok(Driver::Close);
        }
        // Pump completion means transport admission, not marker consumption.
        // Submit the physical size before capture even if pump-ready won first.
        if matches!(self.flush_resize(writer).await?, Driver::Close) {
            return Ok(Driver::Close);
        }
        match self.drive(pump.finish_batch(), writer).await? {
            Driver::Done(_) => Ok(Driver::Done(())),
            Driver::Close => Ok(Driver::Close),
        }
    }

    async fn drive<T>(
        &self,
        future: impl Future<Output = Result<T, AsyncGhosttyPumpError>>,
        writer: &ChannelWriteHalf<client::Msg>,
    ) -> Result<Driver<T>, Error> {
        let future = std::pin::pin!(future);
        let mut future = future;
        loop {
            tokio::select! {
                biased;
                _ = self.close.cancelled() => return Ok(Driver::Close),
                result = &mut future => return Ok(Driver::Done(result?)),
                cmd = self.commands.recv() => {
                    tokio::select! {
                        biased;
                        _ = self.close.cancelled() => return Ok(Driver::Close),
                        result = self.command(cmd, writer) => result?,
                    }
                }
            }
        }
    }
}

struct WriteBudget<'a> {
    transport: &'a SshTransport,
    bytes: usize,
}
impl Drop for WriteBudget<'_> {
    fn drop(&mut self) {
        self.transport.release_write_bytes(self.bytes);
    }
}

async fn read_loop(
    reader: &mut ChannelReadHalf,
    writer: &ChannelWriteHalf<client::Msg>,
    pump: &mut AsyncGhosttyPump<SshTransport>,
    io: &OwnerIo,
) -> Result<Report, Error> {
    let mut exit_code = None;
    let mut lifecycle = LifecycleDelivery::Complete;
    let stop = loop {
        tokio::select! {
            biased;
            _ = io.close.cancelled() => break Stop::Requested,
            input = io.inputs.recv() => {
                // Never dispatch recursively from drive: an active pump batch
                // exclusively owns its native operation and reliable delivery.
                let Ok(input) = input else {
                    while let Ok(command) = io.commands.try_recv() {
                        tokio::select! {
                            biased;
                            _ = io.close.cancelled() => break,
                            result = io.command(Ok(command), writer) => result?,
                        }
                    }
                    break Stop::Requested;
                };
                let result = match input {
                    Input::Key(key) => io.drive(pump.key(key), writer).await?,
                    Input::Raw(bytes, reply, reservation) => io.raw_input(pump, writer, bytes, reply, reservation).await?,
                    Input::KeyReceipt(key, reply) => io.key_receipt(pump, writer, key, reply).await?,
                    Input::Mouse(mouse) => io.drive(pump.mouse(mouse), writer).await?,
                    Input::Wheel(wheel) => io.drive(pump.wheel(wheel), writer).await?,
                    Input::Resize(size) => io.resize(pump, writer, size).await?,
                    Input::Paste(paste) => io.paste(pump, writer, paste).await?,
                    Input::ClearCommand(reply) => io.clear_command(pump, writer, reply).await?,
                    Input::Selection(update, reply) => io.selection(pump, writer, update, reply).await?,
                    Input::Search(request, reply) => io.search(pump, writer, request, reply).await?,
                    Input::ScrollNow(scroll, reply) => io.scroll_now(pump, writer, scroll, reply).await?,
                    Input::DefaultColors(colors, reply) => io.default_colors(pump, writer, colors, reply).await?,
                    Input::Scroll(scroll) => match io.drive(pump.scroll_viewport(scroll), writer).await? {
                        Driver::Done(_) => Driver::Done(()),
                        Driver::Close => Driver::Close,
                    },
                };
                if matches!(result, Driver::Close) {
                    break Stop::Requested;
                }
            }
            cmd = io.commands.recv() => {
                tokio::select! {
                    biased;
                    _ = io.close.cancelled() => break Stop::Requested,
                    result = io.command(cmd, writer) => result?,
                }
            }
            msg = reader.wait() => match msg {
                Some(ChannelMsg::Data { data }) => {
                    io.state.add_rx_bytes(data.len() as u64);
                    if matches!(io.drive(pump.process_chunk(&data), writer).await?, Driver::Close) {
                        break Stop::Requested;
                    }
                    if matches!(io.drive(pump.finish_batch(), writer).await?, Driver::Close) {
                        break Stop::Requested;
                    }
                }
                Some(ChannelMsg::ExitStatus { exit_status }) => {
                    exit_code = Some(exit_status as i32);
                    if matches!(io.drive(pump.publish_exit(exit_code), writer).await?, Driver::Close) {
                        lifecycle = LifecycleDelivery::Cancelled;
                        break Stop::Requested;
                    }
                }
                Some(ChannelMsg::ExitSignal { .. }) => {
                    exit_code = None;
                    if matches!(io.drive(pump.publish_exit(None), writer).await?, Driver::Close) {
                        lifecycle = LifecycleDelivery::Cancelled;
                        break Stop::Requested;
                    }
                }
                Some(ChannelMsg::Eof) => break Stop::Eof,
                Some(ChannelMsg::Close) => break Stop::PeerClosed,
                None => break Stop::Disconnected,
                Some(_) => {}
            }
        }
    };
    Ok(Report {
        stop,
        exit_code,
        lifecycle,
    })
}

async fn run(
    channel: russh::Channel<client::Msg>,
    mut pump: AsyncGhosttyPump<SshTransport>,
    io: OwnerIo,
) -> Result<Report, Error> {
    let (mut reader, writer) = channel.split();
    let result = read_loop(&mut reader, &writer, &mut pump, &io).await;
    // Close admission before draining so a racing endpoint write is rejected or
    // included in this final drain. Pump cancellation never replays any bytes.
    // shutdown signals transport/native admission synchronously, before its
    // future is polled. Do this before closing the receiver so normal teardown
    // cannot manufacture a transport-Closed error from its own wake marker.
    let native = pump.shutdown();
    io.commands.close();
    io.inputs.close();
    while io.inputs.try_recv().is_ok() {}
    while let Ok(command) = io.commands.try_recv() {
        if let Cmd::Write(bytes) = command {
            io.transport.release_write_bytes(bytes.len());
        }
    }
    io.state.set_alive(false);
    let physical = writer.close().await;
    let native = native.await;
    // Final UI backpressure cannot retain native handles or live channel I/O.
    // Attempt Closed on every path, but preserve the original failure first.
    let closed = io.publish_closed().await;
    let mut report = result?;
    physical?;
    match native {
        // Explicit close while a pump operation is pending intentionally aborts
        // delivery. Await destruction, but do not misreport it as an I/O fault.
        Err(AsyncGhosttyPumpError::Engine(
            oneterm_terminal::ghostty_bridge::Error::AbandonedDelivery,
        )) if report.stop == Stop::Requested => {}
        other => {
            other?;
        }
    }
    if closed? == LifecycleDelivery::Cancelled {
        report.lifecycle = LifecycleDelivery::Cancelled;
    }
    Ok(report)
}

#[cfg(test)]
#[path = "ghostty_channel_tests.rs"]
mod tests;
