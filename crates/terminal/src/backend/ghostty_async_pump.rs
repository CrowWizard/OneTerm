//! Async reply-first staging pump. Neither production backend selects this yet.

use std::fmt;
use std::sync::Arc;

use crate::ghostty::events::{Event, MAX_EVENT_BYTES, MAX_EVENTS};
use crate::ghostty::{colors::DefaultColors, input, owner, render, selection, state};
use crate::ghostty_bridge::{AsyncEngine, Command, Error as BridgeError, Reply};
use crate::ghostty_model::SharedModel;
use crate::ghostty_projection::ProjectedFrame;
use crate::paste::{PasteMode, PastePolicy, PasteResult, encode_paste};
use crate::session::PasteError;
use crate::session::{SessionEvent, TerminalError};

use super::{OscRouter, PtyTransport};

#[derive(Debug, Clone)]
pub enum Error {
    Engine(BridgeError),
    Transport(TerminalError),
    EventsClosed,
    Closed,
    Paste(PasteError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine(error) => error.fmt(f),
            Self::Transport(error) => write!(f, "Ghostty transport operation failed: {error}"),
            Self::EventsClosed => f.write_str("Ghostty reliable event channel is closed"),
            Self::Closed => f.write_str("Ghostty asynchronous pump is closed"),
            Self::Paste(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine(error) => Some(error),
            Self::Transport(error) => Some(error),
            Self::Paste(error) => Some(error),
            Self::EventsClosed | Self::Closed => None,
        }
    }
}

impl Error {
    /// Domain validation rejected a selection without mutating it. Session
    /// adapters acknowledge this error but must not tear down a healthy owner.
    pub fn is_selection_rejection(&self) -> bool {
        matches!(self, Self::Engine(BridgeError::Owner(error)) if matches!(error.as_ref(), owner::Error::Selection(_)))
    }

    pub fn is_search_rejection(&self) -> bool {
        matches!(self, Self::Engine(BridgeError::Owner(error)) if matches!(error.as_ref(), owner::Error::Search(_)))
    }

    pub fn is_search_expired(&self) -> bool {
        matches!(self, Self::Engine(BridgeError::Owner(error)) if matches!(error.as_ref(), owner::Error::Search(crate::ghostty::search::SearchError::Expired)))
    }
}

impl From<BridgeError> for Error {
    fn from(error: BridgeError) -> Self {
        Self::Engine(error)
    }
}

/// Only this backend endpoint can feed or publish. UI clones can interact with
/// the same owner but cannot bypass reply/event delivery with a capture request.
pub struct AsyncGhosttyPump<T: PtyTransport> {
    engine: AsyncEngine,
    router: OscRouter<T>,
    pending: Vec<Event>,
    pending_bytes: usize,
    open: bool,
}

/// Armed across every await that can follow consumed output. Dropping an active
/// future is fail-stop even if its native response contained no reply bytes.
struct Operation<'a, T: PtyTransport> {
    engine: &'a AsyncEngine,
    transport: &'a T,
    open: &'a mut bool,
    complete: bool,
}

impl<'a, T: PtyTransport> Operation<'a, T> {
    fn new(engine: &'a AsyncEngine, transport: &'a T, open: &'a mut bool) -> Result<Self, Error> {
        if !*open {
            return Err(Error::Closed);
        }
        Ok(Self {
            engine,
            transport,
            open,
            complete: false,
        })
    }

    async fn request(&self, command: Command) -> Result<Reply, Error> {
        Ok(self.engine.request(command)?.wait().await?)
    }

    fn deliver(&self, reply: Reply) -> Result<(), Error> {
        let Reply::Bytes(bytes) = reply else {
            unreachable!("byte command has matching reply")
        };
        if !bytes.is_empty() {
            self.transport.pty_write(&bytes).map_err(Error::Transport)?;
        }
        Ok(())
    }

    fn finish(mut self) {
        self.complete = true;
    }
}

impl<T: PtyTransport> Drop for Operation<'_, T> {
    fn drop(&mut self) {
        if !self.complete {
            self.engine.abandon_delivery();
            if *self.open {
                *self.open = false;
                if let Err(error) = self.transport.pty_close() {
                    log::warn!(
                        "Ghostty async pump transport close after failure/cancellation: {error}"
                    );
                }
            }
        }
    }
}

