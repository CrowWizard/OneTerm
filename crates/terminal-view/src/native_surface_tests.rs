use std::future::Future;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context as TaskContext, Poll, Waker};
use std::time::{Duration, Instant};

use gpui::{
    AppContext as _, BorrowAppContext as _, Bounds, EntityInputHandler as _, TestAppContext, px,
};
use oneterm_terminal::TerminalError;
use oneterm_terminal::ghostty_bridge::{AsyncEngine, Command, Reply};
use oneterm_terminal::ghostty_content::RenderSource;
use oneterm_terminal::ghostty_ime::Composition;
use oneterm_terminal::ghostty_model::SharedModel;
use oneterm_terminal::ghostty_session::Size;

use super::*;

pub(crate) fn complete<F: Future>(future: F) -> F::Output {
    let end = Instant::now() + Duration::from_secs(20);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(value) = future
            .as_mut()
            .poll(&mut TaskContext::from_waker(Waker::noop()))
        {
            return value;
        }
        assert!(Instant::now() < end, "native surface fixture deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
}

pub(super) fn call(engine: &AsyncEngine, command: Command) -> Reply {
    complete(engine.request(command).unwrap().wait()).unwrap()
}

#[gpui::test]
fn native_semantic_settings_follow_live_mode_and_shell_profile(cx: &mut TestAppContext) {
    use oneterm_core::config::ShellKind;
    use oneterm_highlight::ShellProfile;
    use oneterm_settings::SemanticHighlightingMode;
    let (surface, window, _, _, settings) = color_surface(cx, false);
    settings.update(window, |settings, cx| {
        settings.shell.kind = ShellKind::PowerShell;
        settings.semantic_highlighting = SemanticHighlightingMode::Auto;
        cx.notify();
    });
    window.run_until_parked();
    surface.read_with(window, |surface, _| {
        let state = surface.state.borrow();
        assert!(state.inputs.semantic.is_enabled());
        assert_eq!(state.inputs.semantic.profile(), ShellProfile::PowerShell);
    });
    settings.update(window, |settings, cx| {
        settings.shell.kind = ShellKind::Cmd;
        settings.semantic_highlighting = SemanticHighlightingMode::Off;
        cx.notify();
    });
    window.run_until_parked();
    surface.read_with(window, |surface, _| {
        let state = surface.state.borrow();
        assert!(!state.inputs.semantic.is_enabled());
        assert_eq!(state.inputs.semantic.profile(), ShellProfile::Cmd);
    });
}

#[gpui::test]
fn native_select_all_busy_rejection_and_pending_ack_survives_release(cx: &mut TestAppContext) {
    use oneterm_terminal::{TerminalLifecycle as _, ghostty_session::NativeSession as _};
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(
        &engine,
        Command::Feed(b"first\r\nsecond\r\nthird\r\nfourth".to_vec()),
    );
    call(&engine, Command::Capture);
    let mut endpoint = Endpoint::new(engine.model());
    endpoint.selection_engine = Some(Arc::new(engine));
    let endpoint = Arc::new(endpoint);
    let (release, gate) = async_channel::bounded(1);
    *endpoint.selection_gate.lock().unwrap() = Some(gate);
    let failures = Rc::new(RefCell::new(Vec::new()));
    let errors = failures.clone();
    let source = endpoint.clone();
    let (surface, window) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("monospace"),
            px(14.),
            move |error, _| errors.borrow_mut().push(error),
            window,
            cx,
        )
    });
    surface.update(window, |surface, cx| {
        surface.select_all(cx);
        surface.select_all(cx);
    });
    window.run_until_parked();
    assert!(surface.read_with(window, |surface, _| surface.edit.busy));
    assert_eq!(failures.borrow().len(), 1);
    assert_eq!(endpoint.selections.lock().unwrap().len(), 1);
    assert_eq!(endpoint.selection_completed.load(Ordering::Relaxed), 0);
    let weak = surface.downgrade();
    window.update(|window, _| window.remove_window());
    window.cx.update(|_| drop(surface));
    window.run_until_parked();
    assert!(weak.upgrade().is_none());
    release.send_blocking(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while endpoint.selection_completed.load(Ordering::Relaxed) != 1 {
        window.run_until_parked();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        complete(endpoint.model.selection_text())
            .unwrap()
            .as_deref(),
        Some("first\nsecond\nthird\nfourth")
    );
    endpoint.close().unwrap();
    drop(endpoint.take_events());
    window.run_until_parked();
    assert!(complete(endpoint.complete()).is_ok());
}

#[gpui::test]
fn native_clear_busy_rejection_and_pending_ack_survives_release(cx: &mut TestAppContext) {
    use oneterm_terminal::{TerminalLifecycle as _, ghostty_session::NativeSession as _};
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Feed(b"keep native history".to_vec()));
    call(&engine, Command::SelectAll);
    call(&engine, Command::Capture);
    let mut endpoint = Endpoint::new(engine.model());
    endpoint.selection_engine = Some(Arc::new(engine));
    let endpoint = Arc::new(endpoint);
    let (release, gate) = async_channel::bounded(1);
    *endpoint.selection_gate.lock().unwrap() = Some(gate);
    let failures = Rc::new(RefCell::new(Vec::new()));
    let errors = failures.clone();
    let source = endpoint.clone();
    let (surface, window) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("monospace"),
            px(14.),
            move |error, _| errors.borrow_mut().push(error),
            window,
            cx,
        )
    });
    surface.update(window, |surface, cx| {
        surface.clear_command(cx);
        surface.clear_command(cx);
    });
    window.run_until_parked();
    assert!(surface.read_with(window, |surface, _| surface.edit.busy));
    assert_eq!(failures.borrow().len(), 1);
    assert_eq!(endpoint.clear_calls.load(Ordering::Relaxed), 1);
    assert!(endpoint.pastes.lock().unwrap().is_empty());
    let weak = surface.downgrade();
    window.update(|window, _| window.remove_window());
    window.cx.update(|_| drop(surface));
    window.run_until_parked();
    assert!(weak.upgrade().is_none());
    release.send_blocking(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while endpoint.selection_completed.load(Ordering::Relaxed) != 1 {
        window.run_until_parked();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(complete(endpoint.model.selection_text()).unwrap(), None);
    endpoint.close().unwrap();
    assert!(matches!(
        complete(endpoint.clear_command()),
        Err(oneterm_terminal::backend::AsyncGhosttyPumpError::Transport(
            TerminalError::Closed
        ))
    ));
    drop(endpoint.take_events());
    window.run_until_parked();
    assert!(complete(endpoint.complete()).is_ok());
}

// No legacy TerminalSession implementation: a real native model plus observable
// input admission. Real local/SSH transport proofs live in their own crates.
pub(super) struct Endpoint {
    capabilities: oneterm_terminal::TerminalCapabilities,
    kind: oneterm_terminal::SessionKind,
    pub(super) channel_probe: ChannelProbe,
    completion_probe: Option<Arc<AtomicUsize>>,
    clear_calls: AtomicUsize,
    color_gate: Mutex<Option<async_channel::Receiver<()>>>,
    color_calls: Mutex<Vec<oneterm_terminal::ghostty::colors::DefaultColors>>,
    pub(super) color_completed: AtomicUsize,
    lifecycle_events: Mutex<Option<async_channel::Receiver<oneterm_terminal::SessionEvent>>>,
    lifecycle_sender: Mutex<Option<async_channel::Sender<oneterm_terminal::SessionEvent>>>,
    search_gate: Mutex<Option<async_channel::Receiver<()>>>,
    searches: Mutex<Vec<String>>,
    search_status: Mutex<Option<oneterm_terminal::ghostty::search::Status>>,
    search_steps: AtomicUsize,
    pub(super) selection_engine: Option<Arc<AsyncEngine>>,
    selections: Mutex<Vec<oneterm_terminal::ghostty::selection::Update>>,
    selection_completed: AtomicUsize,
    selection_gate: Mutex<Option<async_channel::Receiver<()>>>,
    model: SharedModel,
    pub(super) keys: Mutex<Vec<KeyEvent>>,
    mice: Mutex<Vec<oneterm_terminal::ghostty::input::MouseEvent>>,
    wheels: Mutex<Vec<oneterm_terminal::ghostty::wheel::WheelEvent>>,
    sizes: Mutex<Vec<Size>>,
    commits: Mutex<Vec<String>>,
    clipboard_replies: Arc<Mutex<Vec<String>>>,
    clipboard_failure: Arc<AtomicBool>,
    pastes: Mutex<Vec<String>>,
    composition: Composition,
    reject: AtomicBool,
    repaint_acks: AtomicUsize,
}

pub(crate) fn native_pane_fixture() -> (
    Arc<dyn oneterm_terminal::ghostty_session::NativeSession>,
    async_channel::Sender<oneterm_terminal::SessionEvent>,
) {
    native_pane_content_fixture(b"")
}

#[derive(Clone, Default)]
pub(crate) struct ChannelProbe {
    pub(crate) inputs: Arc<Mutex<Vec<(bool, Vec<u8>)>>>,
    pub(crate) gate: Arc<Mutex<Option<async_channel::Receiver<()>>>>,
    pub(crate) reject: Arc<AtomicBool>,
    pub(crate) completed: Arc<AtomicUsize>,
}

pub(crate) fn native_channel_fixture() -> (
    Arc<dyn oneterm_terminal::ghostty_session::NativeSession>,
    async_channel::Sender<oneterm_terminal::SessionEvent>,
    ChannelProbe,
) {
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Capture);
    let mut endpoint = Endpoint::new(engine.model());
    endpoint.selection_engine = Some(Arc::new(engine));
    let probe = endpoint.channel_probe.clone();
    endpoint.completion_probe = Some(probe.completed.clone());
    let sender = endpoint
        .lifecycle_sender
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone();
    (Arc::new(endpoint), sender, probe)
}

pub(crate) fn native_pane_content_fixture(
    content: &[u8],
) -> (
    Arc<dyn oneterm_terminal::ghostty_session::NativeSession>,
    async_channel::Sender<oneterm_terminal::SessionEvent>,
) {
    let (session, sender, _) = native_duplicate_fixture(content);
    (session, sender)
}

pub(crate) fn native_duplicate_fixture(
    content: &[u8],
) -> (
    Arc<dyn oneterm_terminal::ghostty_session::NativeSession>,
    async_channel::Sender<oneterm_terminal::SessionEvent>,
    Arc<AtomicUsize>,
) {
    native_capability_fixture(
        content,
        oneterm_terminal::SessionKind::Local,
        Default::default(),
    )
}

pub(crate) fn native_capability_fixture(
    content: &[u8],
    kind: oneterm_terminal::SessionKind,
    capabilities: oneterm_terminal::TerminalCapabilities,
) -> (
    Arc<dyn oneterm_terminal::ghostty_session::NativeSession>,
    async_channel::Sender<oneterm_terminal::SessionEvent>,
    Arc<AtomicUsize>,
) {
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    if !content.is_empty() {
        call(&engine, Command::Feed(content.to_vec()));
    }
    call(&engine, Command::Capture);
    let mut endpoint = Endpoint::new(engine.model());
    endpoint.kind = kind;
    endpoint.capabilities = capabilities;
    let completed = Arc::new(AtomicUsize::new(0));
    endpoint.completion_probe = Some(completed.clone());
    endpoint.selection_engine = Some(Arc::new(engine));
    let sender = endpoint
        .lifecycle_sender
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone();
    (Arc::new(endpoint), sender, completed)
}

