//! Bare real-platform close diagnostics; no OneTerm services are initialized.

use std::io::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use gpui::{Context, Window, div, prelude::*};

const CHILD_MODE: &str = "ONETERM_BARE_GPUI_CLOSE_DIAGNOSTIC";
const TEST: &str = "gpui_close_tests::bare_gpui_owned_window_close_diagnostic";

struct BareWindow {
    close_after_frame: Option<String>,
}

impl Render for BareWindow {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        if let Some(title) = self.close_after_frame.take() {
            window.on_next_frame(move |_, _| post_owned_close(&title));
        }
        div().size_32()
    }
}

fn post_owned_close(title: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    // The unique title locates only this child, and PID ownership is checked
    // immediately before posting. Never broadcast or close a user's window.
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        assert!(!hwnd.is_null(), "bare GPUI HWND must exist");
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        assert_eq!(pid, std::process::id(), "bare HWND owner");
        eprintln!("BARE_GPUI_CLOSE pid={pid} hwnd={hwnd:p}");
        assert_ne!(PostMessageW(hwnd, WM_CLOSE, 0, 0), 0);
    }
}

#[test]
fn bare_gpui_owned_window_close_diagnostic() {
    let Ok(mode) = std::env::var(CHILD_MODE) else {
        run_children();
        return;
    };
    assert!(matches!(
        mode.as_str(),
        "immediate" | "framed" | "logical-immediate" | "logical-framed"
    ));
    let errors = Arc::new(AtomicUsize::new(0));
    let stale = Arc::new(AtomicUsize::new(0));
    let invalid = Arc::new(AtomicUsize::new(0));
    let (logged_errors, logged_stale, logged_invalid) =
        (errors.clone(), stale.clone(), invalid.clone());
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .format(move |buffer, record| {
            let message = record.args().to_string();
            writeln!(
                buffer,
                "[{} {} {}:{}] {message}",
                record.level(),
                record.target(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
            )?;
            if record.level() == log::Level::Error {
                logged_errors.fetch_add(1, Ordering::SeqCst);
                let is_stale = message == "window not found";
                let is_invalid = message.contains("0x80070578");
                if is_stale {
                    logged_stale.fetch_add(1, Ordering::SeqCst);
                }
                if is_invalid {
                    logged_invalid.fetch_add(1, Ordering::SeqCst);
                }
                if is_stale || is_invalid {
                    writeln!(
                        buffer,
                        "BARE_GPUI_ERROR_STACK {}",
                        std::backtrace::Backtrace::force_capture()
                    )?;
                }
            }
            Ok(())
        })
        .try_init()
        .unwrap();
    let closed = Arc::new(AtomicUsize::new(0));
    let observed = closed.clone();
    let title = format!("Bare GPUI close {mode} {}", std::process::id());
    let framed = mode.ends_with("framed");
    let logical = mode.starts_with("logical-");
    // No component init, app init, assets, terminal, workspace or persistence.
    gpui_platform::application()
        .with_quit_mode(gpui::QuitMode::LastWindowClosed)
        .run(move |cx| {
            cx.on_window_closed(move |_, _| {
                observed.fetch_add(1, Ordering::SeqCst);
            })
            .detach();
            cx.open_window(gpui::WindowOptions::default(), |window, cx| {
                window.set_window_title(&title);
                if logical {
                    // Match the application's admitted logical-close seam,
                    // without any terminal cleanup or artificial delay.
                    window.on_window_should_close(cx, |window, _| {
                        eprintln!("BARE_GPUI_LOGICAL_REMOVE");
                        window.remove_window();
                        false
                    });
                }
                cx.new(|_| BareWindow {
                    close_after_frame: framed.then(|| title.clone()),
                })
            })
            .unwrap();
            if !framed {
                post_owned_close(&title);
            }
        });
    assert_eq!(closed.load(Ordering::SeqCst), 1, "close observer");
    eprintln!(
        "BARE_GPUI_RESULT mode={mode} closed=1 errors={} stale_window={} invalid_hwnd={}",
        errors.load(Ordering::SeqCst),
        stale.load(Ordering::SeqCst),
        invalid.load(Ordering::SeqCst),
    );
    // Errors are diagnostic evidence, not a clean-log acceptance assertion.
}

fn run_children() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.tmp")
        .join(format!(
            "us0249-bare-gpui-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&directory).unwrap();
    for round in 0..3 {
        for mode in ["immediate", "framed", "logical-immediate", "logical-framed"] {
            let stem = format!("{round}-{mode}");
            let stderr = directory.join(format!("{stem}.stderr.log"));
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD_MODE, mode)
                .stdout(
                    std::fs::File::create(directory.join(format!("{stem}.stdout.log"))).unwrap(),
                )
                .stderr(std::fs::File::create(&stderr).unwrap())
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(
                        status.success(),
                        "bare GPUI child {stem}: {status}; {stderr:?}"
                    );
                    break;
                }
                if Instant::now() >= deadline {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("bare GPUI child {stem} exceeded watchdog; {stderr:?}");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let log = std::fs::read_to_string(&stderr).unwrap();
            let result = log
                .lines()
                .find(|line| line.starts_with("BARE_GPUI_RESULT"));
            assert!(result.is_some(), "missing close observation: {stderr:?}");
            eprintln!("{stem}: {}; evidence={stderr:?}", result.unwrap());
        }
    }
}