impl<T: PtyTransport> AsyncGhosttyPump<T> {
    pub fn spawn(router: OscRouter<T>, cols: u16, rows: u16) -> Result<Self, Error> {
        Self::spawn_with_scrollback(router, cols, rows, None)
    }

    pub fn spawn_with_scrollback(
        router: OscRouter<T>,
        cols: u16,
        rows: u16,
        scrollback: Option<usize>,
    ) -> Result<Self, Error> {
        Ok(Self {
            engine: AsyncEngine::spawn_with_scrollback(cols, rows, scrollback)?,
            router,
            pending: Vec::new(),
            pending_bytes: 0,
            open: true,
        })
    }

    pub fn model(&self) -> SharedModel {
        self.engine.model()
    }

    pub fn logging(&self) -> crate::logging::TerminalLogController {
        self.router.logging().clone()
    }

    /// Empty reads do no native work. Large reads drain replies between chunks.
    pub async fn process_chunk(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        self.router.logging().process(bytes);
        for chunk in bytes.chunks(owner::MAX_FEED_BYTES) {
            operation.deliver(operation.request(Command::Feed(chunk.to_vec())).await?)?;
            let Reply::Events(events) = operation.request(Command::Events).await? else {
                unreachable!("events command has matching reply")
            };
            let bytes: usize = events.iter().map(Event::text_len).sum();
            if events.len() > MAX_EVENTS - self.pending.len()
                || bytes > MAX_EVENT_BYTES - self.pending_bytes
            {
                return Err(native_error(crate::ghostty::Error::EventOverflow));
            }
            self.pending
                .try_reserve(events.len())
                .map_err(|_| native_error(crate::ghostty::Error::Allocation))?;
            self.pending.extend(events);
            self.pending_bytes += bytes;
        }
        operation.finish();
        Ok(())
    }

    pub async fn finish_batch(&mut self) -> Result<Arc<ProjectedFrame>, Error> {
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        flush_events(&self.router, &mut self.pending, &mut self.pending_bytes).await?;
        let Reply::Frame(frame) = operation.request(Command::Capture).await? else {
            unreachable!("capture command has matching reply")
        };
        if self.router.state().claim_repaint_hint() && !self.router.events().post_repaint() {
            self.router.state().release_repaint_hint();
        }
        operation.finish();
        Ok(frame)
    }

