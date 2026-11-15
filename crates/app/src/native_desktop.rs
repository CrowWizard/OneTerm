//! Isolated local native acceptance window and its event/lifecycle owner.

use std::borrow::Cow;
use std::io::Write as _;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context as _, bail};
use gpui::{
    AppContext as _, Context, Entity, Focusable as _, IntoElement, ParentElement as _, Render,
    Styled as _, Task, Window, WindowOptions, div, px,
};
use gpui_component::{ActiveTheme as _, Root};
use oneterm_core::LocalShellConfig;
use oneterm_local_shell::NativeLocalSession;
use oneterm_terminal::{PtySize, SessionEvent, TerminalLifecycle as _, TerminalSecurityPolicy};
use oneterm_terminal_view::NativeTerminalSurface;

const TITLE: &str = "OneTerm Native Desktop Probe (not production)";
const BANNER: &str = "LOCAL / NATIVE ONLY - program clipboard and notifications denied";

fn validate_arguments(args: impl IntoIterator<Item = std::ffi::OsString>) -> anyhow::Result<()> {
    if let Some(arg) = args.into_iter().next() {
        bail!("Unexpected native desktop argument: {arg:?}; launch without arguments");
    }
    Ok(())
}

#[derive(Default)]
struct Observation {
    dirty: bool,
    lifecycle: String,
    denied: u64,
    error: String,
    stream_ended: bool,
    cleanup_timed_out: bool,
    window_close_started: bool,
    app_quit_started: bool,
    quit_cleanup_complete: bool,
}

impl Observation {
    fn begin_app_quit(&mut self) -> bool {
        if self.window_close_started {
            return false;
        }
        self.app_quit_started = true;
        true
    }

    fn check_cleanup(&self) -> anyhow::Result<()> {
        if self.cleanup_timed_out {
            bail!("Native desktop surface cleanup timed out before GUI exit");
        }
        Ok(())
    }

    fn event(&mut self, event: SessionEvent) {
        self.dirty = true;
        match event {
            SessionEvent::Clipboard(_)
            | SessionEvent::ClipboardRead
            | SessionEvent::Notification(_) => {
                // The probe never lets program output touch the OS clipboard or
                // notification system, including local OSC requests.
                self.denied = self.denied.saturating_add(1);
            }
            SessionEvent::Exited(code) => self.lifecycle = format!("exited: {code:?}"),
            SessionEvent::Closed => {
                if !self.lifecycle.ends_with("; closed") {
                    self.lifecycle.push_str("; closed");
                }
            }
            SessionEvent::Output
            | SessionEvent::Title(_)
            | SessionEvent::Cwd(_)
            | SessionEvent::ShellIntegration(_)
            | SessionEvent::Progress(_)
            | SessionEvent::ProgramStatus(_)
            | SessionEvent::Bell => {}
        }
    }
}

fn consume_events(
    events: async_channel::Receiver<SessionEvent>,
    observation: Arc<Mutex<Observation>>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("native-desktop-events".into())
        .spawn(move || {
            while let Ok(event) = events.recv_blocking() {
                observation
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .event(event);
            }
            {
                let mut state = observation
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state.stream_ended = true;
                state.dirty = true;
            }
            log::info!("Native desktop event consumer ended");
        })
}

/// Created before polling the quit future: even an unpolled cancellation fails.
struct QuitFailure<F: FnOnce()> {
    failure: Option<F>,
}

impl<F: FnOnce()> QuitFailure<F> {
    fn new(failure: F) -> Self {
        Self {
            failure: Some(failure),
        }
    }

    fn complete(mut self) {
        self.failure.take();
    }
}

impl<F: FnOnce()> Drop for QuitFailure<F> {
    fn drop(&mut self) {
        if let Some(failure) = self.failure.take() {
            failure();
        }
    }
}

struct ShutdownOwner {
    session: Arc<NativeLocalSession>,
    consumer: std::thread::JoinHandle<()>,
}

