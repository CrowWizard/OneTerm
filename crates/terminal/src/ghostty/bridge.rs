//! Bounded asynchronous access to the blocking owner, not a transport pump.
//! Feed replies must be delivered before requesting capture. No native handle
//! or blocking join appears in Future::poll or the shared model's frame reads.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard};

use async_channel::{Receiver, Sender, TrySendError};

use crate::ghostty::{self, anchors, input, owner, search, state};
use crate::ghostty_projection::ProjectedFrame;

pub const COMMAND_CAPACITY: usize = 8;

#[derive(Debug, Clone)]
pub enum Error {
    Busy,
    Closed,
    Owner(Arc<owner::Error>),
    Spawn(Arc<std::io::Error>),
    Projection(ghostty::Error),
    AbandonedDelivery,
    Panicked,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => f.write_str("Ghostty command queue is full"),
            Self::Closed => f.write_str("Ghostty asynchronous endpoint is closed"),
            Self::Owner(error) => error.fmt(f),
            Self::Spawn(error) => write!(f, "Could not start the Ghostty dispatcher: {error}"),
            Self::Projection(error) => error.fmt(f),
            Self::AbandonedDelivery => {
                f.write_str("Ghostty reply or reliable event delivery was abandoned")
            }
            Self::Panicked => f.write_str("Ghostty dispatcher panicked"),
        }
    }
}

impl std::error::Error for Error {}

impl From<owner::Error> for Error {
    fn from(error: owner::Error) -> Self {
        Self::Owner(Arc::new(error))
    }
}

#[derive(Debug, Clone)]
pub enum State {
    Starting,
    Running,
    Closed,
    Failed(Error),
}

pub(crate) struct Publication {
    pub(crate) grid_revision: Arc<()>,
    pub frame: Option<Arc<ProjectedFrame>>,
    pub state: State,
}

pub(crate) struct Shared(RwLock<Publication>);

impl Shared {
    pub fn read(&self) -> RwLockReadGuard<'_, Publication> {
        self.0.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn state(&self, state: State) {
        let mut publication = self.0.write().unwrap_or_else(PoisonError::into_inner);
        match (&publication.state, &state) {
            // A native operation already in progress may reveal the causal
            // failure after cancellation. Do not replace it with teardown noise.
            (State::Failed(Error::AbandonedDelivery), State::Failed(error))
                if !matches!(error, Error::AbandonedDelivery) => {}
            (State::Failed(_), _) => return,
            _ => {}
        }
        publication.state = state;
    }

    fn publish(&self, frame: Arc<ProjectedFrame>) -> Result<(), Error> {
        let mut publication = self.0.write().unwrap_or_else(PoisonError::into_inner);
        match &publication.state {
            State::Running => {}
            State::Failed(error) => return Err(error.clone()),
            _ => return Err(Error::Closed),
        }
        let old = publication.frame.replace(frame);
        drop(publication);
        drop(old);
        Ok(())
    }
}

pub enum Command {
    Feed(Vec<u8>),
    Resize {
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    },
    Capture,
    Events,
    Key(input::KeyEvent),
    Mouse(input::MouseEvent),
    Wheel(ghostty::wheel::WheelEvent),
    PasteMode,
    DefaultColors(Box<ghostty::colors::DefaultColors>),
    Scroll(state::Scroll),
    CreateAnchor(anchors::Point),
    ResolveAnchor(anchors::AnchorId, anchors::Space),
    ReleaseAnchor(anchors::AnchorId),
    SetSelection(anchors::AnchorId, anchors::AnchorId, bool),
    Gesture(ghostty::selection::Gesture),
    ClearSelection,
    SelectAll,
    SelectionText,
    Search(String, search::Options),
    SearchStep,
    SearchNavigate(search::Navigation),
    #[cfg(test)]
    MalformedCapture,
    #[cfg(test)]
    Panic,
    #[cfg(test)]
    Wait(
        std::sync::mpsc::SyncSender<()>,
        std::sync::mpsc::Receiver<()>,
    ),
}

pub enum Reply {
    Wheel(ghostty::wheel::Outcome),
    Done,
    PasteMode(bool),
    Bytes(Vec<u8>),
    Events(Vec<ghostty::events::Event>),
    Frame(Arc<ProjectedFrame>),
    Anchor(anchors::AnchorId),
    Position(Option<anchors::Position>),
    Text(Option<String>),
    Search(search::Snapshot),
}

