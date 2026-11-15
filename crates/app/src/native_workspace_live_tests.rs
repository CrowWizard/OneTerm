//! Process-isolated Windows platform proofs, never production launch options.

use super::*;
use gpui::AppContext as _;
use gpui_component::dock::{DockPlacement, PanelView};
use oneterm_terminal::{CreatedSession, PtySize, TerminalSecurityPolicy};
use oneterm_terminal_view::{PanelSpec, TerminalPanel};

#[test]
fn native_live_workspace_window_close_and_multi_panel_app_quit() {
    const CHILD: &str = "ONETERM_NATIVE_LIVE_CLOSE_PROOF";
    let Ok(mode) = std::env::var(CHILD) else {
        for mode in ["window", "quit"] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "native_workspace::live_tests::native_live_workspace_window_close_and_multi_panel_app_quit", "--nocapture"])
                .env(CHILD, mode).spawn().unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(status.success(), "live {mode} process: {status}");
                    break;
                }
                if Instant::now() >= deadline {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("live {mode} child exceeded watchdog");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        return;
    };
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.tmp")
        .canonicalize()
        .unwrap()
        .join(format!("us0249-live-{mode}-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    oneterm_core::config::isolated::select_root(&directory).unwrap();
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .is_test(true)
        .format(|buffer, record| {
            use std::io::Write as _;

            writeln!(
                buffer,
                "[{} {} {}:{}] {}",
                record.level(),
                record.target(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args()
            )?;
            if record.level() == log::Level::Error
                && record.args().to_string() == "window not found"
            {
                writeln!(
                    buffer,
                    "Stale-window diagnostic stack: {}",
                    std::backtrace::Backtrace::force_capture()
                )?;
            }
            Ok(())
        })
        .try_init();
    let exit = Arc::new(Mutex::new(None));
    let gui_exit = exit.clone();
    let title = format!("OneTerm native live {mode} {}", std::process::id());
    gpui_platform::application()
        .with_assets(crate::CustomAssets)
        .with_quit_mode(gpui::QuitMode::Explicit)
        .run(move |cx| {
            gpui_component::init(cx);
            NativeWorkspaceSessions::install(cx);
            crate::init::init_with_factory(cx, crate::session_factory::build_native());
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
            install_app_quit(cx, gui_exit.clone());
            let mut models = Vec::new();
            let window = cx
                .open_window(gpui::WindowOptions::default(), |window, cx| {
                    window.set_window_title(&title);
                    let workspace = cx.new(|cx| OneTermWorkspace::new(window, cx));
                    let dock = workspace.read(cx).dock_area.clone();
                    // Add actual docked native panels, not independent probe surfaces.
                    for _ in 0..2 {
                        let cfg = oneterm_settings::TerminalSettings::global(cx)
                            .read(cx)
                            .shell
                            .clone();
                        let CreatedSession::Native(session) =
                            crate::session_factory::build_native()
                                .create_local(
                                    cfg,
                                    PtySize::INITIAL,
                                    100,
                                    TerminalSecurityPolicy::default(),
                                    Default::default(),
                                )
                                .unwrap()
                        else {
                            panic!("legacy session in native live proof");
                        };
                        models.push(session.render_model());
                        // Queue accepted controls immediately before pane ownership.
                        for _ in 0..8 {
                            session
                                .key(oneterm_terminal::ghostty::input::KeyEvent::new(
                                    oneterm_terminal::ghostty::input::KeyCode::ENTER,
                                ))
                                .unwrap();
                        }
                        let panel = TerminalPanel::open(
                            PanelSpec::NativeSession {
                                session,
                                title: "Native live proof".into(),
                                duplicate_config: None,
                            },
                            window,
                            cx,
                        );
                        dock.update(cx, |dock, cx| {
                            dock.add_panel_view(
                                Arc::new(panel) as Arc<dyn PanelView>,
                                DockPlacement::Center,
                                None,
                                window,
                                cx,
                            );
                        });
                    }
                    let close_exit = gui_exit.clone();
                    install_window_close(window, cx, close_exit);
                    cx.new(|cx| {
                        cx.on_release(|_, cx| cx.quit()).detach();
                        gpui_component::Root::new(workspace, window, cx)
                    })
                })
                .unwrap();
            if mode == "window" {
                // Exact title plus PID guards the only HWND receiving WM_CLOSE.
                let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
                unsafe {
                    use windows_sys::Win32::UI::WindowsAndMessaging::*;
                    let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
                    assert!(!hwnd.is_null(), "live workspace HWND");
                    let mut pid = 0;
                    GetWindowThreadProcessId(hwnd, &mut pid);
                    assert_eq!(pid, std::process::id());
                    assert_ne!(PostMessageW(hwnd, WM_CLOSE, 0, 0), 0);
                }
            } else {
                // The real GPUI loop invokes the shared observer and enforces
                // SHUTDOWN_TIMEOUT; no test-specific replacement timeout.
                cx.spawn(async move |cx| {
                    let deadline = Instant::now() + Duration::from_secs(10);
                    loop {
                        if models.iter().all(|model| {
                            model.reader().poll().unwrap().frame.is_some_and(|frame| {
                                frame
                                    .source()
                                    .rows
                                    .iter()
                                    .flat_map(|row| &row.cells)
                                    .any(|cell| cell.text.contains('>'))
                            })
                        }) {
                            break;
                        }
                        assert!(Instant::now() < deadline, "live shell readiness");
                        cx.background_executor()
                            .timer(Duration::from_millis(1))
                            .await;
                    }
                    window.update(cx, |_, _, cx| cx.quit()).unwrap();
                })
                .detach();
            }
        });
    assert_eq!(
        *exit.lock().unwrap(),
        Some(Ok(())),
        "live cleanup must be observed"
    );
}