fn complete_owner(
    owner: ShutdownOwner,
    observation: &Mutex<Observation>,
    wait_for_references: bool,
) -> anyhow::Result<()> {
    let ShutdownOwner { session, consumer } = owner;
    let close_result = session.close();
    if wait_for_references {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while Arc::strong_count(&session) > 1 {
            if std::time::Instant::now() >= deadline {
                bail!("Native desktop app quit retained session references");
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    let session = Arc::try_unwrap(session)
        .map_err(|_| anyhow::anyhow!("Native desktop GUI retained a session after exit"))?;
    let result = futures::executor::block_on(session.wait());
    consumer
        .join()
        .map_err(|_| anyhow::anyhow!("Native desktop event consumer panicked"))?;
    log::info!("Native desktop owner destroyed: {:?}", result?);
    close_result?;
    observation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .check_cleanup()
}

fn quit_cleanup(
    owner: ShutdownOwner,
    observation: Arc<Mutex<Observation>>,
) -> std::io::Result<async_channel::Receiver<anyhow::Result<()>>> {
    let (sender, receiver) = async_channel::bounded(1);
    std::thread::Builder::new()
        .name("native-desktop-quit".into())
        .spawn(move || {
            let result = complete_owner(owner, &observation, true);
            if sender.send_blocking(result).is_err() {
                // The quit failure guard already reports cancelled observation.
                log::error!("Native desktop quit result receiver closed");
            }
        })?;
    Ok(receiver)
}

struct Desktop {
    surface: Entity<NativeTerminalSurface>,
    observation: Arc<Mutex<Observation>>,
    _refresh: Task<()>,
}

impl Desktop {
    fn new(
        session: Arc<NativeLocalSession>,
        observation: Arc<Mutex<Observation>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let errors = observation.clone();
        let surface = cx.new(|cx| {
            NativeTerminalSurface::new(
                session.clone(),
                gpui::font("Lilex"),
                px(15.),
                move |error, _| {
                    log::error!("Native desktop input: {error:?}");
                    let mut state = errors
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    state.error = format!("{error:?}").chars().take(1024).collect();
                    state.dirty = true;
                },
                window,
                cx,
            )
        });
        surface.read(cx).focus_handle(cx).focus(window, cx);
        let shutdown_observation = observation.clone();
        let released_surface = surface.downgrade();
        cx.on_release(move |_, cx| {
            log::info!("Native desktop root released; requesting session stop");
            let app_quit = {
                let mut state = shutdown_observation
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state.window_close_started = true;
                state.app_quit_started
            };
            if let Err(error) = session.close() {
                log::error!("Native desktop window close: {error}");
            }
            if app_quit {
                // The app-quit observer owns completion; do not quit recursively.
                return;
            }
            // Keep the foreground executor alive for detached search/drag cleanup.
            // The launcher and this close task are the two intentional owners.
            cx.spawn(async move |cx| {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                while Arc::strong_count(&session) > 2 {
                    if std::time::Instant::now() >= deadline {
                        log::error!(
                            "Native desktop cleanup timed out: {} session references remain; surface entity alive: {}",
                            Arc::strong_count(&session),
                            released_surface.upgrade().is_some(),
                        );
                        shutdown_observation
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .cleanup_timed_out = true;
                        break;
                    }
                    cx.background_executor()
                        .timer(Duration::from_millis(10))
                        .await;
                }
                cx.update(|cx| cx.quit());
            })
            .detach();
        })
        .detach();
        let poll = observation.clone();
        let refresh = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let dirty = {
                    let mut state = poll
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    std::mem::take(&mut state.dirty)
                };
                if dirty
                    && this
                        .update(cx, |view, cx| {
                            view.surface
                                .update(cx, |surface, cx| surface.invalidate(cx));
                            cx.notify();
                        })
                        .is_err()
                {
                    break;
                }
            }
        });
        Self {
            surface,
            observation,
            _refresh: refresh,
        }
    }
}

