//! Staged native-worker endpoint. Blocking methods belong on backend workers,
//! never GPUI or a tokio executor. Only owned data crosses the thread boundary.

use std::fmt;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::thread::{self, JoinHandle};

use super::render::{Frame, RenderBuffer, check_budget};
use super::{Error as NativeError, Terminal};

/// Per-request bound; the non-Clone endpoint permits only one outstanding call.
pub const MAX_FEED_BYTES: usize = 64 * 1024;

#[derive(Debug)]
pub enum Error {
    Native(NativeError),
    Anchor(super::anchors::AnchorError),
    Selection(super::selection::SelectionError),
    Search(super::search::SearchError),
    Spawn(io::Error),
    Closed,
    WorkerPanicked,
    Abandoned,
    InputLimit,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native(error) => error.fmt(f),
            Self::Anchor(error) => error.fmt(f),
            Self::Selection(error) => error.fmt(f),
            Self::Search(error) => error.fmt(f),
            Self::Spawn(error) => write!(f, "Could not start the Ghostty worker: {error}"),
            Self::Closed => f.write_str("Ghostty worker is closed"),
            Self::WorkerPanicked => f.write_str("Ghostty worker panicked"),
            Self::Abandoned => f.write_str("Ghostty worker response was abandoned"),
            Self::InputLimit => {
                f.write_str("Ghostty worker input exceeds the 64 KiB request limit")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Native(error) => Some(error),
            Self::Anchor(error) => Some(error),
            Self::Selection(error) => Some(error),
            Self::Search(error) => Some(error),
            Self::Spawn(error) => Some(error),
            _ => None,
        }
    }
}

impl From<NativeError> for Error {
    fn from(error: NativeError) -> Self {
        Self::Native(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    Native(NativeError),
    Panicked,
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Running,
    Closed,
    Failed(Failure),
}

impl State {
    fn error(&self) -> Error {
        match self {
            Self::Failed(Failure::Native(error)) => Error::Native(error.clone()),
            Self::Failed(Failure::Panicked) => Error::WorkerPanicked,
            Self::Failed(Failure::Abandoned) => Error::Abandoned,
            Self::Running | Self::Closed => Error::Closed,
        }
    }
}

struct Publication {
    frame: Option<Arc<Frame>>,
    state: State,
}

struct Shared(Mutex<Publication>);

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Publication> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn state(&self) -> State {
        self.lock().state.clone()
    }

    fn set_state(&self, state: State) {
        self.lock().state = state;
    }

    fn publish(&self, frame: Arc<Frame>) {
        // Drop potentially large old frames outside the pointer-only lock.
        let old = self.lock().frame.replace(frame);
        drop(old);
    }
}

/// Independently owned viewport damage; no terminal-global dirty flag is read.
#[derive(Debug, PartialEq, Eq)]
pub enum Damage {
    None,
    Full,
    Rows(Vec<u16>),
}

pub struct FrameUpdate {
    /// Last complete frame remains readable after closure/failure, with status.
    pub frame: Option<Arc<Frame>>,
    pub damage: Damage,
    pub state: State,
}

/// One consumer's watermark. Creating another never consumes this one's change.
pub struct FrameReader {
    shared: Arc<Shared>,
    previous: Option<Arc<Frame>>,
}

impl FrameReader {
    /// The latest publication only; no native work and no parse/capture lock.
    pub fn poll(&mut self) -> Result<FrameUpdate, NativeError> {
        self.poll_prepared(|_| Ok(()))
    }