pub(crate) fn native_effect_fixture() -> (
    Arc<dyn oneterm_terminal::ghostty_session::NativeSession>,
    async_channel::Sender<oneterm_terminal::SessionEvent>,
    Arc<Mutex<Vec<String>>>,
    Arc<AtomicBool>,
) {
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Capture);
    let mut endpoint = Endpoint::new(engine.model());
    endpoint.selection_engine = Some(Arc::new(engine));
    let sender = endpoint
        .lifecycle_sender
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone();
    let replies = endpoint.clipboard_replies.clone();
    let failure = endpoint.clipboard_failure.clone();
    (Arc::new(endpoint), sender, replies, failure)
}

impl Endpoint {
    pub(super) fn new(model: SharedModel) -> Self {
        let (sender, events) = async_channel::bounded(4);
        Self {
            capabilities: Default::default(),
            kind: oneterm_terminal::SessionKind::Local,
            channel_probe: ChannelProbe::default(),
            completion_probe: None,
            clear_calls: AtomicUsize::new(0),
            color_gate: Mutex::new(None),
            color_calls: Mutex::new(Vec::new()),
            color_completed: AtomicUsize::new(0),
            lifecycle_events: Mutex::new(Some(events)),
            lifecycle_sender: Mutex::new(Some(sender)),
            search_gate: Mutex::new(None),
            searches: Mutex::new(Vec::new()),
            search_status: Mutex::new(None),
            search_steps: AtomicUsize::new(0),
            selection_engine: None,
            selections: Mutex::new(Vec::new()),
            selection_completed: AtomicUsize::new(0),
            selection_gate: Mutex::new(None),
            model,
            keys: Mutex::new(Vec::new()),
            mice: Mutex::new(Vec::new()),
            wheels: Mutex::new(Vec::new()),
            sizes: Mutex::new(Vec::new()),
            commits: Mutex::new(Vec::new()),
            clipboard_replies: Arc::default(),
            clipboard_failure: Arc::default(),
            pastes: Mutex::new(Vec::new()),
            composition: Composition::default(),
            reject: AtomicBool::new(false),
            repaint_acks: AtomicUsize::new(0),
        }
    }
}

impl RenderSource for Endpoint {
    fn render_model(&self) -> SharedModel {
        self.model.clone()
    }
}

impl oneterm_terminal::TerminalLifecycle for Endpoint {
    fn take_events(&self) -> Option<async_channel::Receiver<oneterm_terminal::SessionEvent>> {
        self.lifecycle_events.lock().unwrap().take()
    }
    fn alive(&self) -> bool {
        !self.reject.load(Ordering::Relaxed)
    }
    fn close(&self) -> Result<(), TerminalError> {
        self.reject.store(true, Ordering::Relaxed);
        self.lifecycle_sender.lock().unwrap().take();
        Ok(())
    }
    fn kind(&self) -> oneterm_terminal::SessionKind {
        self.kind
    }
    fn title(&self) -> Option<String> {
        None
    }
    fn cwd(&self) -> Option<std::path::PathBuf> {
        None
    }
}

impl oneterm_terminal::ghostty_session::NativeSession for Endpoint {
    fn capabilities(&self) -> oneterm_terminal::TerminalCapabilities {
        self.capabilities.clone()
    }
    fn reply_clipboard(&self, text: &str) -> Result<(), TerminalError> {
        let _ = oneterm_terminal::ghostty_session::clipboard_reply(text)?;
        if self.reject.load(Ordering::Relaxed) || self.clipboard_failure.load(Ordering::Relaxed) {
            return Err(TerminalError::Transport("fixture closed".into()));
        }
        self.clipboard_replies.lock().unwrap().push(text.to_owned());
        Ok(())
    }

    fn complete(self: Arc<Self>) -> oneterm_terminal::ghostty_session::CompletionFuture {
        Box::pin(async move {
            let endpoint = Arc::try_unwrap(self).map_err(|_| {
                TerminalError::Transport("fixture session references remain".into())
            })?;
            if let Some(engine) = endpoint.selection_engine {
                let engine = Arc::try_unwrap(engine).map_err(|_| {
                    TerminalError::Transport("fixture engine references remain".into())
                })?;
                engine
                    .close()
                    .await
                    .map_err(|error| TerminalError::Transport(error.to_string()))?;
            }
            if let Some(probe) = endpoint.completion_probe {
                probe.fetch_add(1, Ordering::Relaxed);
            }
            Ok(oneterm_terminal::ghostty_session::Completion {
                exit_code: Some(0),
                lifecycle_delivered: true,
            })
        })
    }
}

#[gpui::test]
fn native_pane_real_engine_close_drains_events_and_observes_destruction(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Capture);
    let mut endpoint = Endpoint::new(engine.model());
    endpoint.selection_engine = Some(Arc::new(engine));
    let events = endpoint
        .lifecycle_sender
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone();
    let endpoint = Arc::new(endpoint);
    let weak = Arc::downgrade(&endpoint);
    let (pane, cx) = cx.add_window_view(move |window, cx| {
        crate::NativeTerminalPane::new(endpoint, gpui::font("monospace"), px(14.), window, cx)
            .unwrap()
    });
    let observation = pane.read_with(cx, |pane, _| pane.completion());
    // More than the bounded event capacity: a pane that never paints must drain.
    let producer = std::thread::spawn(move || {
        for index in 0..200 {
            if events
                .send_blocking(oneterm_terminal::SessionEvent::Title(format!(
                    "title-{index}"
                )))
                .is_err()
            {
                break;
            }
        }
    });
    pane.update(cx, |pane, cx| pane.shutdown(cx));
    let deadline = Instant::now() + Duration::from_secs(10);
    while observation.result().is_none() {
        cx.executor().advance_clock(Duration::from_millis(20));
        cx.run_until_parked();
        assert!(Instant::now() < deadline, "native pane completion deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
    producer.join().unwrap();
    assert!(observation.result().unwrap().unwrap().lifecycle_delivered);
    assert!(
        weak.upgrade().is_none(),
        "native owner must release all session controls"
    );
    pane.update(cx, |pane, cx| pane.shutdown(cx));
    assert!(
        observation.result().unwrap().is_ok(),
        "shutdown stays idempotent"
    );
}

impl SurfaceSession for Endpoint {
    fn key_receipt(
        &self,
        key: KeyEvent,
    ) -> oneterm_terminal::ghostty_session::KeyReceiptFuture<'_> {
        Box::pin(async move {
            self.key(key.clone())
                .map_err(|_| oneterm_terminal::backend::AsyncGhosttyPumpError::Closed)?;
            let engine = self
                .selection_engine
                .as_ref()
                .expect("channel key encoder fixture");
            let Reply::Bytes(bytes) = engine
                .request(Command::Key(key))
                .unwrap()
                .wait()
                .await
                .map_err(oneterm_terminal::backend::AsyncGhosttyPumpError::Engine)?
            else {
                panic!("key command must return bytes");
            };
            self.channel_probe
                .inputs
                .lock()
                .unwrap()
                .push((false, bytes.clone()));
            let gate = self.channel_probe.gate.lock().unwrap().take();
            if let Some(gate) = gate {
                gate.recv().await.unwrap();
            }
            if self.channel_probe.reject.load(Ordering::Relaxed) {
                return Err(oneterm_terminal::backend::AsyncGhosttyPumpError::Transport(
                    TerminalError::Closed,
                ));
            }
            Ok(bytes)
        })
    }
    fn write_visible<'a>(
        &'a self,
        bytes: &'a [u8],
    ) -> oneterm_terminal::ghostty_session::PasteFuture<'a> {
        Box::pin(async move {
            if self.reject.load(Ordering::Relaxed)
                || self.channel_probe.reject.load(Ordering::Relaxed)
            {
                return Err(oneterm_terminal::backend::AsyncGhosttyPumpError::Transport(
                    TerminalError::Closed,
                ));
            }
            self.channel_probe
                .inputs
                .lock()
                .unwrap()
                .push((false, bytes.to_vec()));
            let gate = self.channel_probe.gate.lock().unwrap().take();
            if let Some(gate) = gate {
                gate.recv().await.unwrap();
            }
            Ok(())
        })
    }
    fn clear_command(&self) -> oneterm_terminal::ghostty_session::PasteFuture<'_> {
        Box::pin(async {
            if self.reject.load(Ordering::Relaxed) {
                return Err(oneterm_terminal::backend::AsyncGhosttyPumpError::Transport(
                    TerminalError::Closed,
                ));
            }
            self.clear_calls.fetch_add(1, Ordering::Relaxed);
            self.selection(oneterm_terminal::ghostty::selection::Update::Clear)
                .await
        })
    }
    fn set_default_colors(
        &self,
        colors: oneterm_terminal::ghostty::colors::DefaultColors,
    ) -> oneterm_terminal::ghostty_session::ColorFuture<'_> {
        Box::pin(async move {
            use oneterm_terminal::backend::AsyncGhosttyPumpError;
            self.color_calls.lock().unwrap().push(colors.clone());
            let gate = self.color_gate.lock().unwrap().take();
            if let Some(gate) = gate {
                gate.recv().await.unwrap();
            }
            let engine = self
                .selection_engine
                .as_ref()
                .ok_or(AsyncGhosttyPumpError::Closed)?;
            engine
                .request(Command::DefaultColors(Box::new(colors)))
                .map_err(AsyncGhosttyPumpError::Engine)?
                .wait()
                .await
                .map_err(AsyncGhosttyPumpError::Engine)?;
            engine
                .request(Command::Capture)
                .map_err(AsyncGhosttyPumpError::Engine)?
                .wait()
                .await
                .map_err(AsyncGhosttyPumpError::Engine)?;
            self.color_completed.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
    }
    fn search(
        &self,
        needle: String,
        options: oneterm_terminal::ghostty::search::Options,
    ) -> oneterm_terminal::ghostty_session::SearchFuture<'_> {
        Box::pin(async move {
            self.searches.lock().unwrap().push(needle.clone());
            let gate = self.search_gate.lock().unwrap().take();
            if let Some(gate) = gate {
                gate.recv().await.unwrap();
            }
            let mut result = self
                .model
                .search(needle, options)
                .await
                .map_err(oneterm_terminal::backend::AsyncGhosttyPumpError::Engine)?;
            if let Some(status) = *self.search_status.lock().unwrap() {
                result.status = status;
            }
            Ok(result)
        })
    }
    fn search_step(&self) -> oneterm_terminal::ghostty_session::SearchFuture<'_> {
        Box::pin(async {
            self.search_steps.fetch_add(1, Ordering::Relaxed);
            let mut result = self
                .model
                .search_step()
                .await
                .map_err(oneterm_terminal::backend::AsyncGhosttyPumpError::Engine)?;
            if let Some(status) = *self.search_status.lock().unwrap() {
                result.status = status;
            }
            Ok(result)
        })
    }
    fn reveal_search(
        &self,
        navigation: oneterm_terminal::ghostty::search::Navigation,
    ) -> oneterm_terminal::ghostty_session::SearchFuture<'_> {
        Box::pin(async move {
            let engine = self
                .selection_engine
                .as_ref()
                .expect("navigation fixture owner");
            engine
                .request(Command::SearchNavigate(navigation))
                .unwrap()
                .wait()
                .await
                .map_err(oneterm_terminal::backend::AsyncGhosttyPumpError::Engine)?;
            engine
                .request(Command::Capture)
                .unwrap()
                .wait()
                .await
                .map_err(oneterm_terminal::backend::AsyncGhosttyPumpError::Engine)?;
            self.model
                .search_step()
                .await
                .map_err(oneterm_terminal::backend::AsyncGhosttyPumpError::Engine)
        })
    }
    fn scroll_viewport(
        &self,
        scroll: oneterm_terminal::ghostty::state::Scroll,
    ) -> oneterm_terminal::ghostty_session::PasteFuture<'_> {
        Box::pin(async move {
            self.model
                .scroll(scroll)
                .await
                .map_err(oneterm_terminal::backend::AsyncGhosttyPumpError::Engine)
        })
    }
    fn selection(
        &self,
        update: oneterm_terminal::ghostty::selection::Update,
    ) -> oneterm_terminal::ghostty_session::PasteFuture<'_> {
        Box::pin(async move {
            self.selections.lock().unwrap().push(update.clone());
            let gate = self.selection_gate.lock().unwrap().take();
            if let Some(gate) = gate {
                gate.recv().await.unwrap();
            }
            if let Some(engine) = &self.selection_engine {
                use oneterm_terminal::ghostty::selection::Update;
                let command = match update {
                    Update::All => Command::SelectAll,
                    Update::Gesture(gesture) => Command::Gesture(gesture),
                    Update::Set {
                        start,
                        end,
                        rectangle,
                    } => Command::SetSelection(start, end, rectangle),
                    Update::Clear => Command::ClearSelection,
                };
                engine
                    .request(command)
                    .map_err(oneterm_terminal::backend::AsyncGhosttyPumpError::Engine)?
                    .wait()
                    .await
                    .map_err(oneterm_terminal::backend::AsyncGhosttyPumpError::Engine)?;
                engine
                    .request(Command::Capture)
                    .map_err(oneterm_terminal::backend::AsyncGhosttyPumpError::Engine)?
                    .wait()
                    .await
                    .map_err(oneterm_terminal::backend::AsyncGhosttyPumpError::Engine)?;
            }
            self.selection_completed.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
    }
    fn paste_visible<'a>(
        &'a self,
        text: &'a str,
    ) -> oneterm_terminal::ghostty_session::PasteFuture<'a> {
        Box::pin(async move {
            if self.reject.load(Ordering::Relaxed)
                || self.channel_probe.reject.load(Ordering::Relaxed)
            {
                return Err(oneterm_terminal::backend::AsyncGhosttyPumpError::Transport(
                    TerminalError::Closed,
                ));
            }
            self.pastes.lock().unwrap().push(text.to_owned());
            self.channel_probe
                .inputs
                .lock()
                .unwrap()
                .push((true, text.as_bytes().to_vec()));
            let gate = self.channel_probe.gate.lock().unwrap().take();
            if let Some(gate) = gate {
                gate.recv().await.unwrap();
            }
            Ok(())
        })
    }
    fn wheel(&self, wheel: oneterm_terminal::ghostty::wheel::WheelEvent) -> Result<(), InputError> {
        wheel.validate().map_err(InputError::Invalid)?;
        if self.reject.load(Ordering::Relaxed) {
            return Err(InputError::Admission(TerminalError::Closed));
        }
        self.wheels.lock().unwrap().push(wheel);
        Ok(())
    }
    fn mouse(&self, mouse: oneterm_terminal::ghostty::input::MouseEvent) -> Result<(), InputError> {
        mouse.validate().map_err(InputError::Invalid)?;
        if self.reject.load(Ordering::Relaxed) {
            return Err(InputError::Admission(TerminalError::Closed));
        }
        self.mice.lock().unwrap().push(mouse);
        Ok(())
    }
    fn resize(&self, size: Size) -> Result<(), InputError> {
        self.sizes.lock().unwrap().push(size);
        if self.reject.load(Ordering::Relaxed) {
            Err(InputError::Admission(TerminalError::Closed))
        } else {
            Ok(())
        }
    }
    fn key(&self, key: KeyEvent) -> Result<(), InputError> {
        if self.reject.load(Ordering::Relaxed) {
            return Err(InputError::Admission(TerminalError::Closed));
        }
        key.validate().map_err(InputError::Invalid)?;
        self.keys.lock().unwrap().push(key);
        Ok(())
    }
    fn release_repaint_hint(&self) {
        self.repaint_acks.fetch_add(1, Ordering::Relaxed);
    }
    fn set_marked_text(&self, text: String) -> Result<(), oneterm_terminal::ghostty_ime::Error> {
        self.composition.set_marked_text(text)
    }
    fn marked_text(&self) -> Option<String> {
        self.composition.marked_text()
    }
    fn clear_marked_text(&self) {
        self.composition.clear_marked_text();
    }
    fn commit_text(&self, text: &str) -> Result<(), oneterm_terminal::ghostty_ime::Error> {
        self.composition.commit_text(text, |bytes| {
            self.commits
                .lock()
                .unwrap()
                .push(String::from_utf8(bytes.to_vec()).unwrap());
            Ok(())
        })
    }
}

