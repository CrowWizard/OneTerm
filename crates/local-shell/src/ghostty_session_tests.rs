use std::future::Future;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use oneterm_core::ShellKind;
use oneterm_terminal::ghostty::input::KeyCode;
use oneterm_terminal::ghostty_bridge::State;

use super::*;

#[test]
fn native_shell_startup_retains_application_theme_defaults() {
    use oneterm_terminal::ghostty::{colors::DefaultColors, render::Rgb};
    let _spawns = crate::spawn_guard::lock_spawns();
    for kind in [ShellKind::Cmd, ShellKind::PowerShell, ShellKind::Pwsh] {
        let cfg = LocalShellConfig {
            kind,
            ..Default::default()
        };
        if oneterm_core::config::resolve_shell(&cfg).is_err() {
            continue;
        }
        let session =
            NativeLocalSession::spawn(cfg, PtySize::INITIAL, TerminalSecurityPolicy::default())
                .unwrap();
        let events = session.take_events().unwrap();
        let consumer = std::thread::spawn(move || while events.recv_blocking().is_ok() {});
        let colors = DefaultColors {
            foreground: Rgb {
                r: 210,
                g: 211,
                b: 212,
            },
            background: Rgb {
                r: 24,
                g: 25,
                b: 26,
            },
            cursor: None,
            palette: [Rgb {
                r: 180,
                g: 181,
                b: 182,
            }; 256],
        };
        complete(session.set_default_colors(colors.clone())).unwrap();
        until(|| session.endpoint.state().cwd().is_some());
        let viewport = session.model().query().viewport.unwrap();
        let result = (viewport.foreground(), viewport.background());
        let marker = format!("THEME_{kind:?}_PROBE");
        let command = if kind == ShellKind::Cmd {
            format!("echo {marker}\r")
        } else {
            format!("[Console]::WriteLine(('THEME_' + '{kind:?}_PROBE'))\r")
        };
        session.write(command.as_bytes()).unwrap();
        until(|| {
            let frame = session.model().reader().poll().unwrap().frame.unwrap();
            frame.source().rows.iter().any(|row| {
                row.cells
                    .iter()
                    .map(|cell| cell.text.as_str())
                    .collect::<String>()
                    .trim()
                    == marker
            })
        });
        let frame = session.model().reader().poll().unwrap().frame.unwrap();
        let row_index = frame
            .source()
            .rows
            .iter()
            .position(|row| {
                row.cells
                    .iter()
                    .map(|cell| cell.text.as_str())
                    .collect::<String>()
                    .trim()
                    == marker
            })
            .expect("complete marker-only result row");
        session.close().unwrap();
        complete(session.wait()).unwrap();
        consumer.join().unwrap();
        let row = frame.row(row_index).unwrap();
        for (column, cell) in row.cells().iter().take(marker.len()).enumerate() {
            let style = row.style_of(cell);
            let rgb = |color: Rgb| {
                oneterm_terminal::Color::Rgb(oneterm_terminal::Rgb {
                    r: color.r,
                    g: color.g,
                    b: color.b,
                })
            };
            assert_eq!(
                style.fg,
                rgb(colors.foreground),
                "{kind:?} result foreground"
            );
            assert_eq!(
                style.bg,
                rgb(colors.background),
                "{kind:?} result background"
            );
            assert_eq!(row.native_foreground_is_exact(column), Some(false));
        }
        assert_eq!(
            result,
            (colors.foreground, colors.background),
            "{kind:?} startup changed theme"
        );
    }
}

#[test]
fn native_powershell_startup_directory_matches_osc7_publication() {
    let _spawns = crate::spawn_guard::lock_spawns();
    let expected = std::env::current_dir().unwrap();
    let mut kinds = vec![ShellKind::PowerShell];
    if oneterm_core::config::resolve_shell(&LocalShellConfig {
        kind: ShellKind::Pwsh,
        ..Default::default()
    })
    .is_ok()
    {
        kinds.push(ShellKind::Pwsh);
    }
    for kind in kinds {
        let session = NativeLocalSession::spawn(
            LocalShellConfig {
                kind,
                cwd: Some(expected.clone()),
                ..Default::default()
            },
            PtySize::INITIAL,
            TerminalSecurityPolicy::default(),
        )
        .unwrap();
        let events = session.take_events().unwrap();
        let (cwd_tx, cwd_rx) = std::sync::mpsc::channel();
        let consumer = std::thread::spawn(move || {
            while let Ok(event) = events.recv_blocking() {
                if let SessionEvent::Cwd(path) = event {
                    let _ = cwd_tx.send(path);
                }
            }
        });
        let actual = cwd_rx.recv_timeout(Duration::from_secs(15)).unwrap();
        assert_eq!(actual, expected, "{kind:?} initial OSC7 directory");
        session
            .write(b"[Console]::WriteLine('ACTUAL_CWD=' + (Get-Location).Path)\r")
            .unwrap();
        until(|| text(&session.model()).contains(&format!("ACTUAL_CWD={}", expected.display())));
        session.close().unwrap();
        complete(session.wait()).unwrap();
        consumer.join().unwrap();
    }
}