    /// Prepare owned consumer data before acknowledging this capture. Failure
    /// preserves the watermark, so a failed projection cannot swallow damage.
    /// The callback runs without the publication lock or any native borrow.
    pub fn poll_prepared(
        &mut self,
        prepare: impl FnOnce(Option<&Arc<Frame>>) -> Result<(), NativeError>,
    ) -> Result<FrameUpdate, NativeError> {
        let (frame, state) = {
            let publication = self.shared.lock();
            (publication.frame.clone(), publication.state.clone())
        };
        let damage = match (&self.previous, &frame) {
            (_, None) => Damage::None,
            (None, Some(_)) => Damage::Full,
            (Some(before), Some(after)) if Arc::ptr_eq(before, after) => Damage::None,
            (Some(before), Some(after)) => frame_damage(before, after)?,
        };
        prepare(frame.as_ref())?;
        self.previous = frame.clone();
        Ok(FrameUpdate {
            frame,
            damage,
            state,
        })
    }
}

pub(crate) fn frame_damage(before: &Frame, after: &Frame) -> Result<Damage, NativeError> {
    if before.cols != after.cols
        || before.metadata.global_damage_changed(&after.metadata)
        || before.rows.len() != after.rows.len()
        || before.foreground != after.foreground
        || before.background != after.background
        || before.cursor_color != after.cursor_color
        || before.palette != after.palette
    {
        return Ok(Damage::Full);
    }
    let mut rows = Vec::new();
    rows.try_reserve_exact(after.rows.len())
        .map_err(|_| NativeError::Allocation)?;
    for (y, (old, new)) in before.rows.iter().zip(&after.rows).enumerate() {
        let cursor_changed = before.cursor != after.cursor
            && (before
                .cursor
                .position
                .is_some_and(|(_, row)| usize::from(row) == y)
                || after
                    .cursor
                    .position
                    .is_some_and(|(_, row)| usize::from(row) == y));
        if old != new || cursor_changed {
            rows.push(y as u16);
        }
    }
    Ok(if rows.is_empty() {
        Damage::None
    } else {
        Damage::Rows(rows)
    })
}

enum Operation {
    Search(super::search::Command),
    Selection(super::selection::Command),
    Anchor(super::anchors::Command),
    Key(super::input::KeyEvent),
    Mouse(super::input::MouseEvent),
    Wheel(super::wheel::WheelEvent),
    PasteMode,
    DefaultColors(Box<super::colors::DefaultColors>),
    Scroll(super::state::Scroll),
    Events,
    Feed(Vec<u8>),
    Resize {
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    },
    Capture,
    #[cfg(test)]
    Panic,
    #[cfg(test)]
    Wait {
        entered: mpsc::SyncSender<()>,
        release: mpsc::Receiver<()>,
    },
}

enum Response {
    Wheel(super::wheel::Outcome),
    Search(Result<super::search::Snapshot, super::search::SearchError>),
    Selection(Result<Option<String>, super::selection::SelectionError>),
    Anchor(Result<super::anchors::Reply, super::anchors::AnchorError>),
    PasteMode(bool),
    Events(Vec<super::events::Event>),
    Replies(Vec<u8>),
    Frame(Arc<Frame>),
}

struct Request {
    operation: Operation,
    reply: mpsc::SyncSender<Result<Response, NativeError>>,
}

/// Movable control endpoint, not a movable native handle. Requests are blocking
/// and serialized by mutable access; no clone permits an unbounded response queue.
pub struct EngineOwner {
    gesture: Option<(super::selection::GestureId, super::anchors::AnchorId)>,
    sender: Option<mpsc::SyncSender<Request>>,
    worker: Option<JoinHandle<()>>,
    shared: Arc<Shared>,
}

impl EngineOwner {
    /// Called only by the serialized dispatcher after painted-sample validation.
    pub(crate) fn gesture(&mut self, gesture: super::selection::Gesture) -> Result<(), Error> {
        use super::selection::{Gesture, SelectionError};
        match gesture {
            Gesture::Finish(id) => {
                if self
                    .gesture
                    .as_ref()
                    .is_some_and(|(current, _)| current.same(&id))
                {
                    let (_, start) = self.gesture.take().expect("checked gesture");
                    self.release_anchor(&start)?;
                }
                Ok(())
            }
            Gesture::Begin { id, coordinate, .. } => {
                if let Some((_, start)) = self.gesture.take() {
                    self.release_anchor(&start)?;
                }
                let start = self
                    .create_anchor(super::anchors::Point {
                        space: super::anchors::Space::Viewport,
                        coordinate,
                    })
                    .map_err(|error| match error {
                        Error::Anchor(error) => Error::Selection(SelectionError::Anchor(error)),
                        other => other,
                    })?;
                self.gesture = Some((id, start));
                self.clear_selection()
            }
            Gesture::Move { id, coordinate, .. } => {
                let start = self
                    .gesture
                    .as_ref()
                    .filter(|(current, _)| current.same(&id))
                    .map(|(_, start)| start.clone())
                    .ok_or(Error::Selection(SelectionError::Expired))?;
                let end = self
                    .create_anchor(super::anchors::Point {
                        space: super::anchors::Space::Viewport,
                        coordinate,
                    })
                    .map_err(|error| match error {
                        Error::Anchor(error) => Error::Selection(SelectionError::Anchor(error)),
                        other => other,
                    })?;
                let result = self.set_selection(&start, &end, false);
                self.release_anchor(&end)?;
                result
            }
        }
    }
    pub fn search(
        &mut self,
        needle: String,
        options: super::search::Options,
    ) -> Result<super::search::Snapshot, Error> {
        super::search::validate(&needle, options).map_err(Error::Search)?;
        self.search_request(super::search::Command::Set(needle))
    }