pub(super) fn down(key: &str, held: bool) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(key).unwrap(),
        is_held: held,
        prefer_character_input: false,
    }
}

pub(super) fn color_surface(
    cx: &mut TestAppContext,
    gated: bool,
) -> (
    gpui::Entity<NativeTerminalSurface>,
    &mut gpui::VisualTestContext,
    Arc<Endpoint>,
    async_channel::Sender<()>,
    gpui::Entity<oneterm_settings::TerminalSettings>,
) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let settings = cx.new(|_| oneterm_settings::TerminalSettings::default());
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Capture);
    let mut endpoint = Endpoint::new(engine.model());
    endpoint.selection_engine = Some(Arc::new(engine));
    let (release, gate) = async_channel::bounded(1);
    if gated {
        *endpoint.color_gate.lock().unwrap() = Some(gate);
    }
    let endpoint = Arc::new(endpoint);
    let source = endpoint.clone();
    let (surface, window) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("monospace"),
            px(14.),
            |_, _| {},
            window,
            cx,
        )
    });
    surface.update(window, |surface, cx| {
        surface.bind_settings(settings.clone(), cx)
    });
    (surface, window, endpoint, release, settings)
}

pub(super) fn wait_colors(window: &mut gpui::VisualTestContext, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready() {
        window.run_until_parked();
        assert!(
            Instant::now() < deadline,
            "native color acknowledgment deadline"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    window.run_until_parked();
}

#[gpui::test]
fn native_colors_coalesce_and_preserve_osc_until_reset(cx: &mut TestAppContext) {
    use oneterm_terminal::ghostty::render::Rgb;
    let (surface, window, endpoint, release, settings) = color_surface(cx, true);
    wait_colors(window, || endpoint.color_calls.lock().unwrap().len() == 1);
    for value in [0x112233, 0x445566, 0x778899] {
        settings.update(window, |settings, cx| {
            settings.color_overrides.foreground = Some(gpui::rgb(value).into());
            settings.color_overrides.background = Some(gpui::rgb(0x123456).into());
            settings.color_overrides.ansi = vec![Some(gpui::rgb(0xabcdef).into()); 16];
            cx.notify();
        });
        window.run_until_parked();
    }
    assert_eq!(
        endpoint.color_calls.lock().unwrap().len(),
        1,
        "no overlapping admitted updates"
    );
    release.send_blocking(()).unwrap();
    wait_colors(window, || {
        endpoint.color_completed.load(Ordering::Relaxed) == 2
    });
    let colors = endpoint.color_calls.lock().unwrap().last().unwrap().clone();
    assert_eq!(
        colors.foreground,
        Rgb {
            r: 0x77,
            g: 0x88,
            b: 0x99
        }
    );
    assert_eq!(
        colors.palette[1],
        Rgb {
            r: 0xab,
            g: 0xcd,
            b: 0xef
        }
    );
    assert_eq!(colors.palette[16], Rgb { r: 0, g: 0, b: 0 });
    assert_eq!(
        colors.palette[231],
        Rgb {
            r: 255,
            g: 255,
            b: 255
        }
    );
    assert_eq!(
        colors.palette[255],
        Rgb {
            r: 238,
            g: 238,
            b: 238
        }
    );
    let engine = endpoint.selection_engine.as_ref().unwrap();
    call(
        engine,
        Command::Feed(b"\x1b]10;#aabbcc\x07\x1b]4;1;#010203\x07".to_vec()),
    );
    settings.update(window, |settings, cx| {
        settings.color_overrides.foreground = Some(gpui::rgb(0x334455).into());
        cx.notify();
    });
    wait_colors(window, || {
        endpoint.color_completed.load(Ordering::Relaxed) == 3
    });
    let viewport = endpoint.query().viewport.unwrap();
    assert_eq!(
        viewport.foreground(),
        Rgb {
            r: 170,
            g: 187,
            b: 204
        }
    );
    assert_eq!(viewport.palette()[1], Rgb { r: 1, g: 2, b: 3 });
    call(
        engine,
        Command::Feed(b"\x1b]110\x07\x1b]104;1\x07".to_vec()),
    );
    call(engine, Command::Capture);
    let viewport = endpoint.query().viewport.unwrap();
    assert_eq!(
        viewport.foreground(),
        Rgb {
            r: 0x33,
            g: 0x44,
            b: 0x55
        }
    );
    assert_eq!(viewport.palette()[1], colors.palette[1]);
    // Unchanged render refresh must not emit another native control.
    surface.update(window, |surface, cx| surface.refresh_colors(cx));
    window.run_until_parked();
    assert_eq!(endpoint.color_calls.lock().unwrap().len(), 3);
}

#[gpui::test]
fn native_colors_pending_ack_survives_surface_release(cx: &mut TestAppContext) {
    let (surface, window, endpoint, release, _settings) = color_surface(cx, true);
    wait_colors(window, || endpoint.color_calls.lock().unwrap().len() == 1);
    let weak = surface.downgrade();
    window.update(|window, _| window.remove_window());
    window.cx.update(|_| drop(surface));
    window.run_until_parked();
    assert!(weak.upgrade().is_none());
    assert_eq!(endpoint.color_completed.load(Ordering::Relaxed), 0);
    release.send_blocking(()).unwrap();
    wait_colors(window, || {
        endpoint.color_completed.load(Ordering::Relaxed) == 1
    });
    assert_eq!(endpoint.color_calls.lock().unwrap().len(), 1);
}

#[gpui::test]
fn native_colors_observe_active_theme_without_output(cx: &mut TestAppContext) {
    let (_surface, window, endpoint, _release, _settings) = color_surface(cx, false);
    wait_colors(window, || {
        endpoint.color_completed.load(Ordering::Relaxed) == 1
    });
    window.update(|_, cx| {
        cx.update_global::<gpui_component::Theme, _>(|theme, _| {
            theme.colors.background = gpui::rgb(0x243648).into();
        });
    });
    wait_colors(window, || {
        endpoint.color_completed.load(Ordering::Relaxed) == 2
    });
    assert_eq!(
        endpoint.query().viewport.unwrap().background(),
        oneterm_terminal::ghostty::render::Rgb {
            r: 0x24,
            g: 0x36,
            b: 0x48
        }
    );
}

#[gpui::test]
fn native_colors_ansi_only_theme_change_preserves_osc_and_settings(cx: &mut TestAppContext) {
    use oneterm_terminal::ghostty::render::Rgb;
    let (_surface, window, endpoint, _release, settings) = color_surface(cx, false);
    wait_colors(window, || {
        endpoint.color_completed.load(Ordering::Relaxed) == 1
    });
    let initial = endpoint.query().viewport.unwrap().palette().to_vec();
    let engine = endpoint.selection_engine.as_ref().unwrap();
    call(engine, Command::Feed(b"\x1b]4;4;#010203\x07".to_vec()));
    window.update(|_, cx| {
        cx.update_global::<gpui_component::Theme, _>(|theme, _| {
            theme.colors.blue = gpui::rgb(0x87afaf).into();
            theme.colors.green = gpui::rgb(0x7a875f).into();
        });
    });
    wait_colors(window, || {
        endpoint.color_completed.load(Ordering::Relaxed) == 2
    });
    let viewport = endpoint.query().viewport.unwrap();
    assert_eq!(viewport.palette()[4], Rgb { r: 1, g: 2, b: 3 });
    assert_eq!(
        viewport.palette()[2],
        Rgb {
            r: 0x7a,
            g: 0x87,
            b: 0x5f
        }
    );
    assert_eq!(&viewport.palette()[16..], &initial[16..]);
    settings.update(window, |settings, cx| {
        settings.color_overrides.ansi = vec![None; 16];
        settings.color_overrides.ansi[4] = Some(gpui::rgb(0xabcdef).into());
        cx.notify();
    });
    wait_colors(window, || {
        endpoint.color_completed.load(Ordering::Relaxed) == 3
    });
    assert_eq!(
        endpoint.query().viewport.unwrap().palette()[4],
        Rgb { r: 1, g: 2, b: 3 }
    );
    call(engine, Command::Feed(b"\x1b]104;4\x07".to_vec()));
    call(engine, Command::Capture);
    assert_eq!(
        endpoint.query().viewport.unwrap().palette()[4],
        Rgb {
            r: 0xab,
            g: 0xcd,
            b: 0xef
        }
    );
    settings.update(window, |settings, cx| {
        settings.color_overrides.ansi.clear();
        cx.notify();
    });
    wait_colors(window, || {
        endpoint.color_completed.load(Ordering::Relaxed) == 4
    });
    assert_eq!(
        endpoint.query().viewport.unwrap().palette()[4],
        Rgb {
            r: 0x87,
            g: 0xaf,
            b: 0xaf
        }
    );
    call(engine, Command::Feed(b"\x1b[38;2;17;34;51mX".to_vec()));
    call(engine, Command::Capture);
    let viewport = endpoint.query().viewport.unwrap();
    let row = viewport.row(0).unwrap();
    assert_eq!(
        row.style_of(&row.cells()[0]).fg,
        oneterm_terminal::Color::Rgb(oneterm_terminal::Rgb {
            r: 17,
            g: 34,
            b: 51
        })
    );
}

#[gpui::test]
fn native_colors_failure_is_reported_once_without_retry(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let settings = cx.new(|_| oneterm_settings::TerminalSettings::default());
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Capture);
    // No color-capable fixture owner: an explicit endpoint failure.
    let endpoint = Arc::new(Endpoint::new(engine.model()));
    let source = endpoint.clone();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let received = errors.clone();
    let (surface, window) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("monospace"),
            px(14.),
            move |error, _| received.borrow_mut().push(format!("{error:?}")),
            window,
            cx,
        )
    });
    surface.update(window, |surface, cx| {
        surface.bind_settings(settings.clone(), cx)
    });
    wait_colors(window, || !errors.borrow().is_empty());
    assert!(errors.borrow()[0].contains("Colors"));
    settings.update(window, |settings, cx| {
        settings.color_overrides.background = Some(gpui::rgb(0x123456).into());
        cx.notify();
    });
    window.run_until_parked();
    for _ in 0..5 {
        surface.update(window, |surface, cx| surface.refresh_colors(cx));
    }
    window.run_until_parked();
    assert_eq!(endpoint.color_calls.lock().unwrap().len(), 1);
    assert_eq!(errors.borrow().len(), 1);
}