pub struct Pending {
    response: Receiver<Result<Reply, Error>>,
    commands: Sender<Request>,
    abandoned: Arc<AtomicBool>,
}

impl Pending {
    pub async fn wait(self) -> Result<Reply, Error> {
        self.response.recv().await.map_err(|_| Error::Closed)?
    }
}

fn needs_delivery(reply: &Reply) -> bool {
    match reply {
        Reply::Wheel(ghostty::wheel::Outcome::Bytes(bytes)) => !bytes.is_empty(),
        Reply::Wheel(ghostty::wheel::Outcome::Scrolled) => true,
        Reply::Bytes(bytes) => !bytes.is_empty(),
        Reply::Events(events) => !events.is_empty(),
        Reply::Anchor(_) => true,
        _ => false,
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        // Close before draining: a concurrent sender can no longer successfully
        // enqueue a result between our drain and receiver destruction.
        self.response.close();
        if let Ok(Ok(reply)) = self.response.try_recv() {
            if needs_delivery(&reply) {
                self.abandoned.store(true, Ordering::Release);
                self.commands.close();
            }
        }
    }
}

struct Request {
    command: Command,
    reply: Sender<Result<Reply, Error>>,
}

#[derive(Clone)]
pub(crate) struct Client {
    commands: Sender<Request>,
    pub shared: Arc<Shared>,
    abandoned: Arc<AtomicBool>,
}

impl Client {
    pub fn request(&self, command: Command) -> Result<Pending, Error> {
        validate(&command)?;
        let (reply, response) = async_channel::bounded(1);
        match self.commands.try_send(Request { command, reply }) {
            Ok(()) => Ok(Pending {
                response,
                commands: self.commands.clone(),
                abandoned: self.abandoned.clone(),
            }),
            Err(TrySendError::Full(_)) => Err(Error::Busy),
            Err(TrySendError::Closed(_)) => Err(Error::Closed),
        }
    }
}

/// Backend endpoint. Dropping it initiates shutdown even if model clones remain.
pub struct AsyncEngine {
    client: Client,
    closed: Receiver<()>,
}

impl AsyncEngine {
    /// Spawns without waiting for native initialization. State starts at Starting;
    /// queued commands run after initialization, or return its explicit failure.
    pub fn spawn(cols: u16, rows: u16) -> Result<Self, Error> {
        Self::spawn_with_scrollback(cols, rows, None)
    }

    pub fn spawn_with_scrollback(
        cols: u16,
        rows: u16,
        scrollback: Option<usize>,
    ) -> Result<Self, Error> {
        if cols == 0 || rows == 0 {
            return Err(owner::Error::Native(ghostty::Error::InvalidSize).into());
        }
        ghostty::render::check_budget(cols, rows)
            .map_err(|e| Error::from(owner::Error::Native(e)))?;
        let (commands, requests) = async_channel::bounded(COMMAND_CAPACITY);
        let (done, closed) = async_channel::bounded(1);
        let shared = Arc::new(Shared(RwLock::new(Publication {
            grid_revision: Arc::new(()),
            frame: None,
            state: State::Starting,
        })));
        let worker_shared = shared.clone();
        let abandoned = Arc::new(AtomicBool::new(false));
        let worker_abandoned = abandoned.clone();
        std::thread::Builder::new()
            .name("ghostty-dispatch".into())
            .spawn(move || {
                let result = match owner::EngineOwner::new_with_scrollback(cols, rows, scrollback) {
                    Ok(mut engine) => {
                        worker_shared.state(State::Running);
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            run(&mut engine, &requests, &worker_shared, &worker_abandoned)
                        }))
                        .unwrap_or(Err(Error::Panicked));
                        // Blocking native destruction/join stays on this dispatcher.
                        let close = engine.close().map_err(Error::from);
                        result.and(close)
                    }
                    Err(error) => Err(error.into()),
                };
                requests.close();
                worker_shared.state(match &result {
                    Ok(()) => State::Closed,
                    Err(error) => State::Failed(error.clone()),
                });
                while let Ok(request) = requests.try_recv() {
                    // A cancelled response is allowed; no command was executed here.
                    let _ = request.reply.try_send(Err(result
                        .as_ref()
                        .err()
                        .cloned()
                        .unwrap_or(Error::Closed)));
                }
                // Closing rather than sending broadcasts completion to every waiter.
                drop(done);
            })
            .map_err(|error| Error::Spawn(Arc::new(error)))?;
        Ok(Self {
            client: Client {
                commands,
                shared,
                abandoned,
            },
            closed,
        })
    }

    pub fn request(&self, command: Command) -> Result<Pending, Error> {
        self.client.request(command)
    }

    pub fn model(&self) -> crate::ghostty_model::SharedModel {
        crate::ghostty_model::SharedModel::new(self.client.clone())
    }

    /// Stop a cancelled transport operation without joining on its executor.
    /// Publication shares this lock, so no frame can commit after this returns.
    pub(crate) fn abandon_delivery(&self) {
        self.client.abandoned.store(true, Ordering::Release);
        self.client
            .shared
            .state(State::Failed(Error::AbandonedDelivery));
        self.client.commands.close();
    }

    /// Signals shutdown immediately, even before the returned future is polled.
    /// An already executing command completes; queued commands are rejected.
    pub fn close(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        self.client.commands.close();
        async move {
            // No value is sent; channel closure is the worker's join barrier.
            let _ = self.closed.recv().await;
            match &self.client.shared.read().state {
                State::Closed => Ok(()),
                State::Failed(error) => Err(error.clone()),
                _ => Err(Error::Closed),
            }
        }
    }
}