#[test]
fn native_local_clear_command_receives_exact_shell_line_and_clears_selection() {
    use oneterm_terminal::ghostty::selection::Update;
    use oneterm_terminal::ghostty_session::SurfaceSession;
    let _spawns = crate::spawn_guard::lock_spawns();
    let session = NativeLocalSession::spawn(
        config("[Console]::Write([char]27 + '[?2004hCLEAR_READY'); $a=[Console]::ReadLine(); [Console]::Write('RECEIVED:' + $a); $b=[Console]::ReadLine(); exit 0"),
        PtySize::INITIAL, TerminalSecurityPolicy::default()).unwrap();
    let events = session.take_events().unwrap();
    let consumer = std::thread::spawn(move || while events.recv_blocking().is_ok() {});
    let model = session.model();
    until(|| text(&model).contains("CLEAR_READY"));
    complete(session.selection(Update::All)).unwrap();
    assert!(complete(model.selection_text()).unwrap().is_some());
    complete(session.clear_command()).unwrap();
    until(|| text(&model).contains("RECEIVED:clear"));
    assert_eq!(complete(model.selection_text()).unwrap(), None);
    session.close().unwrap();
    assert!(matches!(
        complete(session.clear_command()),
        Err(oneterm_terminal::backend::AsyncGhosttyPumpError::Transport(
            TerminalError::Closed
        ))
    ));
    complete(session.wait()).unwrap();
    consumer.join().unwrap();
}

#[test]
fn configured_native_session_applies_zero_history_before_shell_output() {
    let _spawns = crate::spawn_guard::lock_spawns();
    let session = NativeLocalSession::spawn_configured(
        config("1..20 | ForEach-Object { [Console]::WriteLine('history') }; [Console]::Write('CONFIGURED_READY'); $a=[Console]::ReadLine(); exit 0"),
        PtySize { cols: 80, rows: 3 },
        0,
        TerminalSecurityPolicy::default(),
        oneterm_core::TerminalLogConfig::default(),
    ).unwrap();
    let events = session.take_events().unwrap();
    let consumer = std::thread::spawn(move || while events.recv_blocking().is_ok() {});
    until(|| text(&session.model()).contains("CONFIGURED_READY"));
    assert_eq!(
        session
            .model()
            .query()
            .viewport
            .unwrap()
            .metadata()
            .scrollbar
            .total,
        3
    );
    session.close().unwrap();
    assert_eq!(
        complete(session.wait()).unwrap().lifecycle,
        crate::ghostty_local::LifecycleDelivery::Complete
    );
    consumer.join().unwrap();
}

#[test]
fn native_local_clipboard_reply_uses_own_transport_and_rejects_closed_or_oversize() {
    use oneterm_terminal::ghostty_session::NativeSession;
    let _spawns = crate::spawn_guard::lock_spawns();
    let session = NativeLocalSession::spawn(
        config("[Console]::Write('CLIPBOARD_READY'); $a=[Console]::ReadLine(); exit 0"),
        PtySize::INITIAL,
        TerminalSecurityPolicy::default(),
    )
    .unwrap();
    let events = session.take_events().unwrap();
    let consumer = std::thread::spawn(move || while events.recv_blocking().is_ok() {});
    until(|| text(&session.model()).contains("CLIPBOARD_READY"));
    let native: &dyn NativeSession = &session;
    assert!(native.reply_clipboard(&"x".repeat(256 * 1024 + 1)).is_err());
    native.reply_clipboard("hi").unwrap();
    native.close().unwrap();
    assert_eq!(native.reply_clipboard("late"), Err(TerminalError::Closed));
    complete(session.wait()).unwrap();
    consumer.join().unwrap();
}