    pub fn search_step(&mut self) -> Result<super::search::Snapshot, Error> {
        self.search_request(super::search::Command::Step)
    }

    fn search_request(
        &mut self,
        command: super::search::Command,
    ) -> Result<super::search::Snapshot, Error> {
        match self.request(Operation::Search(command))? {
            Response::Search(result) => result.map_err(Error::Search),
            _ => unreachable!("search response has a private matching variant"),
        }
    }

    pub fn set_selection(
        &mut self,
        start: &super::anchors::AnchorId,
        end: &super::anchors::AnchorId,
        rectangle: bool,
    ) -> Result<(), Error> {
        self.selection_request(super::selection::Command::Set(
            start.clone(),
            end.clone(),
            rectangle,
        ))
        .map(|_| ())
    }

    pub fn clear_selection(&mut self) -> Result<(), Error> {
        self.selection_request(super::selection::Command::Clear)
            .map(|_| ())
    }

    pub fn select_all(&mut self) -> Result<(), Error> {
        self.selection_request(super::selection::Command::All)
            .map(|_| ())
    }

    pub fn selection_text(&mut self) -> Result<Option<String>, Error> {
        self.selection_request(super::selection::Command::Copy)
    }

    fn selection_request(
        &mut self,
        command: super::selection::Command,
    ) -> Result<Option<String>, Error> {
        match self.request(Operation::Selection(command))? {
            Response::Selection(result) => result.map_err(Error::Selection),
            _ => unreachable!("selection response has a private matching variant"),
        }
    }

    pub fn create_anchor(
        &mut self,
        point: super::anchors::Point,
    ) -> Result<super::anchors::AnchorId, Error> {
        match self.anchor_request(super::anchors::Command::Create(point))? {
            super::anchors::Reply::Created(id) => Ok(id),
            _ => unreachable!("create-anchor response has a private matching variant"),
        }
    }

    pub fn resolve_anchor(
        &mut self,
        id: &super::anchors::AnchorId,
        space: super::anchors::Space,
    ) -> Result<Option<super::anchors::Position>, Error> {
        match self.anchor_request(super::anchors::Command::Resolve(id.clone(), space))? {
            super::anchors::Reply::Resolved(position) => Ok(position),
            _ => unreachable!("resolve-anchor response has a private matching variant"),
        }
    }