impl Render for Desktop {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self
            .observation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let status = format!(
            "{} | event stream ended: {} | denied OSC effects: {} | {}",
            state.lifecycle, state.stream_ended, state.denied, state.error
        );
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(div().p_2().child(BANNER))
            .child(div().flex_1().min_h_0().child(self.surface.clone()))
            .child(div().p_2().child(status))
    }
}

fn request_gpui_window_close(window: &mut Window) -> bool {
    // Let GPUI own HWND destruction after removing its root;
    // default WM_CLOSE would destroy the native handle first.
    window.remove_window();
    false
}

/// Launch only the native local acceptance surface, without application persistence.
pub fn run() -> anyhow::Result<()> {
    validate_arguments(std::env::args_os().skip(1))?;
    let elevation = crate::elevation::process_elevation();
    // Record the real token for underlying policies, not as a launch gate.
    oneterm_core::elevation::set_elevation(elevation);
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buffer, record| {
            let source = match (record.file(), record.line()) {
                (Some(file), Some(line)) => format!("{file}:{line}"),
                (Some(file), None) => file.to_owned(),
                (None, Some(line)) => format!("line {line}"),
                (None, None) => "unknown source".to_owned(),
            };
            writeln!(
                buffer,
                "{} {} [{}] {}: {}",
                buffer.timestamp(),
                record.level(),
                record.target(),
                source,
                record.args()
            )
        })
        .init();
    log::info!("Native desktop starting with inherited process token: {elevation:?}");
    let config = LocalShellConfig {
        cwd: Some(std::env::current_dir()?),
        ..Default::default()
    };
    let session = Arc::new(NativeLocalSession::spawn(
        config,
        PtySize::INITIAL,
        TerminalSecurityPolicy::default(),
    )?);
    let events = session
        .take_events()
        .context("Native desktop event receiver already taken")?;
    let observation = Arc::new(Mutex::new(Observation {
        lifecycle: "running".into(),
        ..Default::default()
    }));
    let consumer = consume_events(events, observation.clone())?;
    let shutdown_observation = observation.clone();
    let gui_session = session.clone();
    let owner = Arc::new(Mutex::new(Some(ShutdownOwner { session, consumer })));
    let quit_owner = owner.clone();
    let quit_observation = observation.clone();
    let (startup_sender, startup_receiver) = std::sync::mpsc::sync_channel(1);
    gpui_platform::application()
        .with_assets(crate::CustomAssets)
        .with_quit_mode(gpui::QuitMode::Explicit)
        .run(move |cx| {
            cx.on_app_quit(move |_| {
                let app_quit = quit_observation
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .begin_app_quit();
                let pending = app_quit.then(|| {
                    let guard = QuitFailure::new(|| {
                        log::error!(
                            "Native desktop app quit cleanup incomplete or cancelled within GPUI's quit budget; refusing successful exit"
                        );
                        log::logger().flush();
                        std::process::exit(1);
                    });
                    let owner = quit_owner
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .take()
                        .context("Native desktop quit owner already taken");
                    let receiver = owner.and_then(|owner| {
                        quit_cleanup(owner, quit_observation.clone()).map_err(Into::into)
                    });
                    (guard, receiver, quit_observation.clone())
                });
                async move {
                    if let Some((guard, receiver, observation)) = pending {
                        let result = async {
                            receiver?
                                .recv()
                                .await
                                .context("Native desktop quit worker lost")?
                        }
                        .await;
                        match result {
                            Ok(()) => {
                                observation
                                    .lock()
                                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                                    .quit_cleanup_complete = true;
                                log::info!("Native desktop app quit cleanup complete");
                                guard.complete();
                            }
                            Err(error) => {
                                log::error!("Native desktop app quit cleanup: {error:#}");
                                drop(guard);
                            }
                        }
                    }
                }
            })
            .detach();
            cx.text_system()
                .add_fonts(vec![Cow::Borrowed(
                    include_bytes!("../fonts/Lilex-Regular.ttf").as_slice(),
                )])
                .expect("Embedded Lilex font must load");
            gpui_component::init(cx);
            cx.global_mut::<gpui_component::Theme>().mono_font_family = "Lilex".into();
            let result = cx.open_window(WindowOptions::default(), |window, cx| {
                window.set_window_title(TITLE);
                window.on_window_should_close(cx, |window, _| request_gpui_window_close(window));
                let desktop = cx.new(|cx| Desktop::new(gui_session, observation, window, cx));
                cx.new(|cx| Root::new(desktop, window, cx))
            });
            if let Err(error) = &result {
                log::error!("Native desktop window creation: {error}");
                cx.quit();
            }
            if startup_sender.send(result.map(|_| ())).is_err() {
                log::error!("Native desktop startup result receiver closed");
                cx.quit();
            }
            cx.activate(true);
        });
    let pending = owner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    if let Some(owner) = pending {
        complete_owner(owner, &shutdown_observation, false)?;
    } else if !shutdown_observation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .quit_cleanup_complete
    {
        bail!("Native desktop quit cleanup did not complete");
    }
    startup_receiver
        .recv()
        .context("Native desktop startup did not report a result")??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_standalone_fixture_remains_type_checked() {
        let _entry: fn() -> anyhow::Result<()> = run;
    }

    #[test]
    fn quit_failure_guard_covers_unpolled_and_pending_cancellation() {
        use std::cell::Cell;
        let failures = Cell::new(0);
        let guard = QuitFailure::new(|| failures.set(failures.get() + 1));
        drop(async move {
            std::future::pending::<()>().await;
            guard.complete();
        });
        assert_eq!(failures.get(), 1, "unpolled quit must not report success");
        let guard = QuitFailure::new(|| failures.set(failures.get() + 1));
        let mut pending = Box::pin(async move {
            std::future::pending::<()>().await;
            guard.complete();
        });
        assert!(
            std::future::Future::poll(
                pending.as_mut(),
                &mut std::task::Context::from_waker(std::task::Waker::noop()),
            )
            .is_pending()
        );
        drop(pending);
        assert_eq!(failures.get(), 2, "pending quit must not report success");
        QuitFailure::new(|| failures.set(failures.get() + 1)).complete();
        assert_eq!(failures.get(), 2, "observed completion disarms failure");
    }

    #[test]
    fn app_quit_and_window_close_have_distinct_completion_owners() {
        let mut normal = Observation {
            window_close_started: true,
            ..Default::default()
        };
        assert!(!normal.begin_app_quit());
        assert!(!normal.app_quit_started);
        let mut session_end = Observation::default();
        assert!(session_end.begin_app_quit());
        assert!(session_end.app_quit_started);
        assert!(!session_end.quit_cleanup_complete);
    }

    #[cfg(windows)]
    #[gpui::test]
    fn retained_session_times_out_without_erasing_the_failure(cx: &mut gpui::TestAppContext) {
        cx.executor().allow_parking();
        cx.update(gpui_component::init);
        let session = Arc::new(
            NativeLocalSession::spawn(
                LocalShellConfig {
                    cwd: Some(std::env::current_dir().unwrap()),
                    ..Default::default()
                },
                PtySize::INITIAL,
                TerminalSecurityPolicy::default(),
            )
            .unwrap(),
        );
        let state = Arc::new(Mutex::new(Observation::default()));
        let consumer = consume_events(session.take_events().unwrap(), state.clone()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(25);
        while session.model().query().viewport.is_none() {
            assert!(
                std::time::Instant::now() < deadline,
                "fault fixture publication deadline"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let retained = session.clone();
        let source = session.clone();
        let observed = state.clone();
        let (root, window) = cx.add_window_view(move |window, cx| {
            let desktop = cx.new(|cx| Desktop::new(source, observed, window, cx));
            Root::new(desktop, window, cx)
        });
        let weak = root.downgrade();
        window.update(|window, _| {
            assert!(!request_gpui_window_close(window));
        });
        window.cx.update(|_| drop(root));
        window.run_until_parked();
        assert!(weak.upgrade().is_none());
        assert!(!state.lock().unwrap().cleanup_timed_out);
        let started = std::time::Instant::now();
        let deadline = started + Duration::from_secs(15);
        while Arc::strong_count(&session) > 2 {
            window.executor().advance_clock(Duration::from_millis(20));
            window.run_until_parked();
            assert!(
                std::time::Instant::now() < deadline,
                "fault fixture cleanup deadline"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            started.elapsed() >= Duration::from_secs(4),
            "must exercise the actual five-second wait"
        );
        assert_eq!(
            Arc::strong_count(&session),
            2,
            "close task must release, not wait forever"
        );
        assert!(
            state
                .lock()
                .unwrap()
                .check_cleanup()
                .unwrap_err()
                .to_string()
                .contains("timed out")
        );
        drop(retained);
        let session = Arc::try_unwrap(session).ok().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = futures::executor::block_on(session.wait());
            consumer.join().unwrap();
            sender.send(result).unwrap();
        });
        assert_eq!(
            receiver
                .recv_timeout(Duration::from_secs(25))
                .unwrap()
                .unwrap()
                .stop,
            oneterm_local_shell::ghostty_local::Stop::Requested
        );
        let state = state.lock().unwrap();
        assert!(state.stream_ended);
        assert!(
            state.check_cleanup().is_err(),
            "late cleanup must not turn timeout into success"
        );
    }

    #[cfg(windows)]
    #[gpui::test]
    fn window_release_reclaims_idle_and_pending_search_sessions(cx: &mut gpui::TestAppContext) {
        cx.executor().allow_parking();
        cx.update(gpui_component::init);
        for pending_search in [false, true] {
            let session = Arc::new(
                NativeLocalSession::spawn(
                    LocalShellConfig {
                        cwd: Some(std::env::current_dir().unwrap()),
                        ..Default::default()
                    },
                    PtySize::INITIAL,
                    TerminalSecurityPolicy::default(),
                )
                .unwrap(),
            );
            let state = Arc::new(Mutex::new(Observation::default()));
            let consumer = consume_events(session.take_events().unwrap(), state.clone()).unwrap();
            let observed = state.clone();
            let source = session.clone();
            let (root, window) = cx.add_window_view(move |window, cx| {
                let desktop = cx.new(|cx| Desktop::new(source, state, window, cx));
                Root::new(desktop, window, cx)
            });
            let desktop = root.read_with(window, |root, _| {
                root.view().clone().downcast::<Desktop>().ok().unwrap()
            });
            let deadline = std::time::Instant::now() + Duration::from_secs(25);
            while session.model().query().viewport.is_none() {
                window.run_until_parked();
                assert!(
                    std::time::Instant::now() < deadline,
                    "window publication deadline"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            if pending_search {
                window.simulate_keystrokes("ctrl-f");
                desktop.update_in(window, |desktop, window, cx| {
                    assert!(
                        !desktop.surface.read(cx).focus_handle(cx).is_focused(window),
                        "Ctrl+F must move focus into the search input"
                    );
                });
                window.simulate_input("pending-window-search");
            }
            let weak = desktop.downgrade();
            window.update(|window, _| {
                assert!(!request_gpui_window_close(window));
            });
            window.cx.update(|_| {
                drop(desktop);
                drop(root);
            });
            window.run_until_parked();
            assert!(weak.upgrade().is_none(), "window root must release");
            if pending_search {
                assert!(
                    Arc::strong_count(&session) > 1,
                    "pending search must exercise asynchronous reference drain"
                );
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(25);
            while Arc::strong_count(&session) != 1 {
                window.executor().advance_clock(Duration::from_millis(200));
                window.run_until_parked();
                assert!(
                    std::time::Instant::now() < deadline,
                    "window session reference deadline"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            let session = Arc::try_unwrap(session).ok().unwrap();
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let result = futures::executor::block_on(session.wait());
                consumer.join().unwrap();
                sender.send(result).unwrap();
            });
            assert_eq!(
                receiver
                    .recv_timeout(Duration::from_secs(25))
                    .unwrap()
                    .unwrap()
                    .stop,
                oneterm_local_shell::ghostty_local::Stop::Requested
            );
            let observed = observed.lock().unwrap();
            assert!(observed.stream_ended);
            assert!(observed.check_cleanup().is_ok());
        }
    }

    #[test]
    fn no_argument_startup_needs_no_permission_opt_in() {
        // Validation has no token input: all classifications use the same entry.
        assert!(validate_arguments(Vec::<std::ffi::OsString>::new()).is_ok());
        assert!(!TITLE.contains("ADMIN"));
        assert!(!BANNER.contains("ADMIN"));
    }

    #[test]
    fn unexpected_arguments_are_rejected_including_the_removed_opt_in() {
        let args = |values: &[&str]| {
            values
                .iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>()
        };
        for invalid in [
            vec!["--allow-elevated-test"],
            vec!["--allow-elevated-test", "--allow-elevated-test"],
            vec!["--allow-elevated-test=true"],
            vec!["--allow-elevated"],
            vec!["command"],
            vec!["--allow-elevated-test", "command"],
        ] {
            assert!(validate_arguments(args(&invalid)).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn observation_denies_program_effects_and_keeps_bounded_state() {
        let mut state = Observation::default();
        for event in [
            SessionEvent::Clipboard(Some("secret".into())),
            SessionEvent::ClipboardRead,
            SessionEvent::Notification("notice".into()),
        ] {
            state.event(event);
        }
        assert_eq!(state.denied, 3);
        assert!(state.dirty);
        state.event(SessionEvent::Title("untrusted".into()));
        state.event(SessionEvent::Exited(Some(7)));
        state.event(SessionEvent::Closed);
        assert_eq!(state.lifecycle, "exited: Some(7); closed");
        assert!(state.error.is_empty());
        for _ in 0..1000 {
            state.event(SessionEvent::Closed);
        }
        assert_eq!(state.lifecycle, "exited: Some(7); closed");
    }

    #[test]
    fn reliable_event_consumer_drains_without_a_gui() {
        let (sender, receiver) = async_channel::bounded(1);
        let observation = Arc::new(Mutex::new(Observation::default()));
        let consumer = consume_events(receiver, observation.clone()).unwrap();
        for _ in 0..1000 {
            sender.send_blocking(SessionEvent::ClipboardRead).unwrap();
        }
        sender.send_blocking(SessionEvent::Exited(Some(0))).unwrap();
        sender.send_blocking(SessionEvent::Closed).unwrap();
        drop(sender);
        consumer.join().unwrap();
        let state = observation.lock().unwrap();
        assert_eq!(state.denied, 1000);
        assert!(state.stream_ended);
        assert_eq!(state.lifecycle, "exited: Some(0); closed");
    }

    #[cfg(windows)]
    #[test]
    fn real_local_close_observes_owner_and_consumer_completion() {
        let session = NativeLocalSession::spawn(
            LocalShellConfig {
                cwd: Some(std::env::current_dir().unwrap()),
                ..Default::default()
            },
            PtySize::INITIAL,
            TerminalSecurityPolicy::default(),
        )
        .unwrap();
        let observation = Arc::new(Mutex::new(Observation::default()));
        let consumer = consume_events(session.take_events().unwrap(), observation).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(25);
        while session.model().query().viewport.is_none() {
            assert!(
                std::time::Instant::now() < deadline,
                "native publication deadline"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        session.close().unwrap();
        // Bound the proof rather than leave the test process parked on a lost owner.
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = futures::executor::block_on(session.wait());
            consumer.join().unwrap();
            sender.send(result).unwrap();
        });
        let result = receiver
            .recv_timeout(Duration::from_secs(25))
            .unwrap()
            .unwrap();
        assert_eq!(
            result.stop,
            oneterm_local_shell::ghostty_local::Stop::Requested
        );
    }
}