#[gpui::test]
fn native_colors_pane_close_waits_for_pending_ack(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let settings = cx.new(|_| oneterm_settings::TerminalSettings::default());
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Capture);
    let mut endpoint = Endpoint::new(engine.model());
    endpoint.selection_engine = Some(Arc::new(engine));
    let (release, gate) = async_channel::bounded(1);
    *endpoint.color_gate.lock().unwrap() = Some(gate);
    let endpoint = Arc::new(endpoint);
    let weak = Arc::downgrade(&endpoint);
    let source = endpoint.clone();
    let (pane, window) = cx.add_window_view(move |window, cx| {
        crate::NativeTerminalPane::new(source, gpui::font("monospace"), px(14.), window, cx)
            .unwrap()
    });
    pane.update(window, |pane, cx| pane.bind_settings(settings, cx));
    wait_colors(window, || endpoint.color_calls.lock().unwrap().len() == 1);
    let completion = pane.read_with(window, |pane, _| pane.completion());
    pane.update(window, |pane, cx| pane.shutdown(cx));
    drop(endpoint);
    window.executor().advance_clock(Duration::from_millis(20));
    window.run_until_parked();
    assert!(
        completion.result().is_none(),
        "close admission is not color completion"
    );
    assert!(
        weak.upgrade().is_some(),
        "admitted color wait owns its session"
    );
    release.send_blocking(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while completion.result().is_none() {
        window.executor().advance_clock(Duration::from_millis(20));
        window.run_until_parked();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(completion.result().unwrap().unwrap().lifecycle_delivered);
    assert!(weak.upgrade().is_none());
}

#[gpui::test]
fn native_surface_bound_typography_updates_without_paint_or_output(cx: &mut TestAppContext) {
    use oneterm_settings::{TerminalPadding, TerminalSettings};

    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let settings = cx.new(|_| TerminalSettings {
        font_family: Some("Configured mono".into()),
        font_size: Some(19.),
        font_weight: gpui::FontWeight::BOLD,
        font_features: vec!["liga".into()],
        font_ligatures: false,
        font_fallbacks: vec!["Fallback mono".into()],
        line_height_factor: 1.6,
        cell_width: Some(11.),
        padding: TerminalPadding {
            top: 1.,
            right: 2.,
            bottom: 3.,
            left: 4.,
        },
        ..Default::default()
    });
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Capture);
    let source = Arc::new(Endpoint::new(engine.model()));
    let (surface, window) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("caller mono"),
            px(14.),
            |_, _| {},
            window,
            cx,
        )
    });
    surface.update(window, |surface, cx| {
        surface.bind_settings(settings.clone(), cx)
    });
    surface.read_with(window, |surface, cx| {
        let state = surface.state.borrow();
        assert_eq!(
            state.inputs.font,
            crate::terminal_view::terminal_font(settings.read(cx), &"Configured mono".into())
        );
        assert_eq!(state.inputs.font_size, px(19.));
        assert_eq!(state.inputs.line_height_factor, 1.6);
        assert_eq!(state.inputs.cell_width_override, Some(11.));
        assert_eq!(state.inputs.padding.left, px(4.));
        assert_eq!(state.inputs.padding.bottom, px(3.));
    });
    settings.update(window, |settings, cx| {
        settings.font_family = None;
        settings.font_size = None;
        settings.font_weight = gpui::FontWeight::NORMAL;
        settings.font_features = vec!["calt".into()];
        settings.font_fallbacks.clear();
        settings.line_height_factor = 1.3;
        settings.cell_width = None;
        settings.padding = TerminalPadding::default();
        cx.notify();
    });
    window.run_until_parked();
    // The settings observer refreshes even without terminal output or paint.
    surface.read_with(window, |surface, cx| {
        let state = surface.state.borrow();
        assert_eq!(
            state.inputs.font,
            crate::terminal_view::terminal_font(settings.read(cx), &cx.theme().mono_font_family)
        );
        assert_eq!(state.inputs.font_size, cx.theme().mono_font_size);
        assert_eq!(state.inputs.line_height_factor, 1.3);
        assert_eq!(state.inputs.cell_width_override, None);
        assert_eq!(state.inputs.padding.left, px(0.));
        assert_eq!(state.inputs.padding.top, px(0.));
    });
}

#[gpui::test]
fn native_surface_unbound_typography_preserves_caller_inputs(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Capture);
    let source = Arc::new(Endpoint::new(engine.model()));
    let (surface, window) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("caller mono"),
            px(23.),
            |_, _| {},
            window,
            cx,
        )
    });
    surface.update(window, |surface, cx| {
        surface.refresh_typography(cx);
        let state = surface.state.borrow();
        assert_eq!(state.inputs.font, gpui::font("caller mono"));
        assert_eq!(state.inputs.font_size, px(23.));
        assert!(!state.inputs.semantic.is_enabled());
    });
}

#[gpui::test]
fn native_surface_moves_through_the_shared_space_tree(cx: &mut TestAppContext) {
    use crate::space::{CloseOutcome, SpaceContent, SpaceLeaf, SpaceTree, SplitDir};

    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Capture);
    let source = Arc::new(Endpoint::new(engine.model()));
    let (surface, window) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("monospace"),
            px(14.),
            |error, _| panic!("unexpected native surface error: {error:?}"),
            window,
            cx,
        )
    });
    window.update(|_, cx| {
        let focus = surface.read(cx).focus_handle(cx);
        let mut tree = SpaceTree::new_terminal(surface.clone(), focus.clone());
        let original = tree.active();
        let destination = tree.alloc_id();
        let state = cx.new(|_| gpui_component::resizable::ResizableState::default());
        tree.split(
            original,
            SplitDir::Down,
            SpaceLeaf {
                id: destination,
                content: SpaceContent::Empty,
                focus,
            },
            state,
        );
        let moved = tree.take_leaf_terminal(original).unwrap();
        assert_eq!(moved.entity_id(), surface.entity_id());
        assert!(tree.fill_empty(destination, moved).is_ok());
        assert_eq!(tree.close(original), (CloseOutcome::Removed, None));
        assert_eq!(
            tree.active_terminal().unwrap().entity_id(),
            surface.entity_id()
        );
        assert_eq!(
            tree.active_focus_handle(|view| view.read(cx).focus_handle(cx)),
            Some(surface.read(cx).focus_handle(cx))
        );
        assert!(tree.is_single());
    });
}