    pub fn release_anchor(&mut self, id: &super::anchors::AnchorId) -> Result<(), Error> {
        match self.anchor_request(super::anchors::Command::Release(id.clone()))? {
            super::anchors::Reply::Released => Ok(()),
            _ => unreachable!("release-anchor response has a private matching variant"),
        }
    }

    fn anchor_request(
        &mut self,
        command: super::anchors::Command,
    ) -> Result<super::anchors::Reply, Error> {
        match self.request(Operation::Anchor(command))? {
            Response::Anchor(result) => result.map_err(Error::Anchor),
            _ => unreachable!("anchor response has a private matching variant"),
        }
    }

    /// Blocking native encoding, ordered with all output and viewport commands.
    pub fn encode_key(&mut self, key: super::input::KeyEvent) -> Result<Vec<u8>, Error> {
        key.validate()?;
        match self.request(Operation::Key(key))? {
            Response::Replies(bytes) => Ok(bytes),
            _ => unreachable!("key response has a private matching variant"),
        }
    }

    pub fn encode_mouse(&mut self, mouse: super::input::MouseEvent) -> Result<Vec<u8>, Error> {
        mouse.validate()?;
        match self.request(Operation::Mouse(mouse))? {
            Response::Replies(bytes) => Ok(bytes),
            _ => unreachable!("mouse response has a private matching variant"),
        }
    }

    pub fn bracketed_paste(&mut self) -> Result<bool, Error> {
        match self.request(Operation::PasteMode)? {
            Response::PasteMode(enabled) => Ok(enabled),
            _ => unreachable!("paste-mode response has a private matching variant"),
        }
    }

    pub fn wheel(
        &mut self,
        wheel: super::wheel::WheelEvent,
    ) -> Result<super::wheel::Outcome, Error> {
        wheel.validate()?;
        match self.request(Operation::Wheel(wheel))? {
            Response::Wheel(outcome) => Ok(outcome),
            _ => unreachable!("wheel response has a private matching variant"),
        }
    }

    /// Serialized defaults mutation only. The session pump controls publication.
    pub fn set_default_colors(
        &mut self,
        colors: Box<super::colors::DefaultColors>,
    ) -> Result<(), Error> {
        match self.request(Operation::DefaultColors(colors))? {
            Response::Replies(_) => Ok(()),
            _ => unreachable!("default-colors response has a private matching variant"),
        }
    }
    /// Serialized viewport mutation. Publication remains explicit, so callers
    /// can preserve their reliable-event-before-repaint ordering.
    pub fn scroll(&mut self, scroll: super::state::Scroll) -> Result<(), Error> {
        match self.request(Operation::Scroll(scroll))? {
            Response::Replies(_) => Ok(()),
            _ => unreachable!("scroll response has a private matching variant"),
        }
    }
    /// Handshake completes only after native initialization on the owner thread.
    pub fn new(cols: u16, rows: u16) -> Result<Self, Error> {
        Self::new_with_scrollback(cols, rows, None)
    }

    pub fn new_with_scrollback(
        cols: u16,
        rows: u16,
        scrollback: Option<usize>,
    ) -> Result<Self, Error> {
        validate_size(cols, rows)?;
        let shared = Arc::new(Shared(Mutex::new(Publication {
            frame: None,
            state: State::Running,
        })));
        let (sender, requests) = mpsc::sync_channel(1);
        let (ready, initialized) = mpsc::sync_channel(1);
        let thread_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("ghostty-vt".into())
            .spawn(move || {
                // This catches Rust worker panics, not native faults. Native owners
                // are created inside run and unwind/drop on this same thread.
                let outcome = std::panic::catch_unwind(|| {
                    run(cols, rows, scrollback, requests, ready, &thread_shared)
                });
                thread_shared.set_state(match outcome {
                    Ok(state) => state,
                    Err(_) => State::Failed(Failure::Panicked),
                });
            })
            .map_err(Error::Spawn)?;
        let mut owner = Self {
            gesture: None,
            sender: Some(sender),
            worker: Some(worker),
            shared,
        };
        match initialized.recv() {
            Ok(Ok(())) => Ok(owner),
            Ok(Err(error)) => {
                owner.disconnect();
                owner.join()?;
                Err(Error::Native(error))
            }
            Err(_) => {
                owner.disconnect();
                owner.join()?;
                Err(owner.shared.state().error())
            }
        }
    }