#[test]
fn configured_native_logging_captures_and_flushes_on_completion() {
    use oneterm_terminal::ghostty_session::NativeSession;
    let _spawns = crate::spawn_guard::lock_spawns();
    let directory = std::env::temp_dir().join(format!("native-local-log-{}", std::process::id()));
    let logging = oneterm_core::TerminalLogConfig {
        enabled: true,
        directory: directory.clone(),
        ..Default::default()
    };
    let session = NativeLocalSession::spawn_configured(
        config("[Console]::Write(('LOG_'+'VISIBLE')); exit 0"),
        PtySize::INITIAL,
        100,
        TerminalSecurityPolicy::default(),
        logging,
    )
    .unwrap();
    let logging = session.capabilities().logging.unwrap();
    let events = session.take_events().unwrap();
    let consumer = std::thread::spawn(move || while events.recv_blocking().is_ok() {});
    until(|| !session.alive());
    complete(session.wait()).unwrap();
    consumer.join().unwrap();
    assert_eq!(logging.state(), oneterm_terminal::TerminalLogState::Stopped);
    let files: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert!(!files.is_empty());
    let text = files
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect::<String>();
    assert!(text.contains("LOG_VISIBLE"));
    assert!(!text.contains('\x1b'));
    for path in files {
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn erased_native_session_owns_events_and_observes_completion() {
    use oneterm_terminal::{CreatedSession, ghostty_session::NativeSession};
    use std::sync::Arc;
    let _spawns = crate::spawn_guard::lock_spawns();
    let session: Arc<dyn NativeSession> = Arc::new(
        NativeLocalSession::spawn(
            config("[Console]::Write('ERASED_READY'); $a=[Console]::ReadLine(); exit 0"),
            PtySize::INITIAL,
            TerminalSecurityPolicy::default(),
        )
        .unwrap(),
    );
    let CreatedSession::Native(session) = CreatedSession::Native(session) else {
        panic!("native construction must not select the legacy engine");
    };
    let events = session.take_events().unwrap();
    assert!(session.take_events().is_none());
    let consumer = std::thread::spawn(move || while events.recv_blocking().is_ok() {});
    let model = session.render_model();
    until(|| text(&model).contains("ERASED_READY"));
    assert_eq!(session.kind(), SessionKind::Local);
    session.close().unwrap();
    let report = complete(session.complete()).unwrap();
    consumer.join().unwrap();
    assert!(report.lifecycle_delivered);
    assert!(matches!(model.query().state, State::Closed));
}

#[test]
fn erased_native_completion_rejects_retained_controls_and_requests_close() {
    use oneterm_terminal::ghostty_session::NativeSession;
    use std::sync::Arc;
    let _spawns = crate::spawn_guard::lock_spawns();
    let session: Arc<dyn NativeSession> = Arc::new(
        NativeLocalSession::spawn(
            config("[Console]::Write('RETAINED_READY'); $a=[Console]::ReadLine(); exit 0"),
            PtySize::INITIAL,
            TerminalSecurityPolicy::default(),
        )
        .unwrap(),
    );
    let events = session.take_events().unwrap();
    let consumer = std::thread::spawn(move || while events.recv_blocking().is_ok() {});
    let model = session.render_model();
    until(|| text(&model).contains("RETAINED_READY"));
    let retained = session.clone();
    let error = complete(session.complete()).unwrap_err();
    assert!(error.to_string().contains("references remain"));
    assert!(matches!(retained.commit_text("not sent"), Err(_)));
    let report = complete(retained.complete()).unwrap();
    consumer.join().unwrap();
    assert!(report.lifecycle_delivered);
}

#[track_caller]
fn until(mut predicate: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(25);
    while !predicate() {
        assert!(Instant::now() < end, "native session deadline");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn text(model: &SharedModel) -> String {
    model
        .reader()
        .poll()
        .unwrap()
        .frame
        .map(|frame| {
            frame
                .source()
                .rows
                .iter()
                .map(|row| {
                    row.cells
                        .iter()
                        .map(|cell| cell.text.as_str())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn config(script: &str) -> LocalShellConfig {
    LocalShellConfig {
        kind: ShellKind::Custom,
        program: Some(PathBuf::from(format!(
            "{}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
            std::env::var("SystemRoot").unwrap()
        ))),
        args: vec![
            "-NoLogo".into(),
            "-NoProfile".into(),
            "-Command".into(),
            script.into(),
        ],
        env: [("ONETERM_NATIVE_SESSION_TEST".into(), "configured".into())].into(),
        cwd: Some(std::env::current_dir().unwrap()),
        ..Default::default()
    }
}

fn complete<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut result = None;
    until(|| {
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Pending => false,
            Poll::Ready(value) => {
                result = Some(value);
                true
            }
        }
    });
    result.unwrap()
}

#[test]
fn native_local_search_trait_progress_limits_reveal_and_clear() {
    use oneterm_terminal::ghostty::search::{MAX_NEEDLE_BYTES, Options, Status};
    use oneterm_terminal::ghostty_session::SurfaceSession;
    let _spawns = crate::spawn_guard::lock_spawns();
    let session = NativeLocalSession::spawn(
        config("[Console]::Write('SEARCH_READY'); $a=[Console]::ReadLine(); exit 0"),
        PtySize { cols: 80, rows: 6 },
        TerminalSecurityPolicy::default(),
    )
    .unwrap();
    until(|| text(&session.model()).contains("SEARCH_READY"));
    let surface: &dyn SurfaceSession = &session;
    assert!(
        complete(surface.search("x".repeat(MAX_NEEDLE_BYTES + 1), Options::default()))
            .unwrap_err()
            .is_search_rejection()
    );
    let mut found = complete(surface.search("SEARCH_READY".into(), Options::default())).unwrap();
    for _ in 0..64 {
        if found.status == Status::Complete {
            break;
        }
        found = complete(surface.search_step()).unwrap();
    }
    assert_eq!(found.status, Status::Complete);
    assert_eq!(found.matches.len(), 1);
    assert!(surface.query().matches_search(&found));
    let stale = found.navigation(0).unwrap();
    let revealed = complete(surface.reveal_search(found.navigation(0).unwrap())).unwrap();
    assert!(surface.query().matches_search(&revealed));
    assert!(
        complete(surface.reveal_search(stale))
            .unwrap_err()
            .is_search_rejection()
    );
    assert!(
        complete(surface.search(String::new(), Options::default()))
            .unwrap()
            .matches
            .is_empty()
    );
    session.close().unwrap();
    complete(session.wait()).unwrap();
}

#[test]
fn native_local_session_ime_unicode_reaches_read_console_w() {
    use oneterm_terminal::ghostty_session::{InputError as SurfaceError, Size, SurfaceSession};
    let _spawns = crate::spawn_guard::lock_spawns();
    let script = r#"
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class NativeInputProbe {
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern IntPtr GetStdHandle(int id);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern bool ReadConsoleW(IntPtr handle, StringBuilder buffer, uint length, out uint read, IntPtr reserved);
    public static string ReadLine() {
        var buffer = new StringBuilder(256);
        uint read;
        if (!ReadConsoleW(GetStdHandle(-10), buffer, 256, out read, IntPtr.Zero))
            throw new System.ComponentModel.Win32Exception();
        return buffer.ToString().TrimEnd('\r', '\n');
    }
}
'@
[Console]::InputEncoding=[Text.UTF8Encoding]::new($false)
[Console]::Write('UNICODE_READY')
$a=[NativeInputProbe]::ReadLine()
[Console]::Write('NATIVE:' + (($a.ToCharArray() | ForEach-Object { [int]$_ }) -join ','))
$b=[NativeInputProbe]::ReadLine()
[Console]::Write('SECOND:' + (($b.ToCharArray() | ForEach-Object { [int]$_ }) -join ','))
$c=[Console]::ReadLine()
"#;
    let session = NativeLocalSession::spawn(
        config(script),
        PtySize {
            cols: 100,
            rows: 10,
        },
        TerminalSecurityPolicy::default(),
    )
    .unwrap();
    let model = session.model();
    until(|| text(&model).contains("UNICODE_READY"));
    let surface: &dyn SurfaceSession = &session;
    use oneterm_terminal::ghostty::input::{
        Geometry, Modifiers, MouseAction, MouseButton, MouseEvent,
    };
    let mouse = MouseEvent {
        action: MouseAction::Press,
        button: Some(MouseButton::Left),
        modifiers: Modifiers::default(),
        x: 5.0,
        y: 5.0,
        any_button_pressed: true,
        geometry: Geometry {
            width: 900,
            height: 180,
            cell_width: 9,
            cell_height: 18,
            padding_top: 0,
            padding_bottom: 0,
            padding_left: 0,
            padding_right: 0,
        },
    };
    surface.mouse(mouse).unwrap();
    let wheel = oneterm_terminal::ghostty::wheel::WheelEvent {
        steps: -1,
        pointer: mouse,
    };
    surface.wheel(wheel).unwrap();
    assert!(matches!(
        surface.wheel(oneterm_terminal::ghostty::wheel::WheelEvent { steps: 0, ..wheel }),
        Err(SurfaceError::Invalid(_))
    ));
    let mut invalid_mouse = mouse;
    invalid_mouse.x = f32::NAN;
    assert!(matches!(
        surface.mouse(invalid_mouse),
        Err(SurfaceError::Invalid(_))
    ));
    surface
        .resize(Size {
            cols: 100,
            rows: 10,
            cell_width_px: 9,
            cell_height_px: 18,
        })
        .unwrap();
    surface.set_marked_text("pending".into()).unwrap();
    surface.commit_text("e\u{301}日本語").unwrap();
    surface.key(KeyEvent::new(KeyCode::ENTER)).unwrap();
    until(|| text(&model).contains("NATIVE:"));
    let native = text(&model);
    session.set_marked_text("next".into()).unwrap();
    session.commit_text("\u{1f600}é\r").unwrap();
    assert_eq!(session.marked_text(), None);
    until(|| text(&model).contains("SECOND:"));
    let second = text(&model);
    session.close().unwrap();
    assert!(matches!(
        surface.wheel(wheel),
        Err(SurfaceError::Admission(TerminalError::Closed))
    ));
    assert!(matches!(
        surface.mouse(mouse),
        Err(SurfaceError::Admission(TerminalError::Closed))
    ));
    assert!(matches!(
        surface.key(KeyEvent::new(KeyCode::ENTER)),
        Err(SurfaceError::Admission(TerminalError::Closed))
    ));
    assert!(surface.query().viewport.is_some());
    complete(session.wait()).unwrap();
    assert!(
        native.contains("NATIVE:101,769,26085,26412,35486"),
        "{native}"
    );
    assert!(second.contains("SECOND:55357,56832,233"), "{second}");
}

#[test]
fn native_local_session_ime_preedit_commit_and_closed_admission() {
    use oneterm_terminal::ghostty_ime::{Error as ImeError, MAX_TEXT_BYTES};
    let _spawns = crate::spawn_guard::lock_spawns();
    let session = NativeLocalSession::spawn(
        config("[Console]::InputEncoding=[Text.UTF8Encoding]::new($false); [Console]::Write('IME_READY'); $a=[Console]::ReadLine(); [Console]::Write('COMMITTED:' + (($a.ToCharArray() | ForEach-Object { [int]$_ }) -join ',')); $b=[Console]::ReadLine(); exit 0"),
        PtySize { cols: 80, rows: 6 }, TerminalSecurityPolicy::default(),
    ).unwrap();
    let model = session.model();
    until(|| text(&model).contains("IME_READY"));
    complete(session.paste("")).unwrap();
    let before = text(&model);
    session.set_marked_text("compose".into()).unwrap();
    let saved = session.marked_text().unwrap();
    session.clear_marked_text();
    assert_eq!(session.marked_text(), None);
    session.set_marked_text("pending".into()).unwrap();
    assert_eq!(
        session.commit_text(&"a".repeat(MAX_TEXT_BYTES + 1)),
        Err(ImeError::TextLimit)
    );
    assert_eq!(session.marked_text().as_deref(), Some("pending"));
    complete(session.paste("")).unwrap();
    assert_eq!(text(&model), before);
    assert!(!text(&model).contains("COMMITTED:"));
    session.commit_text("ime\r").unwrap();
    assert_eq!(session.marked_text(), None);
    until(|| text(&model).contains("COMMITTED:"));
    assert!(
        text(&model).contains("COMMITTED:105,109,101"),
        "{}",
        text(&model)
    );
    assert_eq!(saved, "compose");
    session.close().unwrap();
    session.set_marked_text("closed".into()).unwrap();
    assert_eq!(
        session.commit_text("not-sent"),
        Err(ImeError::Input(TerminalError::Closed))
    );
    assert_eq!(session.marked_text(), None);
    complete(session.wait()).unwrap();
}

#[test]
fn native_local_session_default_colors_ack_preserves_override_without_more_output() {
    use oneterm_terminal::ghostty::{colors::DefaultColors, render::Rgb};
    use oneterm_terminal::ghostty_content::{Content, RenderSource};
    let _spawns = crate::spawn_guard::lock_spawns();
    let session = NativeLocalSession::spawn(
        config("[Console]::Write([char]27 + ']10;#aabbcc' + [char]7); [Console]::Write('COLORS_READY'); $a=[Console]::ReadLine(); exit 0"),
        PtySize { cols: 80, rows: 6 }, TerminalSecurityPolicy::default()
    ).unwrap();
    let events = session.take_events().unwrap();
    let model = session.model();
    until(|| text(&model).contains("COLORS_READY"));
    complete(session.paste("")).unwrap();
    while events.try_recv().is_ok() {}
    session.release_repaint_hint();
    let colors = DefaultColors {
        foreground: Rgb {
            r: 17,
            g: 34,
            b: 51,
        },
        background: Rgb { r: 4, g: 5, b: 6 },
        cursor: Some(Rgb { r: 7, g: 8, b: 9 }),
        palette: [Rgb {
            r: 10,
            g: 11,
            b: 12,
        }; 256],
    };
    drop(session.set_default_colors(colors.clone()));
    let surface: &dyn oneterm_terminal::ghostty_session::SurfaceSession = &session;
    complete(surface.set_default_colors(colors.clone())).unwrap();
    let source: &dyn RenderSource = &session;
    let mut content = Content::new(source);
    let mut independent = Content::new(source);
    content.poll().unwrap();
    independent.poll().unwrap();
    assert_eq!(
        content.damage(),
        &oneterm_terminal::ghostty::owner::Damage::Full
    );
    assert_eq!(independent.damage(), content.damage());
    let viewport = content.viewport().unwrap().clone();
    assert_eq!((viewport.columns(), viewport.lines()), (80, 6));
    assert_eq!(viewport.rows(0, usize::MAX).len(), 6);
    assert_eq!(viewport.background(), colors.background);
    assert_eq!(viewport.cursor_color(), colors.cursor);
    assert_eq!(viewport.palette(), &colors.palette);
    assert_eq!(
        viewport.foreground(),
        Rgb {
            r: 170,
            g: 187,
            b: 204
        }
    );
    content.poll().unwrap();
    assert_eq!(
        content.damage(),
        &oneterm_terminal::ghostty::owner::Damage::None
    );
    let frame = model.reader().poll().unwrap().frame.unwrap();
    assert_eq!(
        frame.source().foreground,
        Rgb {
            r: 170,
            g: 187,
            b: 204
        }
    );
    assert_eq!(frame.source().background, colors.background);
    assert_eq!(frame.source().cursor_color, colors.cursor);
    assert_eq!(frame.source().palette, colors.palette);
    assert_eq!(events.try_recv().unwrap(), SessionEvent::Output);
    assert_eq!(
        frame.source().foreground,
        Rgb {
            r: 170,
            g: 187,
            b: 204
        }
    );
    assert!(session.alive());
    session.close().unwrap();
    complete(session.wait()).unwrap();
}

#[test]
fn native_local_session_selection_ack_highlight_copy_and_rejection_recovery() {
    use oneterm_terminal::ghostty::anchors::{Coordinate, Point, Space};
    use oneterm_terminal::ghostty::selection::Update;
    let _spawns = crate::spawn_guard::lock_spawns();
    let session = NativeLocalSession::spawn(
        config("[Console]::Write([char]27 + '[2J' + [char]27 + '[Habcdef' + [char]13 + [char]10 + 'ghijkl'); $a=[Console]::ReadLine(); exit 0"),
        PtySize { cols: 80, rows: 6 }, TerminalSecurityPolicy::default()
    ).unwrap();
    let events = session.take_events().unwrap();
    let model = session.model();
    until(|| text(&model).contains("ghijkl"));
    complete(session.paste("")).unwrap();
    while events.try_recv().is_ok() {}
    session.release_repaint_hint();
    let start = complete(model.create_anchor(Point {
        space: Space::Active,
        coordinate: Coordinate { column: 1, row: 0 },
    }))
    .unwrap();
    let end = complete(model.create_anchor(Point {
        space: Space::Active,
        coordinate: Coordinate { column: 3, row: 1 },
    }))
    .unwrap();
    let update = Update::Set {
        start: start.clone(),
        end: end.clone(),
        rectangle: true,
    };
    drop(session.update_selection(update.clone()));
    complete(session.update_selection(update.clone())).unwrap();
    let mut content = oneterm_terminal::ghostty_content::Content::new(&session);
    content.poll().unwrap();
    let selected = content.viewport().unwrap().clone();
    assert_eq!(selected.selection_spans().len(), 2);
    assert_eq!(selected.selection_spans()[0].columns, 1..4);
    let good = model.reader().poll().unwrap().frame.unwrap();
    assert_eq!(
        good.source()
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .filter(|cell| cell.selected)
            .count(),
        6
    );
    assert_eq!(
        complete(model.selection_text()).unwrap().as_deref(),
        Some("bcd\nhij")
    );
    assert_eq!(events.try_recv().unwrap(), SessionEvent::Output);
    complete(model.release_anchor(&end)).unwrap();
    assert!(
        complete(session.update_selection(update))
            .unwrap_err()
            .is_selection_rejection()
    );
    assert!(session.alive());
    assert!(std::sync::Arc::ptr_eq(
        &good,
        model.reader().poll().unwrap().frame.as_ref().unwrap()
    ));
    assert_eq!(
        complete(model.selection_text()).unwrap().as_deref(),
        Some("bcd\nhij")
    );
    session.release_repaint_hint();
    let native_surface: &dyn oneterm_terminal::ghostty_session::SurfaceSession = &session;
    complete(native_surface.selection(Update::All)).unwrap();
    let all = complete(model.selection_text()).unwrap().unwrap();
    assert!(all.contains("abcdef") && all.contains("ghijkl"), "{all:?}");
    complete(session.update_selection(Update::Clear)).unwrap();
    content.poll().unwrap();
    assert!(content.viewport().unwrap().selection_spans().is_empty());
    assert_eq!(selected.selection_spans().len(), 2);
    assert!(
        model
            .reader()
            .poll()
            .unwrap()
            .frame
            .unwrap()
            .source()
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .all(|cell| !cell.selected)
    );
    assert_eq!(complete(model.selection_text()).unwrap(), None);
    assert_eq!(events.try_recv().unwrap(), SessionEvent::Output);
    complete(model.release_anchor(&start)).unwrap();
    use oneterm_terminal::ghostty::selection::{Gesture, GestureId};
    let surface: &dyn oneterm_terminal::ghostty_session::SurfaceSession = &session;
    let id = GestureId::default();
    let viewport = model.query().viewport.unwrap();
    complete(surface.selection(Update::Gesture(Gesture::Begin {
        id: id.clone(),
        viewport: viewport.clone(),
        coordinate: Coordinate { column: 1, row: 0 },
    })))
    .unwrap();
    complete(surface.selection(Update::Gesture(Gesture::Move {
        id: id.clone(),
        viewport,
        coordinate: Coordinate { column: 3, row: 0 },
    })))
    .unwrap();
    complete(surface.selection(Update::Gesture(Gesture::Finish(id)))).unwrap();
    assert_eq!(
        complete(model.selection_text()).unwrap().as_deref(),
        Some("bcd")
    );
    session.close().unwrap();
    complete(session.wait()).unwrap();
}

#[test]
fn native_local_session_configuration_events_controls_and_lifecycle() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<NativeLocalSession>();
    let _spawns = crate::spawn_guard::lock_spawns();
    // Custom shells bypass powershell_init; preserve the Unicode title on any
    // runner code page before checking the shared bidi sanitization policy.
    let script = "[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); [Console]::WriteLine('CFG:'+ $env:ONETERM_NATIVE_SESSION_TEST); [Console]::WriteLine('CWD:'+ (Get-Location).Path); [Console]::Write([char]27 + ']2;session' + [char]0x202e + 'safe' + [char]7); [Console]::Write([char]27 + ']7;file:///C:/native%20session' + [char]7); $a=[Console]::ReadLine(); [Console]::WriteLine('SIZE:'+ [Console]::WindowWidth + ':' + [Console]::WindowHeight + ':ACK:' + $a); $b=[Console]::ReadLine(); exit 7";
    let session = NativeLocalSession::spawn(
        config(script),
        PtySize { cols: 80, rows: 24 },
        TerminalSecurityPolicy {
            allow_remote_clipboard_write: true,
            allow_remote_clipboard_read: true,
        },
    )
    .unwrap();
    let lifecycle: &dyn TerminalLifecycle = &session;
    let events = lifecycle.take_events().unwrap();
    assert!(lifecycle.take_events().is_none());
    assert_eq!(lifecycle.kind(), SessionKind::Local);
    assert!(lifecycle.alive());
    let model = session.model();
    let wait_for = |phase: &str, ready: &mut dyn FnMut() -> bool| {
        let deadline = Instant::now() + Duration::from_secs(25);
        while !ready() {
            assert!(
                Instant::now() < deadline,
                "native session deadline ({phase}): title={:?}, cwd={:?}, alive={}, state={:?}, output={:?}",
                session.title(),
                session.cwd(),
                session.alive(),
                model.query().state,
                text(&model),
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    wait_for("sanitized title and cwd", &mut || {
        session.title().as_deref() == Some("sessionsafe") && session.cwd().is_some()
    });
    assert_eq!(session.cwd().unwrap(), PathBuf::from("C:/native session"));
    let expected_cwd = format!("CWD:{}", std::env::current_dir().unwrap().display());
    wait_for("configured environment and startup directory", &mut || {
        text(&model).contains("CFG:configured") && text(&model).contains(&expected_cwd)
    });
    session.release_repaint_hint();
    session
        .resize(WindowSize {
            cols: 100,
            rows: 30,
            cell_width: 10,
            cell_height: 20,
        })
        .unwrap();
    let mut key = KeyEvent::new(KeyCode::UNIDENTIFIED);
    key.text = "ping".into();
    session.key(key).unwrap();
    session.key(KeyEvent::new(KeyCode::ENTER)).unwrap();
    wait_for("resize and input acknowledgement", &mut || {
        text(&model).contains("SIZE:100:30:ACK:ping")
    });
    session.write(b"exit\r").unwrap();
    let mut wait = Box::pin(session.wait());
    let mut result = None;
    until(
        || match wait.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Pending => false,
            Poll::Ready(value) => {
                result = Some(value);
                true
            }
        },
    );
    assert_eq!(result.unwrap().unwrap().exit_code, Some(7));
    let mut delivered = Vec::new();
    while let Ok(event) = events.try_recv() {
        if matches!(event, SessionEvent::Exited(_) | SessionEvent::Closed) {
            delivered.push(event);
        }
    }
    assert_eq!(
        delivered,
        vec![SessionEvent::Exited(Some(7)), SessionEvent::Closed]
    );
    assert!(matches!(
        model.reader().poll().unwrap().state,
        State::Closed
    ));
}

#[test]
fn native_local_session_configuration_rejection_is_synchronous() {
    let cfg = LocalShellConfig {
        kind: ShellKind::Custom,
        program: None,
        ..Default::default()
    };
    assert!(
        NativeLocalSession::spawn(
            cfg,
            PtySize { rows: 24, cols: 80 },
            TerminalSecurityPolicy::default()
        )
        .is_err()
    );
}

#[test]
fn native_local_session_close_then_drop_is_nonjoining() {
    let _spawns = crate::spawn_guard::lock_spawns();
    let session = NativeLocalSession::spawn(
        config("[Console]::WriteLine('SESSION_READY'); $a=[Console]::ReadLine(); exit 0"),
        PtySize { cols: 80, rows: 24 },
        TerminalSecurityPolicy::default(),
    )
    .unwrap();
    let model = session.model();
    let events = session.take_events().unwrap();
    until(|| text(&model).contains("SESSION_READY"));
    session.close().unwrap();
    assert_eq!(session.write(b"never"), Err(TerminalError::Closed));
    drop(session);
    until(|| {
        matches!(
            model.reader().poll().unwrap().state,
            State::Closed | State::Failed(_)
        )
    });
    until(|| matches!(events.try_recv(), Ok(SessionEvent::Closed)));
}

#[test]
fn native_local_session_scroll_publishes_without_more_child_output() {
    use oneterm_terminal::ghostty::state::Scroll;
    let _spawns = crate::spawn_guard::lock_spawns();
    let session = NativeLocalSession::spawn(config("[Console]::Write([char]27 + '[2J' + [char]27 + '[H'); 0..59 | ForEach-Object { [Console]::WriteLine('HISTORY_' + $_) }; [Console]::Write('SCROLL_READY'); $a=[Console]::ReadLine(); exit 0"), PtySize { cols: 80, rows: 6 }, TerminalSecurityPolicy::default()).unwrap();
    let events = session.take_events().unwrap();
    let model = session.model();
    until(|| text(&model).contains("SCROLL_READY"));
    let mut barrier = Box::pin(session.paste(""));
    until(|| {
        match barrier
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Pending => false,
            Poll::Ready(result) => {
                result.unwrap();
                true
            }
        }
    });
    drop(barrier);
    let bottom = model.reader().poll().unwrap().frame.unwrap();
    assert!(bottom.source().metadata.scrollbar.total > 6);
    while events.try_recv().is_ok() {}
    session.release_repaint_hint();
    session.scroll(Scroll::Top).unwrap();
    until(|| {
        let frame = model.reader().poll().unwrap().frame.unwrap();
        !std::sync::Arc::ptr_eq(&frame, &bottom) && frame.source().metadata.scrollbar.offset == 0
    });
    assert!(text(&model).contains("HISTORY_0"));
    until(|| matches!(events.try_recv(), Ok(SessionEvent::Output)));
    session.scroll(Scroll::Delta(1)).unwrap();
    until(|| {
        model
            .reader()
            .poll()
            .unwrap()
            .frame
            .unwrap()
            .source()
            .metadata
            .scrollbar
            .offset
            == 1
    });
    session.scroll(Scroll::Row(2)).unwrap();
    until(|| {
        model
            .reader()
            .poll()
            .unwrap()
            .frame
            .unwrap()
            .source()
            .metadata
            .scrollbar
            .offset
            == 2
    });
    session.scroll(Scroll::Delta(-1)).unwrap();
    until(|| {
        model
            .reader()
            .poll()
            .unwrap()
            .frame
            .unwrap()
            .source()
            .metadata
            .scrollbar
            .offset
            == 1
    });
    session.release_repaint_hint();
    session.scroll(Scroll::Bottom).unwrap();
    until(|| model.reader().poll().unwrap().frame.unwrap().source() == bottom.source());
    until(|| matches!(events.try_recv(), Ok(SessionEvent::Output)));
    use oneterm_terminal::ghostty::input::{Geometry, Modifiers, MouseAction, MouseEvent};
    let surface: &dyn oneterm_terminal::ghostty_session::SurfaceSession = &session;
    let mut wheel = oneterm_terminal::ghostty::wheel::WheelEvent {
        steps: 2,
        pointer: MouseEvent {
            action: MouseAction::Motion,
            button: None,
            modifiers: Modifiers::default(),
            x: 5.0,
            y: 5.0,
            any_button_pressed: false,
            geometry: Geometry {
                width: 720,
                height: 108,
                cell_width: 9,
                cell_height: 18,
                padding_top: 0,
                padding_bottom: 0,
                padding_left: 0,
                padding_right: 0,
            },
        },
    };
    session.release_repaint_hint();
    surface.wheel(wheel).unwrap();
    until(|| {
        model
            .reader()
            .poll()
            .unwrap()
            .frame
            .unwrap()
            .source()
            .metadata
            .scrollbar
            .offset
            == bottom.source().metadata.scrollbar.offset - 2
    });
    until(|| matches!(events.try_recv(), Ok(SessionEvent::Output)));
    wheel.steps = -2;
    surface.wheel(wheel).unwrap();
    until(|| model.reader().poll().unwrap().frame.unwrap().source() == bottom.source());
    session.close().unwrap();
    let mut wait = Box::pin(session.wait());
    until(
        || match wait.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Pending => false,
            Poll::Ready(result) => {
                result.unwrap();
                true
            }
        },
    );
}

#[test]
fn native_local_session_paste_ack_and_recoverable_policy_rejection() {
    local_session_paste_ack(false);
}

#[test]
fn native_local_session_visible_paste_trait_ack_and_recoverable_policy_rejection() {
    local_session_paste_ack(true);
}

fn local_session_paste_ack(visible: bool) {
    use oneterm_terminal::ghostty_session::{PasteFuture, SurfaceSession};
    let _spawns = crate::spawn_guard::lock_spawns();
    let session = NativeLocalSession::spawn(config("[Console]::WriteLine('SESSION_READY'); $a=[Console]::ReadLine(); [Console]::WriteLine('PASTE_ACK:'+ $a); $b=[Console]::ReadLine(); exit 0"), PtySize { cols: 80, rows: 24 }, TerminalSecurityPolicy::default()).unwrap();
    let _events = session.take_events().unwrap();
    let model = session.model();
    until(|| text(&model).contains("SESSION_READY"));
    let oversized = "x".repeat(1024 * 1024 + 1);
    let surface: &dyn SurfaceSession = &session;
    let mut rejected: PasteFuture<'_> = if visible {
        surface.paste_visible(&oversized)
    } else {
        Box::pin(session.paste(&oversized))
    };
    assert!(matches!(
        rejected
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(
            oneterm_terminal::backend::AsyncGhosttyPumpError::Paste(_)
        ))
    ));
    drop(rejected);
    let mut paste: PasteFuture<'_> = if visible {
        surface.paste_visible("ping\n")
    } else {
        Box::pin(session.paste("ping\n"))
    };
    until(
        || match paste.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Pending => false,
            Poll::Ready(result) => {
                result.unwrap();
                true
            }
        },
    );
    drop(paste);
    until(|| text(&model).contains("PASTE_ACK:ping"));
    session.close().unwrap();
    let mut wait = Box::pin(session.wait());
    until(
        || match wait.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Pending => false,
            Poll::Ready(result) => {
                result.unwrap();
                true
            }
        },
    );
}