    pub async fn resize(
        &mut self,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result<(), Error> {
        self.ensure_open()?;
        if cols == 0 || rows == 0 {
            return Err(native_error(crate::ghostty::Error::InvalidSize));
        }
        render::check_budget(cols, rows).map_err(native_error)?;
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        let reply = operation
            .request(Command::Resize {
                cols,
                rows,
                cell_width_px,
                cell_height_px,
            })
            .await?;
        operation
            .transport
            .pty_resize(rows, cols)
            .map_err(Error::Transport)?;
        operation.deliver(reply)?;
        operation.finish();
        Ok(())
    }

    pub async fn key(&mut self, key: input::KeyEvent) -> Result<(), Error> {
        self.ensure_open()?;
        key.validate().map_err(native_error)?;
        self.input(Command::Key(key)).await
    }

    /// Return exactly the bytes accepted by the original FIFO, not peer receipt.
    pub async fn key_receipt(&mut self, key: input::KeyEvent) -> Result<Vec<u8>, Error> {
        self.ensure_open()?;
        key.validate().map_err(native_error)?;
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        let Reply::Bytes(bytes) = operation.request(Command::Key(key)).await? else {
            unreachable!("key request returns encoded bytes")
        };
        if !bytes.is_empty() {
            operation
                .transport
                .pty_write(&bytes)
                .map_err(Error::Transport)?;
        }
        operation.finish();
        Ok(bytes)
    }

    pub async fn mouse(&mut self, mouse: input::MouseEvent) -> Result<(), Error> {
        self.ensure_open()?;
        mouse.validate().map_err(native_error)?;
        self.input(Command::Mouse(mouse)).await
    }

    async fn input(&mut self, command: Command) -> Result<(), Error> {
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        operation.deliver(operation.request(command).await?)?;
        operation.finish();
        Ok(())
    }

    pub async fn wheel(&mut self, wheel: crate::ghostty::wheel::WheelEvent) -> Result<(), Error> {
        use crate::ghostty::wheel::Outcome;
        self.ensure_open()?;
        wheel.validate().map_err(native_error)?;
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        flush_events(&self.router, &mut self.pending, &mut self.pending_bytes).await?;
        let Reply::Wheel(outcome) = operation.request(Command::Wheel(wheel)).await? else {
            unreachable!("wheel request returns a typed route outcome")
        };
        match outcome {
            Outcome::Bytes(bytes) => operation.deliver(Reply::Bytes(bytes))?,
            Outcome::Scrolled => {
                let Reply::Frame(_) = operation.request(Command::Capture).await? else {
                    unreachable!("capture request returns a frame")
                };
                if self.router.state().claim_repaint_hint() && !self.router.events().post_repaint()
                {
                    self.router.state().release_repaint_hint();
                }
            }
            Outcome::Ignored => {}
        }
        operation.finish();
        Ok(())
    }

    /// Validate before native/control admission without encoding or allocating.
    pub fn validate_paste(text: &str) -> Result<(), PasteError> {
        crate::ghostty_session::validate_paste(text)
    }

    /// Success acknowledges one original-FIFO enqueue, not physical receipt.
    /// Read the live native mode even if no frame has published it yet.
    pub async fn paste(&mut self, text: &str) -> Result<(), Error> {
        self.paste_request(text, false).await
    }

    /// Clipboard/editing route: policy rejection is side-effect free. Reveal
    /// and write remain one pump operation; raw IME input does not use this.
    pub async fn paste_visible(&mut self, text: &str) -> Result<(), Error> {
        self.paste_request(text, true).await
    }

    async fn paste_request(&mut self, text: &str, reveal: bool) -> Result<(), Error> {
        Self::validate_paste(text).map_err(Error::Paste)?;
        self.ensure_open()?;
        if text.is_empty() && reveal {
            return Ok(());
        }
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        let Reply::PasteMode(bracketed) = operation.request(Command::PasteMode).await? else {
            unreachable!("paste-mode command has matching reply")
        };
        let mode = if bracketed {
            PasteMode::Bracketed
        } else {
            PasteMode::Plain
        };
        let bytes = match encode_paste(text, mode, &PastePolicy::default()) {
            PasteResult::Ok(bytes) => bytes,
            PasteResult::TooLarge(_) => unreachable!("paste was validated before native lookup"),
        };
        if reveal && !bytes.is_empty() {
            flush_events(&self.router, &mut self.pending, &mut self.pending_bytes).await?;
            operation
                .request(Command::Scroll(state::Scroll::Bottom))
                .await?;
            let Reply::Frame(_) = operation.request(Command::Capture).await? else {
                unreachable!("capture command has matching reply")
            };
            if self.router.state().claim_repaint_hint() && !self.router.events().post_repaint() {
                self.router.state().release_repaint_hint();
            }
        }
        operation.deliver(Reply::Bytes(bytes))?;
        operation.finish();
        Ok(())
    }

    /// Reveal then admit exact bytes once; never apply clipboard transformations.
    pub async fn write_visible(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let max_bytes = PastePolicy::default().max_bytes;
        if bytes.len() > max_bytes {
            return Err(Error::Paste(PasteError::TooLarge {
                bytes: bytes.len(),
                max_bytes,
            }));
        }
        self.ensure_open()?;
        if bytes.is_empty() {
            return Ok(());
        }
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        flush_events(&self.router, &mut self.pending, &mut self.pending_bytes).await?;
        operation
            .request(Command::Scroll(state::Scroll::Bottom))
            .await?;
        let Reply::Frame(_) = operation.request(Command::Capture).await? else {
            unreachable!("capture command has matching reply")
        };
        if self.router.state().claim_repaint_hint() && !self.router.events().post_repaint() {
            self.router.state().release_repaint_hint();
        }
        operation
            .transport
            .pty_write(bytes)
            .map_err(Error::Transport)?;
        operation.finish();
        Ok(())
    }

    /// Shell command, never paste encoding or a local terminal reset.
    pub async fn clear_command(&mut self) -> Result<(), Error> {
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        flush_events(&self.router, &mut self.pending, &mut self.pending_bytes).await?;
        let delivery = operation.deliver(Reply::Bytes(b"clear\r".to_vec()));
        let publication = async {
            operation.request(Command::ClearSelection).await?;
            let Reply::Frame(_) = operation.request(Command::Capture).await? else {
                unreachable!("capture command has matching reply")
            };
            if self.router.state().claim_repaint_hint() && !self.router.events().post_repaint() {
                self.router.state().release_repaint_hint();
            }
            Ok::<(), Error>(())
        }
        .await;
        if let Err(error) = delivery {
            if let Err(failure) = publication {
                log::warn!("Native clear selection publication after transport failure: {failure}");
            }
            return Err(error);
        }
        publication?;
        operation.finish();
        Ok(())
    }

    pub async fn scroll(&mut self, scroll: state::Scroll) -> Result<(), Error> {
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        operation.request(Command::Scroll(scroll)).await?;
        operation.finish();
        Ok(())
    }

    /// Session viewport operation: flush prior reliable events before native
    /// mutation and publish/repaint without waiting for another output batch.
    pub async fn scroll_viewport(
        &mut self,
        scroll: state::Scroll,
    ) -> Result<Arc<ProjectedFrame>, Error> {
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        flush_events(&self.router, &mut self.pending, &mut self.pending_bytes).await?;
        operation.request(Command::Scroll(scroll)).await?;
        let Reply::Frame(frame) = operation.request(Command::Capture).await? else {
            unreachable!("capture command has matching reply")
        };
        if self.router.state().claim_repaint_hint() && !self.router.events().post_repaint() {
            self.router.state().release_repaint_hint();
        }
        operation.finish();
        Ok(frame)
    }

    /// Acknowledge a selected-cell publication without another output batch.
    /// Invalid anchors reject without capture or closing the healthy native owner.
    pub async fn update_selection(
        &mut self,
        update: selection::Update,
    ) -> Result<Arc<ProjectedFrame>, Error> {
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        flush_events(&self.router, &mut self.pending, &mut self.pending_bytes).await?;
        let command = match update {
            selection::Update::All => Command::SelectAll,
            selection::Update::Gesture(gesture) => Command::Gesture(gesture),
            selection::Update::Set {
                start,
                end,
                rectangle,
            } => Command::SetSelection(start, end, rectangle),
            selection::Update::Clear => Command::ClearSelection,
        };
        if let Err(error) = operation.request(command).await {
            if error.is_selection_rejection() {
                operation.finish();
            }
            return Err(error);
        }
        let Reply::Frame(frame) = operation.request(Command::Capture).await? else {
            unreachable!("capture command has matching reply")
        };
        if self.router.state().claim_repaint_hint() && !self.router.events().post_repaint() {
            self.router.state().release_repaint_hint();
        }
        operation.finish();
        Ok(frame)
    }

    /// Read native search progress after delivering preceding reliable events.
    /// Results carry the current opaque grid revision and are never captured as UI state.
    pub async fn search(
        &mut self,
        needle: String,
        options: crate::ghostty::search::Options,
    ) -> Result<crate::ghostty::search::Snapshot, Error> {
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        flush_events(&self.router, &mut self.pending, &mut self.pending_bytes).await?;
        let result = operation.request(Command::Search(needle, options)).await;
        if result.as_ref().is_err_and(Error::is_search_rejection) {
            operation.finish();
            return Err(result.err().expect("checked search rejection"));
        }
        let Reply::Search(snapshot) = result? else {
            unreachable!("search command has matching reply")
        };
        operation.finish();
        Ok(snapshot)
    }

    pub async fn search_step(&mut self) -> Result<crate::ghostty::search::Snapshot, Error> {
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        flush_events(&self.router, &mut self.pending, &mut self.pending_bytes).await?;
        let result = operation.request(Command::SearchStep).await;
        if result.as_ref().is_err_and(Error::is_search_rejection) {
            operation.finish();
            return Err(result.err().expect("checked search rejection"));
        }
        let Reply::Search(snapshot) = result? else {
            unreachable!("search-step command has matching reply")
        };
        operation.finish();
        Ok(snapshot)
    }

    pub async fn navigate_search(
        &mut self,
        navigation: crate::ghostty::search::Navigation,
    ) -> Result<crate::ghostty::search::Snapshot, Error> {
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        flush_events(&self.router, &mut self.pending, &mut self.pending_bytes).await?;
        if let Err(error) = operation.request(Command::SearchNavigate(navigation)).await {
            if error.is_search_rejection() {
                operation.finish();
            }
            return Err(error);
        }
        operation.request(Command::Capture).await?;
        let result = operation.request(Command::SearchStep).await;
        if result.as_ref().is_err_and(Error::is_search_rejection) {
            operation.finish();
            return Err(result.err().expect("checked search rejection"));
        }
        let Reply::Search(snapshot) = result? else {
            unreachable!("search-step reply")
        };
        if self.router.state().claim_repaint_hint() && !self.router.events().post_repaint() {
            self.router.state().release_repaint_hint();
        }
        operation.finish();
        Ok(snapshot)
    }

    /// Publish caller defaults without clearing live program color overrides.
    /// Success confirms capture, not paint. Partial setter failure fails closed.
    pub async fn set_default_colors(
        &mut self,
        colors: Box<DefaultColors>,
    ) -> Result<Arc<ProjectedFrame>, Error> {
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        flush_events(&self.router, &mut self.pending, &mut self.pending_bytes).await?;
        operation.request(Command::DefaultColors(colors)).await?;
        let Reply::Frame(frame) = operation.request(Command::Capture).await? else {
            unreachable!("capture command has matching reply")
        };
        if self.router.state().claim_repaint_hint() && !self.router.events().post_repaint() {
            self.router.state().release_repaint_hint();
        }
        operation.finish();
        Ok(frame)
    }

    /// Record observed process exit and deliver it after earlier reliable
    /// events. Does not capture or prevent a backend from reading late output.
    pub async fn publish_exit(&mut self, code: Option<i32>) -> Result<(), Error> {
        let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
        flush_events(&self.router, &mut self.pending, &mut self.pending_bytes).await?;
        self.router.state().record_exit(code);
        if !self
            .router
            .events()
            .send_checked(SessionEvent::Exited(code))
            .await
        {
            return Err(Error::EventsClosed);
        }
        operation.finish();
        Ok(())
    }

    /// Graceful, backpressured close. No capture or repaint is requested.
    /// Dropping its polled future fails closed rather than losing pending events.
    pub async fn close(&mut self) -> Result<(), Error> {
        if self.open {
            let operation = Operation::new(&self.engine, self.router.transport(), &mut self.open)?;
            flush_events(&self.router, &mut self.pending, &mut self.pending_bytes).await?;
            operation.finish();
        }
        self.shutdown().await
    }

    /// Immediate abort of an unfinished batch. Signals before the future is
    /// polled; awaiting it waits for native destruction, never joins on poll.
    pub fn shutdown(&mut self) -> impl Future<Output = Result<(), Error>> + '_ {
        if let Err(error) = self.router.logging().stop() {
            log::warn!("Native terminal logging final flush failed: {error}");
        }
        let transport = if self.open {
            self.open = false;
            self.router
                .transport()
                .pty_close()
                .map_err(Error::Transport)
        } else {
            Ok(())
        };
        self.pending.clear();
        self.pending_bytes = 0;
        let native = self.engine.close();
        async move {
            let native = native.await.map_err(Error::Engine);
            // Always wait for destruction, even when transport closure failed.
            transport.and(native)
        }
    }