    pub fn reader(&self) -> FrameReader {
        FrameReader {
            shared: Arc::clone(&self.shared),
            previous: None,
        }
    }

    /// Returns ordered replies for the originating transport. Deliver them
    /// before requesting capture or notifying the UI; never retry a failed feed.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Error> {
        if bytes.len() > MAX_FEED_BYTES {
            return Err(Error::InputLimit);
        }
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(bytes.len())
            .map_err(|_| NativeError::Allocation)?;
        owned.extend_from_slice(bytes);
        match self.request(Operation::Feed(owned))? {
            Response::Replies(bytes) => Ok(bytes),
            _ => unreachable!("feed response has a private matching variant"),
        }
    }

    /// Native resize only. The transport caller still owns PTY/channel resize.
    /// Deliver any returned in-band resize report before capture/notification.
    pub fn resize(
        &mut self,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result<Vec<u8>, Error> {
        validate_size(cols, rows)?;
        match self.request(Operation::Resize {
            cols,
            rows,
            cell_width_px,
            cell_height_px,
        })? {
            Response::Replies(bytes) => Ok(bytes),
            _ => unreachable!("resize response has a private matching variant"),
        }
    }

    /// Capture once on the worker and publish the same owned Arc to all readers.
    pub fn capture(&mut self) -> Result<Arc<Frame>, Error> {
        match self.request(Operation::Capture)? {
            Response::Frame(frame) => Ok(frame),
            _ => unreachable!("capture response has a private matching variant"),
        }
    }

    /// Drain owned callbacks before the next feed can exhaust the fixed arena.
    pub fn take_events(&mut self) -> Result<Vec<super::events::Event>, Error> {
        match self.request(Operation::Events)? {
            Response::Events(events) => Ok(events),
            _ => unreachable!("event response has a private matching variant"),
        }
    }

    /// Explicit worker-path shutdown: disconnect, free native handles and join.
    pub fn close(mut self) -> Result<(), Error> {
        self.disconnect();
        self.join()?;
        match self.shared.state() {
            State::Closed => Ok(()),
            other => Err(other.error()),
        }
    }

    fn request(&mut self, operation: Operation) -> Result<Response, Error> {
        let (reply, receiver) = mpsc::sync_channel(1);
        let Some(sender) = self.sender.as_ref() else {
            return Err(self.shared.state().error());
        };
        if sender.send(Request { operation, reply }).is_err() {
            return Err(self.disconnected_error());
        }
        match receiver.recv() {
            Ok(result) => result.map_err(Error::Native),
            Err(_) => Err(self.disconnected_error()),
        }
    }

    fn disconnected_error(&mut self) -> Error {
        // During unwinding the response sender drops before catch_unwind
        // publishes failure. Join on this already-blocking worker path so the
        // caller cannot mistake that window for an ordinary closed session.
        self.disconnect();
        if let Err(error) = self.join() {
            return error;
        }
        self.shared.state().error()
    }

    fn disconnect(&mut self) {
        // This endpoint owns the only sender: disconnect also wakes recv.
        self.sender.take();
    }

    fn join(&mut self) -> Result<(), Error> {
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| Error::WorkerPanicked)?;
        }
        Ok(())
    }
}

impl Drop for EngineOwner {
    fn drop(&mut self) {
        // Disconnect rather than enqueueing shutdown behind work. No join on
        // GPUI; dropping JoinHandle detaches while the worker frees its state.
        self.disconnect();
    }
}