#[gpui::test]
fn platform_input_handler_does_not_retain_a_closed_native_surface(cx: &mut TestAppContext) {
    use gpui::InputHandler as _;

    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Capture);
    let endpoint = Arc::new(Endpoint::new(engine.model()));
    let source = endpoint.clone();
    let (surface, window) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("monospace"),
            px(14.),
            |_, _| {},
            window,
            cx,
        )
    });
    let weak = surface.downgrade();
    let mut retained = input_handler::WeakInputHandler::new(weak.clone(), Bounds::default());
    // Alive callbacks use the same GPUI adapter semantics, including IME commit.
    window.update(|window, cx| {
        retained.replace_text_in_range(None, "IME界", window, cx);
    });
    assert_eq!(*endpoint.commits.lock().unwrap(), ["IME界"]);
    // Model a platform retaining an adapter after HWND/root removal. The old
    // strong ElementInputHandler demonstrably keeps the surface/session alive.
    let strong = gpui::ElementInputHandler::new(Bounds::default(), surface.clone());
    window.update(|window, _| window.remove_window());
    window.cx.update(|_| drop(surface));
    window.run_until_parked();
    assert!(
        weak.upgrade().is_some(),
        "strong adapter reproduces retention"
    );
    window.cx.update(|_| drop(strong));
    window.run_until_parked();
    assert!(
        weak.upgrade().is_none(),
        "retained weak adapter cannot own the surface"
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    while Arc::strong_count(&endpoint) != 1 {
        window.run_until_parked();
        assert!(Instant::now() < deadline, "detached native clear deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
    // A stale platform callback must be inert even if a later window exists.
    let other = window.cx.add_empty_window();
    other.update(|window, cx| {
        retained.replace_text_in_range(None, "must not commit", window, cx);
        assert!(!retained.accepts_text_input(window, cx));
        assert!(retained.selected_text_range(false, window, cx).is_none());
        assert!(retained.element_bounds(window, cx).is_none());
    });
    assert_eq!(*endpoint.commits.lock().unwrap(), ["IME界"]);
}

#[gpui::test]
fn native_search_supersession_navigation_invalidation_and_close(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let engine = Arc::new(AsyncEngine::spawn(16, 3).unwrap());
    call(
        &engine,
        Command::Feed(
            "old needle\r\n界needle\r\nneedle\r\nlast"
                .as_bytes()
                .to_vec(),
        ),
    );
    call(&engine, Command::Capture);
    let mut endpoint = Endpoint::new(engine.model());
    endpoint.selection_engine = Some(engine.clone());
    let endpoint = Arc::new(endpoint);
    let errors = Rc::new(RefCell::new(Vec::new()));
    let source = endpoint.clone();
    let received = errors.clone();
    let (surface, cx) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("monospace"),
            px(14.),
            move |error, _| received.borrow_mut().push(format!("{error:?}")),
            window,
            cx,
        )
    });
    let settle = |cx: &mut gpui::VisualTestContext| {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            cx.run_until_parked();
            if !surface.read_with(cx, |surface, _| surface.search.busy) {
                break;
            }
            cx.executor().advance_clock(Duration::from_millis(200));
            assert!(Instant::now() < deadline, "search task deadline");
            std::thread::sleep(Duration::from_millis(1));
        }
    };
    surface.update_in(cx, |surface, window, cx| {
        surface.key_down(&down("ctrl-f", false), window, cx);
        assert!(surface.search.open);
        assert!(
            surface
                .search
                .input
                .as_ref()
                .unwrap()
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        surface.key_down(&down("ctrl-a", false), window, cx);
        surface.search.query = "old".into();
        surface.request_search(cx);
    });
    cx.run_until_parked();
    surface.update(cx, |surface, cx| {
        surface.search.query = "needle".into();
        surface.request_search(cx);
    });
    settle(cx);
    surface.read_with(cx, |surface, _| {
        assert_eq!(surface.search.result.as_ref().unwrap().matches.len(), 3);
        assert_eq!(surface.search.index, Some(0));
    });
    assert!(endpoint.keys.lock().unwrap().is_empty());
    // Script only the progress status, retaining real native result provenance.
    // This is a UI budget proof, not a claim that native tick always runs long.
    use oneterm_terminal::ghostty::search::Status;
    *endpoint.search_status.lock().unwrap() = Some(Status::Running);
    let before_steps = endpoint.search_steps.load(Ordering::Relaxed);
    surface.update(cx, |surface, cx| surface.request_search(cx));
    settle(cx);
    assert_eq!(
        endpoint.search_steps.load(Ordering::Relaxed) - before_steps,
        63
    );
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(
        endpoint.search_steps.load(Ordering::Relaxed) - before_steps,
        63
    );
    *endpoint.search_status.lock().unwrap() = Some(Status::FeedRequired);
    let before_steps = endpoint.search_steps.load(Ordering::Relaxed);
    surface.update(cx, |surface, cx| surface.request_search(cx));
    settle(cx);
    assert_eq!(endpoint.search_steps.load(Ordering::Relaxed), before_steps);
    assert_eq!(
        surface.read_with(cx, |surface, _| surface
            .search
            .result
            .as_ref()
            .unwrap()
            .status),
        Status::FeedRequired
    );
    *endpoint.search_status.lock().unwrap() = None;
    // Hold one endpoint acknowledgment after the driver has started it.
    // Editing must not replace/drop that future or expose its superseded reply.
    let (release, gate) = async_channel::bounded(1);
    *endpoint.search_gate.lock().unwrap() = Some(gate);
    let before = endpoint.searches.lock().unwrap().len();
    surface.update(cx, |surface, cx| {
        surface.search.query = "old".into();
        surface.request_search(cx);
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    while endpoint.searches.lock().unwrap().len() == before {
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        assert!(Instant::now() < deadline, "search admission deadline");
    }
    surface.update(cx, |surface, cx| {
        surface.search.query = "needle".into();
        surface.request_search(cx);
    });
    cx.run_until_parked();
    assert_eq!(endpoint.searches.lock().unwrap().len(), before + 1);
    release.try_send(()).unwrap();
    settle(cx);
    assert_eq!(
        surface.read_with(cx, |surface, _| surface
            .search
            .result
            .as_ref()
            .unwrap()
            .matches
            .len()),
        3
    );
    surface.update(cx, |surface, cx| surface.navigate_search(true, cx));
    settle(cx);
    surface.read_with(cx, |surface, _| {
        assert_eq!(surface.search.index, Some(2));
        let result = surface.search.result.as_ref().unwrap();
        let viewport = endpoint.model.query().viewport.unwrap();
        assert!(viewport.matches_search(result));
        assert_eq!(viewport.metadata().scrollbar.offset, 0);
        let mut highlights = Vec::new();
        crate::render::overlay::native_search_highlights(
            &viewport,
            result,
            surface.search.index,
            &mut highlights,
        );
        assert!(
            highlights
                .iter()
                .any(|highlight| highlight.active && highlight.display_line == 0)
        );
    });
    // Mutation without capture must suppress old coordinates immediately.
    call(&engine, Command::Feed(b"!".to_vec()));
    surface.read_with(cx, |surface, _| {
        assert!(
            !endpoint
                .model
                .query()
                .matches_search(surface.search.result.as_ref().unwrap())
        )
    });
    call(&engine, Command::Capture);
    surface.update(cx, |surface, cx| surface.refresh_search(cx));
    settle(cx);
    assert_eq!(
        surface.read_with(cx, |surface, _| surface.search.index),
        Some(0)
    );
    surface.update(cx, |surface, cx| {
        surface.search.query = "界".repeat(1366);
        surface.request_search(cx);
    });
    settle(cx);
    assert!(surface.read_with(cx, |surface, _| surface.search.error.is_some()));
    assert!(
        complete(endpoint.model.search_step())
            .unwrap()
            .matches
            .is_empty()
    );
    // Closing while an admitted search is awaiting its acknowledgment must not
    // drop that future; the only following native operation is the ordered clear.
    let (release, gate) = async_channel::bounded(1);
    *endpoint.search_gate.lock().unwrap() = Some(gate);
    let before = endpoint.searches.lock().unwrap().len();
    surface.update(cx, |surface, cx| {
        surface.search.query = "needle".into();
        surface.request_search(cx);
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    while endpoint.searches.lock().unwrap().len() == before {
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        assert!(
            Instant::now() < deadline,
            "pending close search admission deadline"
        );
    }
    surface.update_in(cx, |surface, window, cx| surface.close_search(window, cx));
    cx.run_until_parked();
    assert_eq!(endpoint.searches.lock().unwrap().len(), before + 1);
    release.try_send(()).unwrap();
    settle(cx);
    assert_eq!(endpoint.searches.lock().unwrap().last().unwrap(), "");
    surface.update_in(cx, |surface, window, _| {
        assert!(surface.focus.is_focused(window))
    });
    assert!(errors.borrow().is_empty());
    assert!(
        complete(endpoint.model.search_step())
            .unwrap()
            .matches
            .is_empty()
    );
    // A non-root surface can really be released without closing the owner. Its
    // completed driver cannot perform cleanup, so the release hook must do it.
    let idle = surface.update_in(cx, |_, window, cx| {
        cx.new(|cx| {
            NativeTerminalSurface::new(
                endpoint.clone(),
                gpui::font("monospace"),
                px(14.),
                |_, _| panic!("idle search error"),
                window,
                cx,
            )
        })
    });
    idle.update_in(cx, |surface, window, cx| {
        surface.key_down(&down("ctrl-f", false), window, cx);
        surface.search.query = "needle".into();
        surface.request_search(cx);
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    while idle.read_with(cx, |surface, _| surface.search.busy) {
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        assert!(Instant::now() < deadline, "idle release search deadline");
    }
    assert!(
        !complete(endpoint.model.search_step())
            .unwrap()
            .matches
            .is_empty()
    );
    let weak = idle.downgrade();
    let before = endpoint.searches.lock().unwrap().len();
    cx.update(|_, _| drop(idle));
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if endpoint.searches.lock().unwrap().len() > before
            && complete(endpoint.model.search_step())
                .unwrap()
                .matches
                .is_empty()
        {
            break;
        }
        assert!(Instant::now() < deadline, "idle release clear deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(weak.upgrade().is_none());
    // Release a genuinely busy entity: its detached driver, not the idle
    // release hook, must acknowledge the held operation before clearing.
    let busy = surface.update_in(cx, |_, window, cx| {
        cx.new(|cx| {
            NativeTerminalSurface::new(
                endpoint.clone(),
                gpui::font("monospace"),
                px(14.),
                |_, _| panic!("busy search error"),
                window,
                cx,
            )
        })
    });
    let (release, gate) = async_channel::bounded(1);
    *endpoint.search_gate.lock().unwrap() = Some(gate);
    let before = endpoint.searches.lock().unwrap().len();
    busy.update_in(cx, |surface, window, cx| {
        surface.key_down(&down("ctrl-f", false), window, cx);
        surface.search.query = "needle".into();
        surface.request_search(cx);
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    while endpoint.searches.lock().unwrap().len() == before {
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        assert!(Instant::now() < deadline, "busy release admission deadline");
    }
    let weak = busy.downgrade();
    cx.update(|_, _| drop(busy));
    cx.run_until_parked();
    assert!(weak.upgrade().is_none());
    assert_eq!(endpoint.searches.lock().unwrap().len(), before + 1);
    release.try_send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        if endpoint.searches.lock().unwrap().len() == before + 2
            && complete(endpoint.model.search_step())
                .unwrap()
                .matches
                .is_empty()
        {
            break;
        }
        assert!(Instant::now() < deadline, "busy release clear deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(endpoint.searches.lock().unwrap().last().unwrap(), "");
    assert!(
        !complete(endpoint.model.search("needle".into(), Default::default()))
            .unwrap()
            .matches
            .is_empty()
    );
    complete(engine.close()).unwrap();
}

#[gpui::test]
fn native_search_input_edits_composition_and_commit_stay_in_widget(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let engine = Arc::new(AsyncEngine::spawn(32, 3).unwrap());
    call(
        &engine,
        Command::Feed("needle 日本😀 e\u{301}\r\n日本😀".as_bytes().to_vec()),
    );
    call(&engine, Command::Capture);
    let mut endpoint = Endpoint::new(engine.model());
    endpoint.selection_engine = Some(engine.clone());
    let endpoint = Arc::new(endpoint);
    let source = endpoint.clone();
    let (surface, cx) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("monospace"),
            px(14.),
            |error, _| panic!("search input error: {error:?}"),
            window,
            cx,
        )
    });
    surface.update_in(cx, |surface, window, cx| {
        surface.key_down(&down("ctrl-f", false), window, cx)
    });
    let input = surface.read_with(cx, |surface, _| surface.search.input.clone().unwrap());
    let settle = |cx: &mut gpui::VisualTestContext| {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            cx.executor().advance_clock(Duration::from_millis(200));
            cx.run_until_parked();
            if !surface.read_with(cx, |surface, _| surface.search.busy) {
                break;
            }
            assert!(Instant::now() < deadline, "widget search deadline");
            std::thread::sleep(Duration::from_millis(1));
        }
    };
    cx.simulate_input("needle");
    settle(cx);
    surface.read_with(cx, |surface, _| {
        assert_eq!(surface.search.query, "needle");
        assert_eq!(surface.search.result.as_ref().unwrap().matches.len(), 1);
    });
    // The platform's preedit and commit entry points belong to InputState.
    // Use UTF-16 ranges, including a surrogate pair and combining character.
    input.update_in(cx, |input, window, cx| {
        input.replace_and_mark_text_in_range(Some(0..6), "に", Some(1..1), window, cx);
        assert_eq!(input.marked_text_range(window, cx), Some(0..1));
        input.replace_and_mark_text_in_range(None, "日本😀", Some(4..4), window, cx);
        assert_eq!(input.marked_text_range(window, cx), Some(0..4));
        input.replace_text_in_range(None, "日本😀", window, cx);
        assert_eq!(input.marked_text_range(window, cx), None);
    });
    settle(cx);
    surface.read_with(cx, |surface, _| {
        assert_eq!(surface.search.query, "日本😀");
        assert_eq!(surface.search.result.as_ref().unwrap().matches.len(), 2);
        assert!(
            endpoint
                .model
                .query()
                .matches_search(surface.search.result.as_ref().unwrap())
        );
    });
    let entered = Rc::new(RefCell::new(Vec::new()));
    let entered_events = entered.clone();
    let _entered_subscription = surface.update(cx, |_, cx| {
        cx.subscribe(
            &input,
            move |_, _, event: &gpui_component::input::InputEvent, _| {
                entered_events.borrow_mut().push(match event {
                    gpui_component::input::InputEvent::PressEnter { shift, .. } => {
                        if *shift {
                            "prev"
                        } else {
                            "next"
                        }
                    }
                    gpui_component::input::InputEvent::Change => "change",
                    _ => "focus",
                });
            },
        )
    });
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(
        surface.read_with(cx, |surface, _| surface.search.index),
        Some(1),
        "events: {:?}; keys: {:?}; searches: {:?}",
        entered.borrow(),
        endpoint.keys.lock().unwrap(),
        endpoint.searches.lock().unwrap()
    );
    cx.simulate_keystrokes("shift-enter");
    settle(cx);
    assert_eq!(
        surface.read_with(cx, |surface, _| surface.search.index),
        Some(0)
    );
    input.update_in(cx, |input, window, cx| {
        input.replace_text_in_range(Some(0..4), "e\u{301}", window, cx)
    });
    settle(cx);
    assert_eq!(
        surface.read_with(cx, |surface, _| surface.search.query.clone()),
        "e\u{301}"
    );
    assert_eq!(
        surface.read_with(cx, |surface, _| surface
            .search
            .result
            .as_ref()
            .unwrap()
            .matches
            .len()),
        1
    );
    input.update_in(cx, |input, window, cx| {
        input.replace_text_in_range(Some(0..2), "", window, cx)
    });
    settle(cx);
    assert!(
        surface.read_with(cx, |surface, _| surface.search.query.is_empty()
            && surface.search.result.is_none())
    );
    assert!(
        complete(endpoint.model.search_step())
            .unwrap()
            .matches
            .is_empty()
    );
    assert!(endpoint.keys.lock().unwrap().is_empty());
    assert!(endpoint.commits.lock().unwrap().is_empty());
    assert!(endpoint.pastes.lock().unwrap().is_empty());
    assert!(endpoint.composition.marked_text().is_none());
    surface.update_in(cx, |surface, window, cx| {
        surface.key_down(&down("escape", false), window, cx)
    });
    settle(cx);
    surface.update_in(cx, |surface, window, _| {
        assert!(surface.focus.is_focused(window))
    });
    complete(engine.close()).unwrap();
}

#[test]
fn native_search_highlights_soft_wraps_and_full_width_endpoints() {
    use oneterm_terminal::ghostty::search::Options;
    let engine = AsyncEngine::spawn(6, 3).unwrap();
    call(
        &engine,
        Command::Feed("abcDEFghi\r\n界".as_bytes().to_vec()),
    );
    call(&engine, Command::Capture);
    let model = engine.model();
    let viewport = model.query().viewport.unwrap();
    let found = complete(model.search("DEFg".into(), Options::default())).unwrap();
    let mut highlights = Vec::new();
    crate::render::overlay::native_search_highlights(&viewport, &found, Some(0), &mut highlights);
    assert_eq!(
        highlights
            .iter()
            .map(|span| (span.display_line, span.start_col, span.end_col, span.active))
            .collect::<Vec<_>>(),
        [(0, 3, 6, true), (1, 0, 1, true)]
    );
    let wide = complete(model.search("界".into(), Options::default())).unwrap();
    crate::render::overlay::native_search_highlights(&viewport, &wide, Some(0), &mut highlights);
    assert_eq!(
        highlights
            .iter()
            .map(|span| (span.display_line, span.start_col, span.end_col))
            .collect::<Vec<_>>(),
        [(2, 0, 2)]
    );
    complete(engine.close()).unwrap();
}

#[gpui::test]
fn native_surface_clipboard_copy_paste_busy_repeat_and_errors(cx: &mut TestAppContext) {
    // The native owner is a real OS thread, not GPUI's deterministic executor.
    cx.executor().allow_parking();
    use gpui::ClipboardItem;
    use oneterm_terminal::ghostty::anchors::{Coordinate, Point, Space};
    cx.update(gpui_component::init);
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Feed(b"native copy".to_vec()));
    call(&engine, Command::Capture);
    let endpoint = Arc::new(Endpoint::new(engine.model()));
    let errors = Rc::new(RefCell::new(Vec::new()));
    let source = endpoint.clone();
    let received = errors.clone();
    let (surface, cx) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("monospace"),
            px(14.0),
            move |error, _| received.borrow_mut().push(format!("{error:?}")),
            window,
            cx,
        )
    });
    let settle = |cx: &mut gpui::VisualTestContext| {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            cx.run_until_parked();
            if !surface.read_with(cx, |surface, _| surface.edit.busy) {
                break;
            }
            assert!(Instant::now() < deadline, "clipboard task deadline");
            std::thread::sleep(Duration::from_millis(1));
        }
    };
    cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("keep".into())));
    surface.update(cx, |surface, cx| surface.copy_clipboard(cx));
    settle(cx);
    cx.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("keep")
        )
    });
    let anchor = |column| {
        let Reply::Anchor(id) = call(
            &engine,
            Command::CreateAnchor(Point {
                space: Space::Screen,
                coordinate: Coordinate { column, row: 0 },
            }),
        ) else {
            panic!("anchor reply")
        };
        id
    };
    call(&engine, Command::SetSelection(anchor(0), anchor(5), false));
    surface.update(cx, |surface, cx| surface.copy_clipboard(cx));
    settle(cx);
    cx.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("native")
        )
    });
    let chord = if cfg!(target_os = "macos") {
        "cmd-v"
    } else {
        "ctrl-shift-v"
    };
    surface.update_in(cx, |surface, window, cx| {
        surface.key_down(&down(chord, false), window, cx);
        surface.paste_clipboard(cx); // Busy rejection, never replayed.
        surface.key_down(&down(chord, true), window, cx);
    });
    settle(cx);
    assert_eq!(*endpoint.pastes.lock().unwrap(), ["native"]);
    assert!(endpoint.keys.lock().unwrap().is_empty());
    assert!(endpoint.commits.lock().unwrap().is_empty());
    assert_eq!(errors.borrow().len(), 1);
    endpoint.reject.store(true, Ordering::Relaxed);
    surface.update(cx, |surface, cx| surface.paste_clipboard(cx));
    settle(cx);
    assert_eq!(errors.borrow().len(), 2);
    assert_eq!(endpoint.pastes.lock().unwrap().len(), 1);
    endpoint.reject.store(false, Ordering::Relaxed);
    cx.update(|_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string("x".repeat(1024 * 1024 + 1)))
    });
    surface.update(cx, |surface, cx| surface.paste_clipboard(cx));
    assert_eq!(errors.borrow().len(), 3);
    assert!(!surface.read_with(cx, |surface, _| surface.edit.busy));
    assert_eq!(endpoint.pastes.lock().unwrap().len(), 1);
    cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string(String::new())));
    surface.update(cx, |surface, cx| surface.paste_clipboard(cx));
    assert_eq!(errors.borrow().len(), 3);
    assert!(!surface.read_with(cx, |surface, _| surface.edit.busy));
}

