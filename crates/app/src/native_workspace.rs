//! Explicit native composition root using the actual OneTerm workspace.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context as _, bail};
use gpui::{App, TaskExt as _, Window};
use oneterm_terminal_view::NativeWorkspaceSessions;
use oneterm_workspace::OneTermWorkspace;

type ExitResult = Arc<Mutex<Option<Result<(), String>>>>;

fn prepare_storage(executable: &Path) -> anyhow::Result<PathBuf> {
    let parent = executable
        .parent()
        .context("native executable has no parent directory")?;
    if !parent.is_absolute() {
        bail!("native executable path must be absolute");
    }
    let directory = parent.join("oneterm-native-data");
    oneterm_core::config::isolated::prepare_root(&directory)
        .context("prepare isolated native workspace storage")?;
    Ok(directory)
}

fn record(result: &ExitResult, value: Result<(), String>) {
    if let Err(error) = &value {
        log::error!("Native workspace cleanup failed: {error}");
    }
    let mut current = result
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !matches!(&*current, Some(Err(_))) {
        *current = Some(value);
    }
}

/// A cancelled or unpolled forced-quit observer must never report successful exit.
struct QuitGuard(bool);

impl Drop for QuitGuard {
    fn drop(&mut self) {
        if !self.0 {
            log::error!("Native workspace cleanup incomplete within GPUI app-quit budget");
            log::logger().flush();
            std::process::exit(1);
        }
    }
}

fn install_window_close(window: &mut Window, cx: &mut App, exit: ExitResult) {
    let closing = std::cell::Cell::new(false);
    window.on_window_should_close(cx, move |window, cx| {
        if closing.replace(true) {
            return false;
        }
        begin_window_close(window, cx, exit.clone());
        false
    });
}

fn install_app_quit(cx: &mut App, exit: ExitResult) {
    cx.on_app_quit(move |cx| {
        let guard = QuitGuard(false);
        NativeWorkspaceSessions::shutdown(cx);
        let (failure, completions) = NativeWorkspaceSessions::completion_snapshot(cx);
        let executor = cx.background_executor().clone();
        let exit = exit.clone();
        async move {
            let result = async {
                if let Some(error) = failure {
                    return Err(error);
                }
                loop {
                    let mut pending = false;
                    for completion in &completions {
                        match completion.result() {
                            Some(Ok(_)) => {}
                            Some(Err(error)) => return Err(error.to_string()),
                            None => pending = true,
                        }
                    }
                    if !pending {
                        return Ok(());
                    }
                    executor.timer(Duration::from_millis(1)).await;
                }
            }
            .await;
            let successful = result.is_ok();
            record(&exit, result);
            let mut guard = guard;
            guard.0 = successful;
        }
    })
    .detach();
}

fn begin_window_close(window: &mut Window, cx: &mut App, exit: ExitResult) {
    let deadline = Instant::now() + Duration::from_secs(5);
    NativeWorkspaceSessions::shutdown(cx);
    window
        .spawn(cx, async move |cx| {
            let result = loop {
                match cx.update(|_, cx| NativeWorkspaceSessions::result(cx)) {
                    Ok(Some(result)) => break result,
                    Err(error) => break Err(error.to_string()),
                    Ok(None) if Instant::now() >= deadline => {
                        break Err("native workspace window cleanup timed out".into());
                    }
                    Ok(None) => {
                        cx.background_executor()
                            .timer(Duration::from_millis(1))
                            .await
                    }
                }
            };
            record(&exit, result);
            if let Err(error) = cx.update(|window, _| window.remove_window()) {
                record(
                    &exit,
                    Err(format!("remove native workspace window: {error}")),
                );
            }
        })
        .detach();
}