    fn ensure_open(&self) -> Result<(), Error> {
        if self.open {
            Ok(())
        } else {
            Err(Error::Closed)
        }
    }
}

impl<T: PtyTransport> Drop for AsyncGhosttyPump<T> {
    fn drop(&mut self) {
        if let Err(error) = self.router.logging().stop() {
            log::warn!("Native terminal logging flush during drop failed: {error}");
        }
        if self.open {
            self.engine.abandon_delivery();
            if let Err(error) = self.router.transport().pty_close() {
                log::warn!("Ghostty async pump transport close during drop failed: {error}");
            }
        }
        // AsyncEngine Drop closes admission; its dispatcher performs native join.
    }
}

fn native_error(error: crate::ghostty::Error) -> Error {
    Error::Engine(owner::Error::Native(error).into())
}

async fn flush_events<T: PtyTransport>(
    router: &OscRouter<T>,
    pending: &mut Vec<Event>,
    bytes: &mut usize,
) -> Result<(), Error> {
    let mut routed = Vec::new();
    for event in pending.drain(..) {
        router.route_ghostty(event, &mut routed);
        for event in routed.drain(..) {
            if !router.events().send_checked(event).await {
                return Err(Error::EventsClosed);
            }
        }
    }
    *bytes = 0;
    Ok(())
}

#[cfg(test)]
#[path = "ghostty_async_pump_tests.rs"]
mod tests;