#[test]
fn native_surface_mapper_preserves_layout_without_inventing_physical_keys() {
    for text in ["a", "é", "日", "😀"] {
        let stroke = Keystroke {
            key: text.into(),
            key_char: Some(text.into()),
            ..Default::default()
        };
        let key = key_event(&stroke).unwrap();
        assert_eq!(key.code, KeyCode::UNIDENTIFIED);
        assert_eq!(key.text, text);
        assert_eq!(key.unshifted, None);
        key.validate().unwrap();
    }
    let mut stroke = Keystroke::parse("ctrl-c").unwrap();
    stroke.key_char = Some("\x03".into());
    let key = key_event(&stroke).unwrap();
    assert_eq!(key.code, KeyCode::UNIDENTIFIED);
    assert_eq!(key.text, "c");
    assert!(key.modifiers.control);
    key.validate().unwrap();
    assert_eq!(
        key_event(&Keystroke::parse("f25").unwrap()).unwrap().code,
        KeyCode::from_code(145).unwrap()
    );
    assert!(key_event(&Keystroke::parse("f26").unwrap()).is_none());
}

#[gpui::test]
fn native_surface_pointer_pairing_shift_resize_and_errors(cx: &mut TestAppContext) {
    use gpui::{MouseDownEvent, MouseMoveEvent, MouseUpEvent, point};
    use oneterm_terminal::ghostty::input::{MouseAction, MouseButton as NativeButton};
    cx.update(gpui_component::init);
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Feed(b"\x1b[?1000h".to_vec()));
    let endpoint = Arc::new(Endpoint::new(engine.model()));
    let errors = Rc::new(RefCell::new(Vec::new()));
    let source = endpoint.clone();
    let received = errors.clone();
    let (surface, cx) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("monospace"),
            px(14.0),
            move |error, _| received.borrow_mut().push(format!("{error:?}")),
            window,
            cx,
        )
    });
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let geometry = surface.read_with(cx, |surface, _| surface.state.borrow().geometry.unwrap());
    let position = point(
        geometry.origin.x + geometry.metrics.cell_width * 2.25,
        geometry.origin.y + geometry.metrics.line_height * 1.5,
    );
    let mut press = MouseDownEvent {
        position,
        button: MouseButton::Left,
        ..Default::default()
    };
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_down(&press, window, cx)
    });
    assert!(
        endpoint.mice.lock().unwrap().is_empty(),
        "no absent-publication input"
    );
    call(
        &engine,
        Command::Resize {
            cols: geometry.size.cols,
            rows: geometry.size.rows,
            cell_width_px: geometry.metrics.device.w as u32,
            cell_height_px: geometry.metrics.device.h as u32,
        },
    );
    call(&engine, Command::Capture);
    press.modifiers.shift = true;
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_down(&press, window, cx)
    });
    assert!(
        endpoint.mice.lock().unwrap().is_empty(),
        "Shift bypasses reporting"
    );
    press.modifiers.shift = false;
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_down(&press, window, cx);
        surface.pointer_down(&press, window, cx);
        surface.pointer_move(
            &MouseMoveEvent {
                position,
                pressed_button: Some(MouseButton::Left),
                ..Default::default()
            },
            window,
            cx,
        );
        surface.pointer_up(
            &MouseUpEvent {
                position: point(px(-100.0), px(-100.0)),
                button: MouseButton::Left,
                modifiers: gpui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            window,
            cx,
        );
        surface.pointer_up(
            &MouseUpEvent {
                position,
                ..Default::default()
            },
            window,
            cx,
        );
    });
    let mice = endpoint.mice.lock().unwrap();
    assert_eq!(
        mice.len(),
        3,
        "duplicate down/up cannot fabricate another pair"
    );
    assert!(matches!(mice[0].action, MouseAction::Press));
    assert!(matches!(mice[1].action, MouseAction::Motion));
    assert!(matches!(mice[2].action, MouseAction::Release));
    assert!(matches!(mice[0].button, Some(NativeButton::Left)));
    assert_eq!(mice[0].x, 2.25 * geometry.metrics.device.w as f32);
    assert_eq!(mice[0].y, 1.5 * geometry.metrics.device.h as f32);
    assert_eq!(mice[2].x, mice[1].x);
    assert!(mice[2].modifiers.shift);
    assert!(!mice[2].any_button_pressed);
    drop(mice);
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_down(&press, window, cx);
        surface.release_pointer(cx);
        surface.release_pointer(cx);
    });
    assert_eq!(endpoint.mice.lock().unwrap().len(), 5);
    cx.simulate_mouse_move(position, None, gpui::Modifiers::default());
    cx.simulate_mouse_down(position, MouseButton::Middle, gpui::Modifiers::default());
    cx.simulate_mouse_up(
        point(px(-100.0), px(-100.0)),
        MouseButton::Middle,
        gpui::Modifiers::default(),
    );
    let mice = endpoint.mice.lock().unwrap();
    assert!(matches!(mice[mice.len() - 2].action, MouseAction::Press));
    assert!(matches!(
        mice[mice.len() - 2].button,
        Some(NativeButton::Middle)
    ));
    assert!(
        matches!(mice[mice.len() - 1].action, MouseAction::Release),
        "wired outside-up callback releases admitted press"
    );
    let before_reject = mice.len();
    drop(mice);
    endpoint.reject.store(true, Ordering::Relaxed);
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_down(&press, window, cx);
        surface.release_pointer(cx);
    });
    assert_eq!(
        errors.borrow().len(),
        1,
        "rejected press has no cleanup retry"
    );
    assert_eq!(endpoint.mice.lock().unwrap().len(), before_reject);
    endpoint.reject.store(false, Ordering::Relaxed);
    call(
        &engine,
        Command::Resize {
            cols: 16,
            rows: 3,
            cell_width_px: 9,
            cell_height_px: 18,
        },
    );
    call(&engine, Command::Capture);
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_down(&press, window, cx)
    });
    assert_eq!(
        endpoint.mice.lock().unwrap().len(),
        before_reject,
        "size mismatch suppresses input"
    );
    call(
        &engine,
        Command::Resize {
            cols: geometry.size.cols,
            rows: geometry.size.rows,
            cell_width_px: geometry.metrics.device.w as u32,
            cell_height_px: geometry.metrics.device.h as u32,
        },
    );
    call(&engine, Command::Capture);
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_down(&press, window, cx)
    });
    call(
        &engine,
        Command::Resize {
            cols: 16,
            rows: 3,
            cell_width_px: 9,
            cell_height_px: 18,
        },
    );
    call(&engine, Command::Capture);
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_up(
            &MouseUpEvent {
                position,
                button: MouseButton::Left,
                ..Default::default()
            },
            window,
            cx,
        )
    });
    let mice = endpoint.mice.lock().unwrap();
    assert_eq!(mice.len(), before_reject + 2);
    assert!(matches!(mice.last().unwrap().action, MouseAction::Release));
    assert_eq!(
        mice.last().unwrap().geometry.width,
        u32::from(geometry.size.cols) * geometry.metrics.device.w as u32,
        "resize-gap release retains admitted geometry"
    );
    drop(mice);
    call(
        &engine,
        Command::Resize {
            cols: geometry.size.cols,
            rows: geometry.size.rows,
            cell_width_px: geometry.metrics.device.w as u32,
            cell_height_px: geometry.metrics.device.h as u32,
        },
    );
    call(&engine, Command::Capture);
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_down(&press, window, cx)
    });
    endpoint.reject.store(true, Ordering::Relaxed);
    surface.update(cx, |surface, cx| {
        surface.release_pointer(cx);
        surface.release_pointer(cx);
    });
    assert_eq!(
        errors.borrow().len(),
        2,
        "rejected cleanup is observable exactly once"
    );
    complete(engine.close()).unwrap();
}