impl Drop for AsyncEngine {
    fn drop(&mut self) {
        self.client.commands.close();
    }
}

fn validate(command: &Command) -> Result<(), Error> {
    let native = match command {
        Command::Feed(bytes) if bytes.len() > owner::MAX_FEED_BYTES => {
            return Err(owner::Error::InputLimit.into());
        }
        Command::Resize { cols, rows, .. } => {
            if *cols == 0 || *rows == 0 {
                Err(ghostty::Error::InvalidSize)
            } else {
                ghostty::render::check_budget(*cols, *rows)
            }
        }
        Command::Key(event) => event.validate(),
        Command::Mouse(event) => event.validate(),
        Command::Wheel(event) => event.validate(),
        Command::Search(needle, options) => {
            return search::validate(needle, *options).map_err(|e| owner::Error::Search(e).into());
        }
        _ => Ok(()),
    };
    native.map_err(|e| owner::Error::Native(e).into())
}

fn recoverable(error: &Error) -> bool {
    matches!(error, Error::Owner(error) if matches!(error.as_ref(), owner::Error::Anchor(_) | owner::Error::Selection(_) | owner::Error::Search(_)))
}

fn run(
    engine: &mut owner::EngineOwner,
    requests: &Receiver<Request>,
    shared: &Shared,
    abandoned: &AtomicBool,
) -> Result<(), Error> {
    while let Ok(request) = requests.recv_blocking() {
        if abandoned.load(Ordering::Acquire) {
            return Err(Error::AbandonedDelivery);
        }
        if requests.is_closed() {
            // Out-of-band close cancels queued work, without replaying output.
            let _ = request.reply.try_send(Err(Error::Closed));
            break;
        }
        let result = execute(engine, request.command, shared);
        let fatal = result
            .as_ref()
            .err()
            .filter(|error| !recoverable(error))
            .cloned();
        if let Some(error) = &fatal {
            shared.state(State::Failed(error.clone()));
        }
        if let Err(TrySendError::Closed(result) | TrySendError::Full(result)) =
            request.reply.try_send(result)
        {
            // Cancelled interactions are at-most-once. Never silently discard
            // a transport reply or reliable event that still needs delivery.
            match result {
                Ok(Reply::Bytes(bytes)) if !bytes.is_empty() => {
                    return Err(Error::AbandonedDelivery);
                }
                Ok(Reply::Events(events)) if !events.is_empty() => {
                    return Err(Error::AbandonedDelivery);
                }
                Ok(Reply::Anchor(id)) => engine.release_anchor(&id)?,
                _ => {}
            }
        }
        if let Some(error) = fatal {
            return Err(error);
        }
    }
    if abandoned.load(Ordering::Acquire) {
        Err(Error::AbandonedDelivery)
    } else {
        Ok(())
    }
}