fn validate_size(cols: u16, rows: u16) -> Result<(), NativeError> {
    if cols == 0 || rows == 0 {
        return Err(NativeError::InvalidSize);
    }
    check_budget(cols, rows)
}

fn run(
    cols: u16,
    rows: u16,
    scrollback: Option<usize>,
    requests: mpsc::Receiver<Request>,
    ready: mpsc::SyncSender<Result<(), NativeError>>,
    shared: &Shared,
) -> State {
    let initialized = Terminal::new(cols, rows).and_then(|mut terminal| {
        if let Some(lines) = scrollback {
            terminal.set_scrollback_limit(lines)?;
        }
        let search = super::search::Search::new(&mut terminal)?;
        Ok((terminal, RenderBuffer::new()?, search))
    });
    let (mut terminal, mut render, mut search) = match initialized {
        Ok(owners) => owners,
        Err(error) => {
            // Startup caller may have disappeared; no initialized owner is
            // exposed and failure still reaches the shared lifecycle status.
            let _ = ready.send(Err(error.clone()));
            return State::Failed(Failure::Native(error));
        }
    };
    if ready.send(Ok(())).is_err() {
        return State::Closed;
    }
    // Constructed here, and dropped before the terminal on this same worker.
    let mut anchors = super::anchors::Anchors::new();
    while let Ok(request) = requests.recv() {
        let result = match request.operation {
            Operation::Search(command) => {
                search.execute(&mut terminal, command).map(Response::Search)
            }
            Operation::Selection(command) => {
                super::selection::execute(&mut terminal, &anchors, command).map(Response::Selection)
            }
            Operation::Anchor(command) => anchors
                .execute(&mut terminal, command)
                .map(Response::Anchor),
            Operation::Key(key) => terminal.encode_key(&key).map(Response::Replies),
            Operation::Mouse(mouse) => terminal.encode_mouse(&mouse).map(Response::Replies),
            Operation::Wheel(wheel) => terminal.wheel(wheel).map(Response::Wheel),
            Operation::PasteMode => terminal
                .metadata()
                .map(|state| Response::PasteMode(state.modes.bracketed_paste)),
            Operation::DefaultColors(colors) => terminal
                .set_default_colors(&colors)
                .map(|()| Response::Replies(Vec::new())),
            Operation::Scroll(scroll) => terminal
                .scroll(scroll)
                .map(|()| Response::Replies(Vec::new())),
            Operation::Events => terminal.take_events().map(Response::Events),
            Operation::Feed(bytes) => terminal
                .write(&bytes)
                .and_then(|()| terminal.take_replies())
                .map(Response::Replies),
            Operation::Resize {
                cols,
                rows,
                cell_width_px,
                cell_height_px,
            } => terminal
                .resize(cols, rows, cell_width_px, cell_height_px)
                .and_then(|()| terminal.take_replies())
                .map(Response::Replies),
            Operation::Capture => render.capture(&mut terminal).map(|frame| {
                let frame = Arc::new(frame);
                shared.publish(Arc::clone(&frame));
                Response::Frame(frame)
            }),
            #[cfg(test)]
            Operation::Panic => panic!("native-worker panic fixture"),
            #[cfg(test)]
            Operation::Wait { entered, release } => {
                entered.send(()).unwrap();
                release.recv().unwrap();
                Ok(Response::Replies(Vec::new()))
            }
        };
        let failure = result.as_ref().err().cloned();
        if let Some(error) = &failure {
            shared.set_state(State::Failed(Failure::Native(error.clone())));
        }
        if request.reply.send(result).is_err() {
            // Preserve the causal native failure even if its recipient left.
            return State::Failed(match failure {
                Some(error) => Failure::Native(error),
                None => Failure::Abandoned,
            });
        }
        if let Some(error) = failure {
            return State::Failed(Failure::Native(error));
        }
    }
    State::Closed
}

#[cfg(test)]
#[path = "owner_tests.rs"]
mod owner_tests;