/// No command-line opt-in, production configuration, or legacy engine fallback.
pub fn run() -> anyhow::Result<()> {
    if let Some(argument) = std::env::args_os().nth(1) {
        bail!("Unexpected native workspace argument: {argument:?}; launch without arguments");
    }
    let elevation = crate::elevation::process_elevation();
    oneterm_core::elevation::set_elevation(elevation);
    let directory = prepare_storage(&std::env::current_exe()?)?;
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();
    log::info!(
        "Native workspace storage: {}; inherited token: {elevation:?}",
        directory.display()
    );
    let exit = Arc::new(Mutex::new(None));
    let gui_exit = exit.clone();
    gpui_platform::application()
        .with_assets(crate::CustomAssets)
        .with_quit_mode(gpui::QuitMode::Explicit)
        .run(move |cx| {
            cx.text_system()
                .add_fonts(vec![
                    Cow::Borrowed(include_bytes!("../fonts/Lilex-Regular.ttf").as_slice()),
                    Cow::Borrowed(include_bytes!("../fonts/Lilex-Bold.ttf").as_slice()),
                    Cow::Borrowed(include_bytes!("../fonts/Lilex-Italic.ttf").as_slice()),
                    Cow::Borrowed(include_bytes!("../fonts/Lilex-BoldItalic.ttf").as_slice()),
                ])
                .expect("embedded Lilex fonts must load");
            gpui_component::init(cx);
            cx.global_mut::<gpui_component::Theme>().mono_font_family = "Lilex".into();
            NativeWorkspaceSessions::install(cx);
            crate::init::init_with_factory(cx, crate::session_factory::build_native());
            OneTermWorkspace::bind_keys(cx);
            install_app_quit(cx, gui_exit.clone());
            let opened = crate::window::open_window_with_effects(
                Vec::new(),
                crate::window::StartupEffects::NativeTest,
                cx,
            );
            cx.spawn(async move |cx| {
                match opened.await {
                    Ok(window) => {
                        let exit = gui_exit.clone();
                        window
                            .update(cx, |_, window, cx| install_window_close(window, cx, exit))?;
                    }
                    Err(error) => {
                        record(
                            &gui_exit,
                            Err(format!("native workspace startup: {error:#}")),
                        );
                        cx.update(|cx| {
                            NativeWorkspaceSessions::shutdown(cx);
                            cx.quit();
                        });
                    }
                }
                anyhow::Ok(())
            })
            .detach_and_log_err(cx);
            cx.activate(true);
        });
    let result = exit
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    match result {
        Some(Ok(())) => Ok(()),
        Some(Err(error)) => bail!(error),
        None => bail!("native workspace exited without observed cleanup"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_startup_cleanup_error_cannot_be_overwritten_by_later_success() {
        let result = Arc::new(Mutex::new(None));
        record(&result, Err("first failure".into()));
        record(&result, Ok(()));
        assert_eq!(*result.lock().unwrap(), Some(Err("first failure".into())));
    }

    #[test]
    fn native_startup_forced_quit_cancellation_is_a_failed_process() {
        const CHILD: &str = "ONETERM_NATIVE_QUIT_GUARD_PROOF";
        if let Ok(mode) = std::env::var(CHILD) {
            if mode == "unpolled" {
                let guard = QuitGuard(false);
                drop(async move {
                    std::future::pending::<()>().await;
                    drop(guard);
                });
            } else if mode == "pending" {
                let guard = QuitGuard(false);
                let mut future = Box::pin(async move {
                    std::future::pending::<()>().await;
                    drop(guard);
                });
                let waker = futures::task::noop_waker();
                let mut context = std::task::Context::from_waker(&waker);
                assert!(std::future::Future::poll(future.as_mut(), &mut context).is_pending());
                drop(future);
            } else {
                drop(QuitGuard(true));
            }
            return;
        }
        for (mode, code) in [("unpolled", 1), ("pending", 1), ("complete", 0)] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "native_workspace::tests::native_startup_forced_quit_cancellation_is_a_failed_process"])
                .env(CHILD, mode).output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(code),
                "{mode}: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[cfg(windows)]
    #[gpui::test]
    fn native_drag_hover_reveals_actual_workspace_rail(cx: &mut gpui::TestAppContext) {
        use gpui::{AppContext as _, Modifiers, MouseButton, point, px};
        use gpui_component::dock::{DockPlacement, PaneRef, panel_handle};
        use oneterm_terminal_view::{PanelSpec, TerminalPanel};

        const CHILD: &str = "ONETERM_NATIVE_RAIL_HOVER_PROOF";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "native_workspace::tests::native_drag_hover_reveals_actual_workspace_rail",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.tmp")
            .canonicalize()
            .unwrap()
            .join(format!("us0249-rail-hover-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        oneterm_core::config::isolated::select_root(&directory).unwrap();
        cx.executor().allow_parking();
        cx.update(gpui_component::init);
        cx.update(NativeWorkspaceSessions::install);
        cx.update(|cx| crate::init::init_with_factory(cx, crate::session_factory::build_native()));
        cx.update(|cx| {
            oneterm_settings::TerminalSettings::global(cx).update(cx, |settings, _| {
                settings.shell = oneterm_core::LocalShellConfig {
                    kind: oneterm_core::ShellKind::Custom,
                    program: Some(
                        format!(
                            "{}\\System32\\cmd.exe",
                            std::env::var("SystemRoot").unwrap()
                        )
                        .into(),
                    ),
                    args: vec!["/D".into(), "/Q".into(), "/K".into()],
                    cwd: Some(directory.clone()),
                    ..Default::default()
                };
            });
            oneterm_settings::UiConfig::set_vertical_tabs_enabled(true, cx);
        });
        let mut area = None;
        let (_root, cx) = cx.add_window_view(|window, cx| {
            let workspace = cx.new(|cx| OneTermWorkspace::new(window, cx));
            let dock = workspace.read(cx).dock_area.clone();
            let panel =
                TerminalPanel::open(PanelSpec::DefaultShell { workspace: None }, window, cx);
            dock.update(cx, |dock, cx| {
                dock.add_panel_view(panel_handle(panel), DockPlacement::Center, None, window, cx)
            });
            area = Some(dock);
            gpui_component::Root::new(workspace, window, cx)
        });
        let area = area.unwrap();
        let tabs = |cx: &mut gpui::VisualTestContext| {
            area.read_with(cx, |dock, _| {
                let mut found = None;
                dock.layout(DockPlacement::Center)
                    .unwrap()
                    .root()
                    .walk(&mut |node| {
                        if let PaneRef::Tabs { panels, active_ix } = node.kind() {
                            found.get_or_insert((panels.to_vec(), active_ix));
                        }
                    });
                found.unwrap()
            })
        };
        cx.run_until_parked();
        let (ids, _) = tabs(cx);
        assert_eq!(ids.len(), 2);
        for collapsed in [false, true] {
            cx.update(|window, cx| {
                oneterm_settings::UiConfig::set_vertical_tabs_collapsed(collapsed, cx);
                area.update(cx, |dock, cx| dock.select_panel(ids[0], window, cx));
            });
            cx.run_until_parked();
            cx.update(|window, cx| window.draw(cx).clear(cx));
            let from = cx.debug_bounds("oneterm-tab-rail-row-0").unwrap().center();
            let to = cx.debug_bounds("oneterm-tab-rail-row-1").unwrap().center();
            cx.simulate_mouse_move(to, None, Modifiers::none());
            assert_eq!(tabs(cx), (ids.clone(), 0), "ordinary hover");
            cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
            cx.simulate_mouse_move(
                from + point(px(0.), px(6.)),
                MouseButton::Left,
                Modifiers::none(),
            );
            assert_eq!(tabs(cx), (ids.clone(), 0), "source hover");
            assert!(cx.update(|_, cx| cx.has_active_drag()), "rail starts drag");
            cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
            assert_eq!(
                tabs(cx),
                (ids.clone(), 1),
                "reveal without reorder, collapsed={collapsed}"
            );
            assert!(
                cx.update(|_, cx| cx.has_active_drag()),
                "original drag remains live"
            );
            // Cancel over the title bar, not a row-drop reorder target.
            cx.simulate_mouse_up(point(px(5.), px(5.)), MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
            assert_eq!(tabs(cx), (ids.clone(), 1));
            // Add an independent source so both rail layouts can prove a
            // real directional move without preparing an empty destination.
            let source_id = cx.update(|window, cx| {
                let panel =
                    TerminalPanel::open(PanelSpec::DefaultShell { workspace: None }, window, cx);
                let id = gpui_component::dock::BasePanelView::panel_id(&panel, cx);
                area.update(cx, |dock, cx| {
                    dock.add_panel_view(
                        panel_handle(panel),
                        DockPlacement::Center,
                        None,
                        window,
                        cx,
                    )
                });
                id
            });
            cx.run_until_parked();
            cx.update(|window, cx| window.draw(cx).clear(cx));
            let from = cx.debug_bounds("oneterm-tab-rail-row-2").unwrap().center();
            let title = cx.debug_bounds("oneterm-tab-rail-row-1").unwrap().center();
            cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
            cx.simulate_mouse_move(
                from + point(px(0.), px(6.)),
                MouseButton::Left,
                Modifiers::none(),
            );
            cx.simulate_mouse_move(title, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
            cx.update(|window, cx| window.draw(cx).clear(cx));
            let bounds = cx.debug_bounds("terminal-split-target").unwrap();
            let to = point(
                bounds.left() + bounds.size.width * 0.9,
                bounds.top() + bounds.size.height * 0.5,
            );
            cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
            cx.update(|window, cx| window.draw(cx).clear(cx));
            assert_eq!(
                cx.debug_bounds("terminal-split-target").unwrap(),
                bounds,
                "preview does not resize the terminal"
            );
            cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
            cx.update(|window, cx| window.draw(cx).clear(cx));
            assert!(
                cx.debug_bounds("terminal-split-target").unwrap().size.width < bounds.size.width,
                "release creates real split geometry"
            );
            assert!(area.read_with(cx, |dock, _| dock.panel(source_id).is_none()));
            assert_eq!(tabs(cx), (ids.clone(), 1));
        }
        let observed = Arc::new(Mutex::new(None));
        let close_result = observed.clone();
        cx.update(|window, cx| begin_window_close(window, cx, close_result));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            cx.executor().advance_clock(Duration::from_millis(1));
            cx.run_until_parked();
            if let Some(result) = observed.lock().unwrap().clone() {
                result.unwrap();
                break;
            }
            assert!(Instant::now() < deadline, "native rail cleanup deadline");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[cfg(windows)]
    #[gpui::test]
    fn native_startup_actual_workspace_uses_native_factory_and_isolated_storage(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::AppContext as _;
        use oneterm_core::{LocalShellConfig, SshConfig, SshKeepaliveConfig, TerminalLogConfig};
        use oneterm_terminal::{
            CreatedSession, PtySize, SessionFactory, TerminalSecurityPolicy, TerminalSession,
        };
        use std::sync::atomic::{AtomicUsize, Ordering};

        const CHILD: &str = "ONETERM_NATIVE_WORKSPACE_PROOF";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "native_workspace::tests::native_startup_actual_workspace_uses_native_factory_and_isolated_storage", "--nocapture"])
                .env(CHILD, "1").output().unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.tmp")
            .canonicalize()
            .unwrap()
            .join(format!("us0249-workspace-proof-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        oneterm_core::config::isolated::select_root(&directory).unwrap();
        assert_eq!(
            oneterm_core::config_dir(),
            directory.canonicalize().unwrap()
        );

        struct Factory(
            Arc<dyn SessionFactory>,
            Arc<AtomicUsize>,
            Arc<Mutex<Vec<oneterm_terminal::ghostty_model::SharedModel>>>,
        );
        impl SessionFactory for Factory {
            fn create_local(
                &self,
                cfg: LocalShellConfig,
                size: PtySize,
                history: usize,
                security: TerminalSecurityPolicy,
                logging: TerminalLogConfig,
            ) -> oneterm_core::Result<CreatedSession> {
                let session = self.0.create_local(cfg, size, history, security, logging)?;
                assert!(matches!(session, CreatedSession::Native(_)));
                if let CreatedSession::Native(native) = &session {
                    self.2.lock().unwrap().push(native.render_model());
                }
                self.1.fetch_add(1, Ordering::Relaxed);
                Ok(session)
            }
            fn spawn_local(
                &self,
                _: LocalShellConfig,
                _: PtySize,
                _: usize,
                _: TerminalSecurityPolicy,
                _: TerminalLogConfig,
            ) -> oneterm_core::Result<Box<dyn TerminalSession>> {
                panic!("legacy startup")
            }
            fn connect_ssh(
                &self,
                _: SshConfig,
                _: PtySize,
                _: usize,
                _: TerminalSecurityPolicy,
                _: TerminalLogConfig,
                _: SshKeepaliveConfig,
            ) -> oneterm_core::Result<Box<dyn TerminalSession>> {
                panic!("automatic SSH startup")
            }
        }
        let count = Arc::new(AtomicUsize::new(0));
        let models = Arc::new(Mutex::new(Vec::new()));
        cx.executor().allow_parking();
        cx.update(gpui_component::init);
        cx.update(NativeWorkspaceSessions::install);
        cx.update(|cx| {
            crate::init::init_with_factory(
                cx,
                Arc::new(Factory(
                    crate::session_factory::build_native(),
                    count.clone(),
                    models.clone(),
                )),
            )
        });
        cx.update(|cx| {
            oneterm_settings::TerminalSettings::global(cx).update(cx, |settings, _| {
                settings.shell = LocalShellConfig {
                    kind: oneterm_core::ShellKind::Custom,
                    program: Some(
                        format!(
                            "{}\\System32\\cmd.exe",
                            std::env::var("SystemRoot").unwrap()
                        )
                        .into(),
                    ),
                    args: vec![
                        "/D".into(),
                        "/Q".into(),
                        "/K".into(),
                        "echo NATIVE_WORKSPACE_READY".into(),
                    ],
                    cwd: Some(directory.clone()),
                    ..Default::default()
                };
            })
        });
        let (_root, window) = cx.add_window_view(|window, cx| {
            let workspace = cx.new(|cx| OneTermWorkspace::new(window, cx));
            gpui_component::Root::new(workspace, window, cx)
        });
        window.run_until_parked();
        assert_eq!(
            count.load(Ordering::Relaxed),
            1,
            "actual workspace's initial terminal must be native"
        );
        // Close immediately after initial layout: no ready-output or quiet
        // delay is allowed to conceal the startup/publication race.
        let observed = Arc::new(Mutex::new(None));
        let close_result = observed.clone();
        window.update(|window, cx| begin_window_close(window, cx, close_result));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            window.executor().advance_clock(Duration::from_millis(1));
            window.run_until_parked();
            if let Some(result) = observed.lock().unwrap().clone() {
                result.unwrap();
                break;
            }
            assert!(
                Instant::now() < deadline,
                "actual native workspace teardown deadline"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        window.run_until_parked();
    }
}

#[cfg(all(test, windows))]
#[path = "native_workspace_live_tests.rs"]
mod live_tests;