#[gpui::test]
fn native_surface_drag_coalesces_motion_copies_native_text_and_finishes_outside(
    cx: &mut TestAppContext,
) {
    use gpui::{MouseDownEvent, MouseMoveEvent, MouseUpEvent, point};
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let settings = cx.new(|_| oneterm_settings::TerminalSettings::default());
    settings.update(cx, |settings, _| settings.copy_on_select = true);
    let engine = Arc::new(AsyncEngine::spawn(16, 3).unwrap());
    let mut endpoint = Endpoint::new(engine.model());
    endpoint.selection_engine = Some(engine.clone());
    let endpoint = Arc::new(endpoint);
    let source = endpoint.clone();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let received = errors.clone();
    let (surface, cx) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("monospace"),
            px(14.0),
            move |error, _| received.borrow_mut().push(format!("{error:?}")),
            window,
            cx,
        )
    });
    surface.update(cx, |surface, cx| {
        surface.bind_settings(settings.clone(), cx)
    });
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let geometry = surface.read_with(cx, |surface, _| surface.state.borrow().geometry.unwrap());
    call(
        &engine,
        Command::Resize {
            cols: geometry.size.cols,
            rows: geometry.size.rows,
            cell_width_px: geometry.metrics.device.w as u32,
            cell_height_px: geometry.metrics.device.h as u32,
        },
    );
    call(&engine, Command::Feed(b"native selection".to_vec()));
    call(&engine, Command::Capture);
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let position = |column| {
        point(
            geometry.origin.x + geometry.metrics.cell_width * column,
            geometry.origin.y + geometry.metrics.line_height * 0.25,
        )
    };
    let settle = |cx: &mut gpui::VisualTestContext| {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            cx.run_until_parked();
            if !surface.read_with(cx, |surface, _| surface.drag.active()) {
                break;
            }
            assert!(Instant::now() < deadline, "drag task deadline");
            std::thread::sleep(Duration::from_millis(1));
        }
    };
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_down(
            &MouseDownEvent {
                position: position(0.25),
                button: MouseButton::Left,
                ..Default::default()
            },
            window,
            cx,
        );
        for col in 1..=1000 {
            surface.pointer_move(
                &MouseMoveEvent {
                    position: position(if col == 1000 { 5.25 } else { 2.25 }),
                    pressed_button: Some(MouseButton::Left),
                    ..Default::default()
                },
                window,
                cx,
            );
        }
        surface.pointer_up(
            &MouseUpEvent {
                position: point(px(-100.0), px(-100.0)),
                button: MouseButton::Left,
                ..Default::default()
            },
            window,
            cx,
        );
    });
    settle(cx);
    assert!(errors.borrow().is_empty(), "{:?}", errors.borrow());
    assert_eq!(
        endpoint.selections.lock().unwrap().len(),
        3,
        "begin, latest move, finish only"
    );
    assert!(endpoint.mice.lock().unwrap().is_empty());
    assert_eq!(
        complete(engine.model().selection_text())
            .unwrap()
            .as_deref(),
        Some("native")
    );
    let copy_deadline = Instant::now() + Duration::from_secs(20);
    while surface.read_with(cx, |surface, _| surface.edit.busy) {
        cx.run_until_parked();
        assert!(Instant::now() < copy_deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    cx.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("native")
        )
    });
    assert!(
        !engine
            .model()
            .query()
            .viewport
            .unwrap()
            .selection_spans()
            .is_empty()
    );
    // Shift forces local selection even when the published terminal owns mouse input.
    call(&engine, Command::Feed(b"\x1b[?1000h".to_vec()));
    call(&engine, Command::Capture);
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_down(
            &MouseDownEvent {
                position: position(0.25),
                button: MouseButton::Left,
                modifiers: gpui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            window,
            cx,
        );
        surface.pointer_move(
            &MouseMoveEvent {
                position: position(2.25),
                pressed_button: Some(MouseButton::Left),
                ..Default::default()
            },
            window,
            cx,
        );
        surface.release_pointer(cx); // Blur finishes at the last valid sample.
    });
    settle(cx);
    assert!(endpoint.mice.lock().unwrap().is_empty());
    assert_eq!(
        complete(engine.model().selection_text())
            .unwrap()
            .as_deref(),
        Some("nat")
    );
    assert!(errors.borrow().is_empty(), "{:?}", errors.borrow());
    // Drop the gesture state while idle, as entity teardown does. Its one task
    // must complete Finish rather than leaking the owner-held start anchor.
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_down(
            &MouseDownEvent {
                position: position(0.25),
                button: MouseButton::Left,
                modifiers: gpui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            window,
            cx,
        )
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    while endpoint.selection_completed.load(Ordering::Relaxed) < 7 {
        cx.run_until_parked();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    surface.update(cx, |surface, _| {
        drop(std::mem::take(&mut surface.drag));
    });
    while endpoint.selection_completed.load(Ordering::Relaxed) < 8 {
        cx.run_until_parked();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    use oneterm_terminal::ghostty::anchors::{Coordinate, Point, Space};
    let model = engine.model();
    let mut anchors = Vec::new();
    for _ in 0..oneterm_terminal::ghostty::anchors::MAX_ANCHORS {
        anchors.push(
            complete(model.create_anchor(Point {
                space: Space::Viewport,
                coordinate: Coordinate { column: 0, row: 0 },
            }))
            .unwrap(),
        );
    }
    for anchor in anchors {
        complete(model.release_anchor(&anchor)).unwrap();
    }
}

#[gpui::test]
fn native_terminal_tab_reaches_shell_without_changing_focus(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    cx.update(crate::init);
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Capture);
    let endpoint = Arc::new(Endpoint::new(engine.model()));
    let source = endpoint.clone();
    let surface = Rc::new(RefCell::new(None));
    let created = surface.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let terminal = cx.new(|cx| {
            NativeTerminalSurface::new(
                source,
                gpui::font("monospace"),
                px(14.),
                |error, _| panic!("Tab input failed: {error:?}"),
                window,
                cx,
            )
        });
        window.focus(&terminal.read(cx).focus_handle(cx), cx);
        *created.borrow_mut() = Some(terminal.clone());
        gpui_component::Root::new(terminal, window, cx)
    });
    let surface = surface.borrow_mut().take().unwrap();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    for chord in ["tab", "shift-tab"] {
        endpoint.keys.lock().unwrap().clear();
        cx.simulate_keystrokes(chord);
        cx.run_until_parked();
        let keys = endpoint.keys.lock().unwrap();
        let presses: Vec<_> = keys
            .iter()
            .filter(|key| matches!(key.action, KeyAction::Press))
            .collect();
        assert_eq!(presses.len(), 1, "{chord} is delivered once: {keys:?}");
        assert_eq!(presses[0].code, KeyCode::TAB);
        assert_eq!(presses[0].modifiers.shift, chord == "shift-tab");
        drop(keys);
        cx.update(|window, cx| {
            assert!(surface.read(cx).focus.is_focused(window));
        });
    }
    assert!(endpoint.commits.lock().unwrap().is_empty());
    complete(engine.close()).unwrap();
}