fn execute(
    engine: &mut owner::EngineOwner,
    command: Command,
    shared: &Shared,
) -> Result<Reply, Error> {
    if let Command::Gesture(ref gesture) = command {
        use ghostty::selection::Gesture;
        let viewport = match gesture {
            Gesture::Begin { viewport, .. } | Gesture::Move { viewport, .. } => Some(viewport),
            Gesture::Finish(_) => None,
        };
        if let Some(viewport) = viewport {
            let publication = shared.read();
            if !Arc::ptr_eq(&publication.grid_revision, &viewport.0.grid_revision) {
                return Err(
                    owner::Error::Selection(ghostty::selection::SelectionError::Expired).into(),
                );
            }
        }
    }
    if matches!(
        &command,
        Command::Feed(_) | Command::Resize { .. } | Command::Scroll(_)
    ) {
        shared
            .0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .grid_revision = Arc::new(());
    }
    Ok(match command {
        Command::Feed(bytes) => Reply::Bytes(engine.feed(&bytes)?),
        Command::Events => Reply::Events(engine.take_events()?),
        Command::Resize {
            cols,
            rows,
            cell_width_px,
            cell_height_px,
        } => Reply::Bytes(engine.resize(cols, rows, cell_width_px, cell_height_px)?),
        Command::Capture => {
            let mut frame = ProjectedFrame::new(engine.capture()?).map_err(Error::Projection)?;
            frame.grid_revision = shared.read().grid_revision.clone();
            let frame = Arc::new(frame);
            shared.publish(frame.clone())?;
            Reply::Frame(frame)
        }
        #[cfg(test)]
        Command::MalformedCapture => {
            let mut malformed = (*engine.capture()?).clone();
            malformed.rows[0].cells[0].style.flags = 0x8000;
            let frame =
                Arc::new(ProjectedFrame::new(Arc::new(malformed)).map_err(Error::Projection)?);
            shared.publish(frame.clone())?;
            Reply::Frame(frame)
        }
        #[cfg(test)]
        Command::Panic => panic!("dispatcher panic fixture"),
        Command::Key(event) => Reply::Bytes(engine.encode_key(event)?),
        Command::Mouse(event) => Reply::Bytes(engine.encode_mouse(event)?),
        Command::Wheel(event) => {
            let outcome = engine.wheel(event)?;
            if matches!(&outcome, ghostty::wheel::Outcome::Scrolled) {
                shared
                    .0
                    .write()
                    .unwrap_or_else(PoisonError::into_inner)
                    .grid_revision = Arc::new(());
            }
            Reply::Wheel(outcome)
        }
        Command::PasteMode => Reply::PasteMode(engine.bracketed_paste()?),
        Command::DefaultColors(colors) => {
            engine.set_default_colors(colors)?;
            Reply::Done
        }
        Command::Scroll(scroll) => {
            engine.scroll(scroll)?;
            Reply::Done
        }
        Command::SearchNavigate(navigation) => {
            let rows = {
                let publication = shared.read();
                if !Arc::ptr_eq(&publication.grid_revision, &navigation.grid_revision) {
                    return Err(owner::Error::Search(search::SearchError::Expired).into());
                }
                publication
                    .frame
                    .as_ref()
                    .map_or(0, |frame| frame.source().rows.len())
            };
            if rows == 0 {
                return Err(owner::Error::Search(search::SearchError::Expired).into());
            }
            shared
                .0
                .write()
                .unwrap_or_else(PoisonError::into_inner)
                .grid_revision = Arc::new(());
            engine.scroll(state::Scroll::Row(
                u64::from(navigation.row).saturating_sub(rows as u64 / 2),
            ))?;
            Reply::Done
        }
        Command::CreateAnchor(point) => Reply::Anchor(engine.create_anchor(point)?),
        Command::ResolveAnchor(id, space) => Reply::Position(engine.resolve_anchor(&id, space)?),
        Command::ReleaseAnchor(id) => {
            engine.release_anchor(&id)?;
            Reply::Done
        }
        Command::SetSelection(start, end, rectangle) => {
            engine.set_selection(&start, &end, rectangle)?;
            Reply::Done
        }
        Command::Gesture(gesture) => {
            engine.gesture(gesture)?;
            Reply::Done
        }
        Command::ClearSelection => {
            engine.clear_selection()?;
            Reply::Done
        }
        Command::SelectAll => {
            engine.select_all()?;
            Reply::Done
        }
        Command::SelectionText => Reply::Text(engine.selection_text()?),
        Command::Search(needle, options) => {
            let mut snapshot = engine.search(needle, options)?;
            snapshot.attach_revision(shared.read().grid_revision.clone());
            Reply::Search(snapshot)
        }
        Command::SearchStep => {
            let mut snapshot = engine.search_step()?;
            snapshot.attach_revision(shared.read().grid_revision.clone());
            Reply::Search(snapshot)
        }
        #[cfg(test)]
        Command::Wait(entered, release) => {
            entered.send(()).unwrap();
            release.recv().unwrap();
            Reply::Done
        }
    })
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