#[gpui::test]
fn native_tui_ime_candidates_commit_and_primary_screen_return(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Capture);
    let endpoint = Arc::new(Endpoint::new(engine.model()));
    let source = endpoint.clone();
    let (surface, cx) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("monospace"),
            px(14.),
            |error, _| panic!("TUI IME failed: {error:?}"),
            window,
            cx,
        )
    });
    for alternate in [false, true, false] {
        let sequence = if alternate {
            b"\x1b[?1049h\x1b[2;4H".as_slice()
        } else {
            b"\x1b[?1049l\x1b[2;4H".as_slice()
        };
        call(&engine, Command::Feed(sequence.to_vec()));
        call(&engine, Command::Capture);
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        endpoint.keys.lock().unwrap().clear();
        endpoint.commits.lock().unwrap().clear();
        surface.update_in(cx, |surface, window, cx| {
            assert_eq!(
                surface
                    .session
                    .query()
                    .viewport
                    .unwrap()
                    .metadata()
                    .alternate_screen,
                alternate
            );
            assert!(surface.accepts_text_input(window, cx));
            assert_eq!(
                surface
                    .selected_text_range(false, window, cx)
                    .unwrap()
                    .range,
                0..0
            );
            surface.key_down(&down("n", false), window, cx);
            assert!(
                endpoint.keys.lock().unwrap().is_empty(),
                "printable text belongs to IME"
            );
            surface.replace_and_mark_text_in_range(None, "ni", None, window, cx);
            assert_eq!(surface.marked_text_range(window, cx), Some(0..2));
            surface.key_down(&down("enter", false), window, cx);
            surface.key_down(&down("up", false), window, cx);
            assert!(
                endpoint.keys.lock().unwrap().is_empty(),
                "candidate keys cannot reach TUI"
            );
            let bounds = surface
                .bounds_for_range(0..2, Bounds::default(), window, cx)
                .unwrap();
            let geometry = surface.state.borrow().geometry.unwrap();
            assert_eq!(bounds.origin, geometry.cell_origin(1, 3));
            surface.replace_text_in_range(None, "你好", window, cx);
            assert!(surface.session.marked_text().is_none());
            assert!(surface.held.is_empty());
            assert_eq!(*endpoint.commits.lock().unwrap(), ["你好"]);
            surface.key_down(&down("a", false), window, cx);
            surface.replace_text_in_range(None, "a", window, cx);
            assert!(
                endpoint.keys.lock().unwrap().is_empty(),
                "ASCII must not be sent twice"
            );
            assert_eq!(*endpoint.commits.lock().unwrap(), ["你好", "a"]);
            for chord in ["up", "ctrl-c"] {
                surface.key_down(&down(chord, false), window, cx);
                surface.key_up(
                    &KeyUpEvent {
                        keystroke: Keystroke::parse(chord).unwrap(),
                    },
                    window,
                    cx,
                );
            }
            assert_eq!(
                endpoint.keys.lock().unwrap().len(),
                4,
                "functional keys and chords retain press/release"
            );
            assert!(surface.held.is_empty());
            surface.replace_and_mark_text_in_range(None, "cancel", None, window, cx);
            surface.unmark_text(window, cx);
            assert!(surface.marked_text_range(window, cx).is_none());
            assert_eq!(
                *endpoint.commits.lock().unwrap(),
                ["你好", "a"],
                "cancel does not commit"
            );
        });
    }
    complete(engine.close()).unwrap();
}

#[gpui::test]
fn native_surface_render_keys_ime_and_errors_without_legacy_session(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    call(&engine, Command::Feed(b"abc".to_vec()));
    call(&engine, Command::Capture);
    let endpoint = Arc::new(Endpoint::new(engine.model()));
    let errors = Rc::new(RefCell::new(Vec::new()));
    let source = endpoint.clone();
    let received = errors.clone();
    let (surface, cx) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("monospace"),
            px(14.0),
            move |error, _| received.borrow_mut().push(format!("{error:?}")),
            window,
            cx,
        )
    });
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    assert!(endpoint.repaint_acks.load(Ordering::Relaxed) > 0);
    let sizes = endpoint.sizes.lock().unwrap();
    assert!(!sizes.is_empty());
    assert!(sizes[0].cell_width_px > 0 && sizes[0].cell_height_px > 0);
    drop(sizes);
    surface.update_in(cx, |surface, window, cx| {
        surface.key_down(&down("up", false), window, cx);
        surface.key_down(&down("up", true), window, cx);
        surface.key_up(
            &KeyUpEvent {
                keystroke: Keystroke::parse("up").unwrap(),
            },
            window,
            cx,
        );
        surface.key_down(&down("a", false), window, cx);
        assert!(surface.held.is_empty());
        surface.replace_and_mark_text_in_range(None, "e\u{301}日本😀", None, window, cx);
        surface.key_down(&down("enter", false), window, cx);
        surface.key_down(&down("up", false), window, cx);
        assert!(surface.held.is_empty(), "candidate keys belong to IME");
        assert_eq!(surface.marked_text_range(window, cx), Some(0..6));
        let first = surface
            .bounds_for_range(0..0, Bounds::default(), window, cx)
            .unwrap();
        let last = surface
            .bounds_for_range(5..6, Bounds::default(), window, cx)
            .unwrap();
        assert_eq!(
            first, last,
            "all ranges conservatively anchor at native cursor"
        );
        let geometry = surface.state.borrow().geometry.unwrap();
        assert_eq!(first.origin, geometry.cell_origin(0, 3));
        surface.replace_text_in_range(None, "e\u{301}日本😀", window, cx);
        assert_eq!(surface.session.marked_text(), None);
    });
    let keys = endpoint.keys.lock().unwrap();
    assert_eq!(keys.len(), 3);
    assert!(matches!(keys[0].action, KeyAction::Press));
    assert!(matches!(keys[1].action, KeyAction::Repeat));
    assert!(matches!(keys[2].action, KeyAction::Release));
    assert_eq!(keys[0].code, KeyCode::ARROW_UP);
    drop(keys);
    assert_eq!(*endpoint.commits.lock().unwrap(), vec!["e\u{301}日本😀"]);
    endpoint.reject.store(true, Ordering::Relaxed);
    surface.update_in(cx, |surface, window, cx| {
        surface.key_down(&down("enter", false), window, cx);
        assert!(
            surface.held.is_empty(),
            "rejected press never owns a release"
        );
        surface.session.set_marked_text("retained".into()).unwrap();
        surface.replace_and_mark_text_in_range(
            None,
            &"x".repeat(oneterm_terminal::ghostty_ime::MAX_TEXT_BYTES + 1),
            None,
            window,
            cx,
        );
        assert_eq!(surface.session.marked_text().as_deref(), Some("retained"));
        surface.state.borrow_mut().last_cell_pixels = None;
    });
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    assert_eq!(
        errors.borrow().len(),
        3,
        "key, composition and resize rejection reach caller"
    );
    complete(engine.close()).unwrap();
}

#[gpui::test]
fn native_surface_wheel_fractional_dispatch_geometry_and_rejection(cx: &mut TestAppContext) {
    use gpui::{ScrollDelta, ScrollWheelEvent, point};
    cx.update(gpui_component::init);
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    let endpoint = Arc::new(Endpoint::new(engine.model()));
    let errors = Rc::new(RefCell::new(Vec::new()));
    let source = endpoint.clone();
    let received = errors.clone();
    let (surface, cx) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("monospace"),
            px(14.0),
            move |error, _| received.borrow_mut().push(format!("{error:?}")),
            window,
            cx,
        )
    });
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let geometry = surface.read_with(cx, |surface, _| surface.state.borrow().geometry.unwrap());
    let position = point(
        geometry.origin.x + geometry.metrics.cell_width * 2.25,
        geometry.origin.y + geometry.metrics.line_height * 1.5,
    );
    let event = |y| ScrollWheelEvent {
        position,
        delta: ScrollDelta::Lines(point(0.0, y)),
        ..Default::default()
    };
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_wheel(&event(1.0), window, cx)
    });
    assert!(
        endpoint.wheels.lock().unwrap().is_empty(),
        "no absent-publication wheel"
    );
    call(
        &engine,
        Command::Resize {
            cols: geometry.size.cols,
            rows: geometry.size.rows,
            cell_width_px: geometry.metrics.device.w as u32,
            cell_height_px: geometry.metrics.device.h as u32,
        },
    );
    call(&engine, Command::Capture);
    cx.simulate_mouse_move(position, None, gpui::Modifiers::default());
    for _ in 0..4 {
        cx.simulate_event(event(0.25));
    }
    {
        let wheels = endpoint.wheels.lock().unwrap();
        assert_eq!(
            wheels.len(),
            1,
            "real scroll callback accumulates fractional lines"
        );
        assert_eq!(wheels[0].steps, 1);
        assert_eq!(wheels[0].pointer.x, geometry.metrics.device.w as f32 * 2.25);
    }
    let mut shifted = event(-2.0);
    shifted.modifiers.shift = true;
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_wheel(&shifted, window, cx)
    });
    assert!(endpoint.wheels.lock().unwrap()[1].pointer.modifiers.shift);
    endpoint.reject.store(true, Ordering::Relaxed);
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_wheel(&event(1.0), window, cx)
    });
    assert_eq!(errors.borrow().len(), 1);
    endpoint.reject.store(false, Ordering::Relaxed);
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_wheel(&event(0.5), window, cx);
        let outside = ScrollWheelEvent {
            position: point(px(-100.0), px(-100.0)),
            ..event(1.0)
        };
        surface.pointer_wheel(&outside, window, cx);
        surface.pointer_wheel(&event(0.5), window, cx);
        surface.release_pointer(cx);
        surface.pointer_wheel(&event(0.5), window, cx);
    });
    assert_eq!(
        endpoint.wheels.lock().unwrap().len(),
        2,
        "invalid geometry and blur reset fractions; failed steps never retry"
    );
    call(
        &engine,
        Command::Resize {
            cols: 16,
            rows: 3,
            cell_width_px: 9,
            cell_height_px: 18,
        },
    );
    call(&engine, Command::Capture);
    surface.update_in(cx, |surface, window, cx| {
        surface.pointer_wheel(&event(2.0), window, cx)
    });
    assert_eq!(
        endpoint.wheels.lock().unwrap().len(),
        2,
        "stale grid is not input geometry"
    );
    complete(engine.close()).unwrap();
}

#[gpui::test]
fn native_surface_alt_screen_owns_text_and_absent_publication_has_no_modes(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_component::init);
    let engine = AsyncEngine::spawn(16, 3).unwrap();
    let endpoint = Arc::new(Endpoint::new(engine.model()));
    let source = endpoint.clone();
    let (surface, cx) = cx.add_window_view(move |window, cx| {
        NativeTerminalSurface::new(
            source,
            gpui::font("monospace"),
            px(14.0),
            |error, _| panic!("{error:?}"),
            window,
            cx,
        )
    });
    surface.update_in(cx, |surface, window, cx| {
        assert!(!surface.accepts_text_input(window, cx));
        surface.key_down(&down("up", false), window, cx);
        assert!(surface.held.is_empty());
    });
    call(&engine, Command::Feed(b"\x1b[?1049h".to_vec()));
    call(&engine, Command::Capture);
    surface.update_in(cx, |surface, window, cx| {
        assert!(!surface.accepts_text_input(window, cx));
        surface.key_down(&down("a", false), window, cx);
        surface.replace_text_in_range(None, "a", window, cx);
        assert_eq!(surface.held.len(), 1);
    });
    assert!(endpoint.commits.lock().unwrap().is_empty());
    let keys = endpoint.keys.lock().unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].code, KeyCode::UNIDENTIFIED);
    assert_eq!(keys[0].text, "a");
    drop(keys);
    surface.update(cx, |surface, cx| {
        surface.release_held_keys(cx);
        surface.release_held_keys(cx);
        assert!(surface.held.is_empty());
    });
    let keys = endpoint.keys.lock().unwrap();
    assert_eq!(keys.len(), 2, "cleanup sends one release, never retries");
    assert!(matches!(keys[1].action, KeyAction::Release));
    drop(keys);
    complete(engine.close()).unwrap();
}
