use std::time::{Duration, Instant};

use oneterm_terminal::ghostty_bridge::State;
use oneterm_vt::pty::Shell;

use super::*;

fn options(script: &str) -> Options {
    Options {
        shell: Some(Shell::new(
            format!(
                "{}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
                std::env::var("SystemRoot").unwrap()
            ),
            vec![
                "-NoLogo".into(),
                "-NoProfile".into(),
                "-Command".into(),
                script.into(),
            ],
        )),
        escape_args: true,
        ..Default::default()
    }
}

fn start(
    script: &str,
    capacity: usize,
) -> (
    NativeLocal,
    async_channel::Sender<SessionEvent>,
    async_channel::Receiver<SessionEvent>,
) {
    let (tx, rx) = async_channel::bounded(capacity);
    let endpoint = NativeLocal::spawn(
        options(script),
        WindowSize {
            cols: 80,
            rows: 24,
            ..Default::default()
        },
        tx.clone(),
    )
    .unwrap();
    (endpoint, tx, rx)
}

#[track_caller]
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(25);
    while !condition() {
        assert!(Instant::now() < deadline, "native local owner deadline");
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

fn complete<F: std::future::Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
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

fn wait(endpoint: NativeLocal) -> Result<Report, Error> {
    complete(endpoint.wait())
}

#[test]
fn native_local_owner_autonomous_ack_and_normal_exit() {
    let _spawns = crate::spawn_guard::lock_spawns();
    let (endpoint, _, events) = start(
        "[Console]::WriteLine('OWNER_READY'); $a=[Console]::ReadLine(); [Console]::WriteLine('ACK:'+ $a); $b=[Console]::ReadLine(); exit 7",
        4096,
    );
    let model = endpoint.model();
    until(|| {
        if let Ok(result) = endpoint.result.try_recv() {
            panic!("early owner result: {result:?}");
        }
        text(&model).contains("OWNER_READY")
    });
    endpoint.write(b"pi").unwrap();
    endpoint.write(b"ng\r").unwrap();
    until(|| text(&model).contains("ACK:ping"));
    assert_eq!(
        endpoint.control.queued_input_bytes.load(Ordering::Relaxed),
        0
    );
    endpoint.write(b"exit\r").unwrap();
    let report = wait(endpoint).unwrap();
    assert_eq!(report.stop, Stop::ChildExit);
    assert_eq!(report.exit_code, Some(7));
    assert_eq!(report.lifecycle, LifecycleDelivery::Complete);
    let mut lifecycle = Vec::new();
    while let Ok(event) = events.try_recv() {
        if !matches!(event, SessionEvent::Output | SessionEvent::Title(_)) {
            lifecycle.push(event);
        }
    }
    assert_eq!(
        lifecycle,
        vec![SessionEvent::Exited(Some(7)), SessionEvent::Closed]
    );
    assert!(matches!(
        model.reader().poll().unwrap().state,
        State::Closed
    ));
}

#[test]
fn native_local_orderly_shutdown_finishes_blocked_publication_and_fifo() {
    let _spawns = crate::spawn_guard::lock_spawns();
    let (mut endpoint, tx, events) = start(
        "[Console]::WriteLine('OWNER_READY'); $a=[Console]::ReadLine(); [Console]::Write([char]27 + ']2;sealed-publication' + [char]7); $b=[Console]::ReadLine()",
        1,
    );
    let model = endpoint.model();
    until(|| {
        while events.try_recv().is_ok() {}
        text(&model).contains("OWNER_READY")
    });
    while events.try_recv().is_ok() {}
    tx.try_send(SessionEvent::Bell).unwrap();
    endpoint.write(b"publish\r").unwrap();
    until(|| endpoint.state().title().as_deref() == Some("sealed-publication"));
    let mut acknowledgments = Vec::new();
    for cols in [81, 92, 103] {
        endpoint
            .resize(WindowSize {
                cols,
                rows: 24,
                ..Default::default()
            })
            .unwrap();
        let (reply, ack) = async_channel::bounded(1);
        endpoint
            .enqueue(Input::Selection(SelectionUpdate::Clear, reply))
            .unwrap();
        acknowledgments.push(ack);
    }
    endpoint.request_shutdown().unwrap();
    endpoint.request_shutdown().unwrap();
    assert!(!endpoint.control.shutdown.load(Ordering::Acquire));
    assert!(matches!(
        endpoint.write(b"rejected"),
        Err(TerminalError::Closed)
    ));
    assert!(matches!(
        endpoint.resize(WindowSize {
            cols: 80,
            rows: 24,
            ..Default::default()
        }),
        Err(InputError::Admission(TerminalError::Closed))
    ));
    assert!(
        endpoint.result.try_recv().is_err(),
        "full reliable queue must keep completion pending"
    );
    for ack in &acknowledgments {
        assert!(
            ack.try_recv().is_err(),
            "queued control cannot overtake blocked publication"
        );
    }
    let mut closed = false;
    let report = {
        let mut result = None;
        until(|| {
            while let Ok(event) = events.try_recv() {
                closed |= matches!(event, SessionEvent::Closed);
            }
            if let Ok(value) = endpoint.result.try_recv() {
                result = Some(value);
                true
            } else {
                false
            }
        });
        result.unwrap().unwrap()
    };
    while let Ok(event) = events.try_recv() {
        closed |= matches!(event, SessionEvent::Closed);
    }
    assert!(closed);
    for ack in acknowledgments {
        ack.try_recv().unwrap().unwrap();
    }
    assert_eq!(
        model.reader().poll().unwrap().frame.unwrap().source().cols,
        103
    );
    assert_eq!(report.stop, Stop::Requested);
    assert_eq!(report.lifecycle, LifecycleDelivery::Complete);
    // The private result was consumed while independently draining events.
    endpoint.completed = true;
}

#[test]
fn native_local_owner_ui_pressure_services_fifo_and_close() {
    let _spawns = crate::spawn_guard::lock_spawns();
    let script = "[Console]::WriteLine('OWNER_READY'); $a=[Console]::ReadLine(); [Console]::Write([char]27 + ']2;blocked' + [char]7); $b=[Console]::ReadLine(); exit 0";
    let (endpoint, tx, events) = start(script, 1);
    let model = endpoint.model();
    until(|| {
        while events.try_recv().is_ok() {}
        text(&model).contains("OWNER_READY")
    });
    while events.try_recv().is_ok() {}
    tx.try_send(SessionEvent::Bell).unwrap();
    endpoint.write(b"title\r").unwrap();
    // Native title processing proves the next batch is waiting on the full UI.
    until(|| endpoint.state().title().as_deref() == Some("blocked"));
    let good = model.reader().poll().unwrap().frame.unwrap();
    for _ in 0..CONTROL_QUEUE_CAPACITY {
        endpoint
            .resize(WindowSize {
                cols: 90,
                rows: 26,
                ..Default::default()
            })
            .unwrap();
    }
    assert!(matches!(
        endpoint.key(KeyEvent::new(
            oneterm_terminal::ghostty::input::KeyCode::ENTER
        )),
        Err(InputError::Admission(TerminalError::QueueFull))
    ));
    endpoint.write(b"exit\r").unwrap();
    until(|| endpoint.control.queued_input_bytes.load(Ordering::Relaxed) == 0);
    let mut paste = Box::pin(endpoint.paste("queue full"));
    assert!(matches!(
        endpoint.scroll(Scroll::Top),
        Err(InputError::Admission(TerminalError::QueueFull))
    ));
    assert!(matches!(
        paste.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(AsyncGhosttyPumpError::Transport(
            TerminalError::QueueFull
        )))
    ));
    drop(paste);
    assert_eq!(endpoint.paste_budget.load(Ordering::Acquire), 0);
    assert!(!endpoint.control.shutdown.load(Ordering::Acquire));
    endpoint.close().unwrap();
    let mut closed = Box::pin(endpoint.paste("closed"));
    assert!(matches!(
        endpoint.scroll(Scroll::Top),
        Err(InputError::Admission(TerminalError::Closed))
    ));
    assert!(matches!(
        closed
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(AsyncGhosttyPumpError::Transport(TerminalError::Closed)))
    ));
    drop(closed);
    assert_eq!(endpoint.paste_budget.load(Ordering::Acquire), 0);
    assert_eq!(endpoint.write(b"never"), Err(TerminalError::Closed));
    assert!(matches!(
        endpoint.resize(WindowSize {
            cols: 80,
            rows: 24,
            ..Default::default()
        }),
        Err(InputError::Admission(TerminalError::Closed))
    ));
    assert!(matches!(
        endpoint.mouse(mouse_press()),
        Err(InputError::Admission(TerminalError::Closed))
    ));
    let control = endpoint.control.clone();
    let report = wait(endpoint).unwrap();
    assert_eq!(report.stop, Stop::Requested);
    assert_eq!(report.lifecycle, LifecycleDelivery::Cancelled);
    assert_eq!(control.queued_input_bytes.load(Ordering::Relaxed), 0);
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));
    assert_eq!(events.try_recv().unwrap(), SessionEvent::Bell);
    assert!(events.is_empty());
}

#[test]
fn native_local_owner_drop_and_cancel_wait_request_teardown() {
    let _spawns = crate::spawn_guard::lock_spawns();
    for cancel_wait in [false, true] {
        let (endpoint, _, events) = start(
            "[Console]::WriteLine('OWNER_READY'); $a=[Console]::ReadLine(); exit 0",
            1,
        );
        let model = endpoint.model();
        until(|| {
            while events.try_recv().is_ok() {}
            text(&model).contains("OWNER_READY")
        });
        let completion = endpoint.result.clone();
        if cancel_wait {
            let mut future = Box::pin(endpoint.wait());
            assert!(
                future
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
            drop(future);
        } else {
            drop(endpoint);
        }
        let mut result = None;
        until(|| {
            result = completion.try_recv().ok();
            result.is_some()
        });
        assert_eq!(result.unwrap().unwrap().stop, Stop::Requested);
    }
}

struct PartialWriter {
    bytes: Vec<u8>,
    allowance: usize,
    fail: bool,
}
impl Write for PartialWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "fixture writer"));
        }
        if self.allowance == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let n = bytes.len().min(self.allowance).min(2);
        self.bytes.extend_from_slice(&bytes[..n]);
        self.allowance -= n;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
struct MockPty {
    reader: io::Cursor<Vec<u8>>,
    writer: PartialWriter,
    sizes: Vec<WindowSize>,
    fail_resize: bool,
}
impl EventedReadWrite for MockPty {
    type Reader = io::Cursor<Vec<u8>>;
    type Writer = PartialWriter;
    unsafe fn register(&mut self, _: &Arc<Poller>, _: PollEvent, _: PollMode) -> io::Result<()> {
        Ok(())
    }
    fn reregister(&mut self, _: &Arc<Poller>, _: PollEvent, _: PollMode) -> io::Result<()> {
        Ok(())
    }
    fn deregister(&mut self, _: &Arc<Poller>) -> io::Result<()> {
        Ok(())
    }
    fn reader(&mut self) -> &mut Self::Reader {
        &mut self.reader
    }
    fn writer(&mut self) -> &mut Self::Writer {
        &mut self.writer
    }
}
impl EventedPty for MockPty {
    fn next_child_event(&mut self) -> Option<ChildEvent> {
        None
    }
}
impl OnResize for MockPty {
    fn on_resize(&mut self, size: WindowSize) -> io::Result<()> {
        if self.fail_resize {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "fixture resize"));
        }
        self.sizes.push(size);
        Ok(())
    }
}

#[test]
fn native_local_owner_partial_write_error_reclaims_exact_budget() {
    let poll = Arc::new(Poller::new().unwrap());
    let control = Arc::new(ShellControl::default());
    let (sender, input) = mpsc::sync_channel(LOCAL_COMMAND_QUEUE_CAPACITY);
    let notifier = ShellNotifier {
        sender,
        poller: poll.clone(),
        control: control.clone(),
    };
    let mut io = OwnerIo {
        pty: Some(MockPty {
            sizes: Vec::new(),
            fail_resize: false,
            reader: io::Cursor::new(Vec::new()),
            writer: PartialWriter {
                bytes: Vec::new(),
                allowance: 3,
                fail: false,
            },
        }),
        registered: false,
        input,
        pending: None,
        offset: 0,
        poll,
        control: control.clone(),
        admission: Arc::new(Admission {
            gate: Mutex::new(()),
            requested: AtomicBool::new(false),
            sealed: AtomicBool::new(false),
        }),
        exit: None,
        cancelled: false,
        controls: async_channel::bounded(1).1,
    };
    notifier
        .send(ShellMsg::Input(Cow::Borrowed(b"first")))
        .unwrap();
    notifier
        .send(ShellMsg::Input(Cow::Borrowed(b"second")))
        .unwrap();
    io.writes().unwrap();
    assert_eq!(io.pty.as_ref().unwrap().writer.bytes, b"fir");
    assert_eq!(control.queued_input_bytes.load(Ordering::Relaxed), 8);
    io.pty.as_mut().unwrap().writer.allowance = 4;
    io.writes().unwrap();
    assert_eq!(io.pty.as_ref().unwrap().writer.bytes, b"firstse");
    assert_eq!(control.queued_input_bytes.load(Ordering::Relaxed), 4);
    io.pty.as_mut().unwrap().writer.fail = true;
    assert_eq!(io.writes().unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(control.queued_input_bytes.load(Ordering::Relaxed), 4);
    notifier
        .send(ShellMsg::Input(Cow::Borrowed(b"queued")))
        .unwrap();
    io.dispose().unwrap();
    assert_eq!(control.queued_input_bytes.load(Ordering::Relaxed), 0);
    assert_eq!(
        notifier
            .send(ShellMsg::Input(Cow::Borrowed(b"never")))
            .unwrap_err()
            .kind(),
        io::ErrorKind::BrokenPipe
    );
    io.dispose().unwrap();
    assert_eq!(control.queued_input_bytes.load(Ordering::Relaxed), 0);
}

#[test]
fn native_local_owner_spawn_failure_reports_after_native_teardown() {
    let _spawns = crate::spawn_guard::lock_spawns();
    let (events, rx) = async_channel::bounded(1);
    let endpoint = NativeLocal::spawn(
        Options {
            shell: Some(Shell::new(
                "Z:\\not-a-real-oneterm-shell.exe".into(),
                Vec::new(),
            )),
            ..Default::default()
        },
        WindowSize {
            cols: 80,
            rows: 24,
            ..Default::default()
        },
        events,
    )
    .unwrap();
    let model = endpoint.model();
    let state = endpoint.state();
    endpoint.write(b"queued before startup result").unwrap();
    let control = endpoint.control.clone();
    assert!(matches!(wait(endpoint), Err(Error::Io(_))));
    assert!(!state.alive());
    assert_eq!(control.queued_input_bytes.load(Ordering::Relaxed), 0);
    assert!(matches!(
        model.reader().poll().unwrap().state,
        State::Closed
    ));
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Closed);
}

#[test]
fn native_local_owner_final_closed_waits_after_teardown_or_cancels() {
    let _spawns = crate::spawn_guard::lock_spawns();
    for cancel in [false, true] {
        let (endpoint, _, events) = start(
            "[Console]::WriteLine('OWNER_READY'); $a=[Console]::ReadLine(); exit 7",
            1,
        );
        let model = endpoint.model();
        until(|| {
            while events.try_recv().is_ok() {}
            text(&model).contains("OWNER_READY")
        });
        while events.try_recv().is_ok() {}
        endpoint.write(b"exit\r").unwrap();
        until(|| matches!(model.reader().poll().unwrap().state, State::Closed));
        assert!(!endpoint.state().alive());
        assert_eq!(
            endpoint.control.queued_input_bytes.load(Ordering::Relaxed),
            0
        );
        assert_eq!(endpoint.write(b"never"), Err(TerminalError::Closed));
        assert!(endpoint.result.is_empty());
        if cancel {
            endpoint.close().unwrap();
        } else {
            assert_eq!(events.try_recv().unwrap(), SessionEvent::Exited(Some(7)));
        }
        let report = wait(endpoint).unwrap();
        assert_eq!(report.stop, Stop::ChildExit);
        assert_eq!(
            report.lifecycle,
            if cancel {
                LifecycleDelivery::Cancelled
            } else {
                LifecycleDelivery::Complete
            }
        );
        assert_eq!(
            events.try_recv().unwrap(),
            if cancel {
                SessionEvent::Exited(Some(7))
            } else {
                SessionEvent::Closed
            }
        );
        assert!(events.is_empty());
    }
}

#[test]
fn native_local_raw_visible_exact_input_without_bracketed_paste() {
    let _spawns = crate::spawn_guard::lock_spawns();
    let (endpoint, _, _events) = start(
        "[Console]::Write([char]27 + '[?2004h'); [Console]::WriteLine('RAW_READY'); $a=[Console]::ReadLine(); [Console]::WriteLine('ACK:'+$a); $b=[Console]::ReadLine(); exit 0",
        4096,
    );
    let model = endpoint.model();
    until(|| text(&model).contains("RAW_READY"));
    complete(endpoint.write_visible(b"raw-ping\r")).unwrap();
    until(|| text(&model).contains("ACK:raw-ping"));
    endpoint.close().unwrap();
    assert!(matches!(
        complete(endpoint.write_visible(b"late")),
        Err(AsyncGhosttyPumpError::Transport(TerminalError::Closed))
    ));
    wait(endpoint).unwrap();
}

#[test]
fn native_local_key_receipt_writes_once_and_acknowledges_original_fifo() {
    use oneterm_terminal::ghostty::input::{KeyCode, MAX_KEY_TEXT_BYTES};
    let _spawns = crate::spawn_guard::lock_spawns();
    let (endpoint, _, _events) = start(
        "[Console]::WriteLine('OWNER_READY'); $a=[Console]::ReadLine(); [Console]::WriteLine('ACK:'+$a); $b=[Console]::ReadLine(); exit 0",
        4096,
    );
    let model = endpoint.model();
    until(|| text(&model).contains("OWNER_READY"));
    let mut invalid = KeyEvent::new(KeyCode::UNIDENTIFIED);
    invalid.text = "x".repeat(MAX_KEY_TEXT_BYTES + 1);
    assert!(complete(endpoint.key_receipt(invalid)).is_err());
    assert!(!endpoint.control.shutdown.load(Ordering::Acquire));
    let mut key = KeyEvent::new(KeyCode::UNIDENTIFIED);
    key.text = "ping".into();
    assert_eq!(complete(endpoint.key_receipt(key)).unwrap(), b"ping");
    assert_eq!(
        complete(endpoint.key_receipt(KeyEvent::new(KeyCode::ENTER))).unwrap(),
        b"\r"
    );
    until(|| text(&model).contains("ACK:ping"));
    assert!(!text(&model).contains("ACK:pingping"));
    endpoint.close().unwrap();
    assert!(matches!(
        complete(endpoint.key_receipt(KeyEvent::new(KeyCode::ENTER))),
        Err(AsyncGhosttyPumpError::Transport(TerminalError::Closed))
    ));
    wait(endpoint).unwrap();
}

#[test]
fn native_local_key_receipt_queue_rejection_and_admitted_cancellation() {
    use oneterm_terminal::ghostty::input::KeyCode;
    let _spawns = crate::spawn_guard::lock_spawns();
    let (endpoint, tx, events) = start(
        "[Console]::WriteLine('OWNER_READY'); $a=[Console]::ReadLine(); [Console]::Write([char]27 + ']2;blocked' + [char]7); $b=[Console]::ReadLine(); exit 0",
        1,
    );
    let model = endpoint.model();
    until(|| {
        while events.try_recv().is_ok() {}
        text(&model).contains("OWNER_READY")
    });
    drop(endpoint.key_receipt(KeyEvent::new(KeyCode::ENTER)));
    assert!(endpoint.inputs.is_empty());
    while events.try_recv().is_ok() {}
    tx.try_send(SessionEvent::Bell).unwrap();
    endpoint.write(b"title\r").unwrap();
    until(|| endpoint.state().title().as_deref() == Some("blocked"));
    let mut receipts: Vec<_> = (0..64)
        .map(|_| Box::pin(endpoint.key_receipt(KeyEvent::new(KeyCode::ENTER))))
        .collect();
    for receipt in &mut receipts {
        assert!(
            receipt
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
    }
    let mut full = Box::pin(endpoint.key_receipt(KeyEvent::new(KeyCode::ENTER)));
    assert!(matches!(
        full.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(AsyncGhosttyPumpError::Transport(
            TerminalError::QueueFull
        )))
    ));
    drop(full);
    assert!(!endpoint.control.shutdown.load(Ordering::Acquire));
    drop(receipts.remove(0));
    assert!(endpoint.control.shutdown.load(Ordering::Acquire));
    until(|| endpoint.inputs.is_empty());
    for mut receipt in receipts {
        until(|| {
            match receipt
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
            {
                Poll::Pending => false,
                Poll::Ready(result) => {
                    assert!(matches!(result, Err(AsyncGhosttyPumpError::Closed)));
                    true
                }
            }
        });
    }
    assert_eq!(
        wait(endpoint).unwrap().lifecycle,
        LifecycleDelivery::Cancelled
    );
}

#[test]
fn native_local_owner_resize_key_and_invalid_controls() {
    use oneterm_terminal::ghostty::input::{KeyCode, MAX_KEY_TEXT_BYTES};
    let _spawns = crate::spawn_guard::lock_spawns();
    let (endpoint, _, _events) = start(
        "[Console]::WriteLine('OWNER_READY'); $a=[Console]::ReadLine(); [Console]::WriteLine('SIZE:'+ [Console]::WindowWidth + ':' + [Console]::WindowHeight + ':ACK:' + $a); $b=[Console]::ReadLine(); exit 0",
        4096,
    );
    let model = endpoint.model();
    until(|| text(&model).contains("OWNER_READY"));
    let mut invalid = KeyEvent::new(KeyCode::UNIDENTIFIED);
    invalid.text = "x".repeat(MAX_KEY_TEXT_BYTES + 1);
    assert!(matches!(endpoint.key(invalid), Err(InputError::Invalid(_))));
    assert!(matches!(
        endpoint.resize(WindowSize {
            cols: 0,
            rows: 1,
            ..Default::default()
        }),
        Err(InputError::Invalid(_))
    ));
    assert!(matches!(
        endpoint.resize(WindowSize {
            cols: u16::MAX,
            rows: u16::MAX,
            ..Default::default()
        }),
        Err(InputError::Invalid(_))
    ));
    let mut invalid_mouse = mouse_press();
    invalid_mouse.x = f32::NAN;
    assert!(matches!(
        endpoint.mouse(invalid_mouse),
        Err(InputError::Invalid(_))
    ));
    assert!(endpoint.state().alive());
    endpoint
        .resize(WindowSize {
            cols: 90,
            rows: 26,
            cell_width: 8,
            cell_height: 16,
        })
        .unwrap();
    endpoint
        .resize(WindowSize {
            cols: 100,
            rows: 30,
            cell_width: 10,
            cell_height: 20,
        })
        .unwrap();
    let mut key = KeyEvent::new(KeyCode::UNIDENTIFIED);
    key.text = "ping".into();
    endpoint.key(key).unwrap();
    endpoint.key(KeyEvent::new(KeyCode::ENTER)).unwrap();
    until(|| text(&model).contains("SIZE:100:30:ACK:ping"));
    let frame = model.reader().poll().unwrap().frame.unwrap();
    assert_eq!((frame.source().cols, frame.source().rows.len()), (100, 30));
    endpoint.key(KeyEvent::new(KeyCode::ENTER)).unwrap();
    assert_eq!(wait(endpoint).unwrap().stop, Stop::ChildExit);
}

fn mouse_press() -> MouseEvent {
    use oneterm_terminal::ghostty::input::{Geometry, Modifiers, MouseAction, MouseButton};
    MouseEvent {
        action: MouseAction::Press,
        button: Some(MouseButton::Left),
        modifiers: Modifiers::default(),
        x: 15.0,
        y: 25.0,
        any_button_pressed: true,
        geometry: Geometry {
            width: 160,
            height: 60,
            cell_width: 10,
            cell_height: 20,
            padding_top: 0,
            padding_bottom: 0,
            padding_left: 0,
            padding_right: 0,
        },
    }
}

#[test]
fn native_local_owner_default_colors_full_queue_cancel_and_pending_ack_cleanup() {
    use oneterm_terminal::ghostty::render::Rgb;
    let _spawns = crate::spawn_guard::lock_spawns();
    let (endpoint, tx, events) = start(
        "[Console]::WriteLine('OWNER_READY'); $a=[Console]::ReadLine(); [Console]::Write([char]27 + ']2;blocked' + [char]7); $b=[Console]::ReadLine(); exit 0",
        1,
    );
    let model = endpoint.model();
    until(|| {
        while events.try_recv().is_ok() {}
        text(&model).contains("OWNER_READY")
    });
    let colors = DefaultColors {
        foreground: Rgb::default(),
        background: Rgb::default(),
        cursor: None,
        palette: [Rgb::default(); 256],
    };
    drop(endpoint.set_default_colors(colors.clone()));
    assert!(endpoint.inputs.is_empty());
    while events.try_recv().is_ok() {}
    tx.try_send(SessionEvent::Bell).unwrap();
    endpoint.write(b"title\r").unwrap();
    until(|| endpoint.state().title().as_deref() == Some("blocked"));
    let good = model.reader().poll().unwrap().frame.unwrap();
    let mut updates: Vec<_> = (0..64)
        .map(|_| Box::pin(endpoint.set_default_colors(colors.clone())))
        .collect();
    for update in &mut updates {
        assert!(
            update
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
    }
    let mut full = Box::pin(endpoint.set_default_colors(colors.clone()));
    assert!(matches!(
        full.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(AsyncGhosttyPumpError::Transport(
            TerminalError::QueueFull
        )))
    ));
    drop(full);
    assert!(!endpoint.control.shutdown.load(Ordering::Acquire));
    drop(updates.remove(0));
    assert!(endpoint.control.shutdown.load(Ordering::Acquire));
    until(|| endpoint.inputs.is_empty());
    for mut update in updates {
        until(|| {
            match update
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
            {
                Poll::Pending => false,
                Poll::Ready(result) => {
                    assert!(matches!(result, Err(AsyncGhosttyPumpError::Closed)));
                    true
                }
            }
        });
    }
    let mut closed = Box::pin(endpoint.set_default_colors(colors));
    assert!(matches!(
        closed
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(AsyncGhosttyPumpError::Transport(TerminalError::Closed)))
    ));
    drop(closed);
    assert!(Arc::ptr_eq(
        &good,
        model.reader().poll().unwrap().frame.as_ref().unwrap()
    ));
    assert_eq!(
        wait(endpoint).unwrap().lifecycle,
        LifecycleDelivery::Cancelled
    );
}

#[test]
fn native_local_owner_selection_full_queue_cancel_and_pending_ack_cleanup() {
    let _spawns = crate::spawn_guard::lock_spawns();
    for clear in [false, true] {
        let (endpoint, tx, events) = start(
            "[Console]::WriteLine('OWNER_READY'); $a=[Console]::ReadLine(); [Console]::Write([char]27 + ']2;blocked' + [char]7); $b=[Console]::ReadLine(); exit 0",
            1,
        );
        let model = endpoint.model();
        until(|| {
            while events.try_recv().is_ok() {}
            text(&model).contains("OWNER_READY")
        });
        drop(endpoint.update_selection(SelectionUpdate::Clear));
        drop(endpoint.clear_command());
        assert!(endpoint.inputs.is_empty());
        while events.try_recv().is_ok() {}
        tx.try_send(SessionEvent::Bell).unwrap();
        endpoint.write(b"title\r").unwrap();
        until(|| endpoint.state().title().as_deref() == Some("blocked"));
        let good = model.reader().poll().unwrap().frame.unwrap();
        let mut selections: Vec<_> = (0..64)
            .map(|_| {
                Box::pin(async {
                    if clear {
                        endpoint.clear_command().await
                    } else {
                        endpoint.update_selection(SelectionUpdate::Clear).await
                    }
                })
            })
            .collect();
        for selection in &mut selections {
            assert!(
                selection
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
        }
        let mut full = Box::pin(async {
            if clear {
                endpoint.clear_command().await
            } else {
                endpoint.update_selection(SelectionUpdate::Clear).await
            }
        });
        assert!(matches!(
            full.as_mut().poll(&mut Context::from_waker(Waker::noop())),
            Poll::Ready(Err(AsyncGhosttyPumpError::Transport(
                TerminalError::QueueFull
            )))
        ));
        drop(full);
        assert!(!endpoint.control.shutdown.load(Ordering::Acquire));
        drop(selections.remove(0));
        assert!(endpoint.control.shutdown.load(Ordering::Acquire));
        until(|| endpoint.inputs.is_empty());
        for mut selection in selections {
            until(|| {
                match selection
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                {
                    Poll::Pending => false,
                    Poll::Ready(result) => {
                        assert!(matches!(result, Err(AsyncGhosttyPumpError::Closed)));
                        true
                    }
                }
            });
        }
        let mut closed = Box::pin(async {
            if clear {
                endpoint.clear_command().await
            } else {
                endpoint.update_selection(SelectionUpdate::Clear).await
            }
        });
        assert!(matches!(
            closed
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Ready(Err(AsyncGhosttyPumpError::Transport(TerminalError::Closed)))
        ));
        drop(closed);
        assert!(Arc::ptr_eq(
            &good,
            model.reader().poll().unwrap().frame.as_ref().unwrap()
        ));
        assert_eq!(
            wait(endpoint).unwrap().lifecycle,
            LifecycleDelivery::Cancelled
        );
    }
}

#[test]
fn native_local_owner_ui_release_executes_queued_controls() {
    use oneterm_terminal::ghostty::input::KeyCode;
    let _spawns = crate::spawn_guard::lock_spawns();
    let (endpoint, tx, events) = start(
        "[Console]::WriteLine('OWNER_READY'); $a=[Console]::ReadLine(); [Console]::Write([char]27 + ']2;blocked' + [char]7); $b=[Console]::ReadLine(); [Console]::WriteLine('SIZE:'+ [Console]::WindowWidth + ':' + [Console]::WindowHeight + ':ACK:' + $b); $c=[Console]::ReadLine(); exit 0",
        1,
    );
    let model = endpoint.model();
    until(|| {
        while events.try_recv().is_ok() {}
        text(&model).contains("OWNER_READY")
    });
    while events.try_recv().is_ok() {}
    tx.try_send(SessionEvent::Bell).unwrap();
    endpoint.write(b"title\r").unwrap();
    until(|| endpoint.state().title().as_deref() == Some("blocked"));
    for cols in 90..94 {
        endpoint
            .resize(WindowSize {
                cols,
                rows: 30,
                ..Default::default()
            })
            .unwrap();
    }
    let mut paste = Box::pin(endpoint.paste("queued"));
    assert!(
        paste
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    endpoint.key(KeyEvent::new(KeyCode::ENTER)).unwrap();
    assert_eq!(events.try_recv().unwrap(), SessionEvent::Bell);
    until(|| {
        while events.try_recv().is_ok() {}
        match paste.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Pending => false,
            Poll::Ready(result) => {
                result.unwrap();
                true
            }
        }
    });
    drop(paste);
    until(|| endpoint.paste_budget.load(Ordering::Acquire) == 0);
    until(|| {
        while events.try_recv().is_ok() {}
        text(&model).contains("SIZE:93:30:ACK:queued")
    });
    endpoint.close().unwrap();
    assert_eq!(wait(endpoint).unwrap().stop, Stop::Requested);
}

#[test]
fn native_local_search_queue_pressure_and_post_admission_cancellation() {
    use oneterm_terminal::ghostty::search::Request;
    let _spawns = crate::spawn_guard::lock_spawns();
    let (endpoint, tx, events) = start(
        "[Console]::WriteLine('OWNER_READY'); $a=[Console]::ReadLine(); [Console]::Write([char]27 + ']2;blocked' + [char]7); $b=[Console]::ReadLine(); exit 0",
        1,
    );
    let model = endpoint.model();
    until(|| {
        while events.try_recv().is_ok() {}
        text(&model).contains("OWNER_READY")
    });
    drop(endpoint.search_request(Request::Step));
    assert!(endpoint.inputs.is_empty());
    while events.try_recv().is_ok() {}
    tx.try_send(SessionEvent::Bell).unwrap();
    endpoint.write(b"title\r").unwrap();
    until(|| endpoint.state().title().as_deref() == Some("blocked"));
    let mut searches: Vec<_> = (0..64)
        .map(|_| Box::pin(endpoint.search_request(Request::Step)))
        .collect();
    for search in &mut searches {
        assert!(
            search
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
    }
    let mut full = Box::pin(endpoint.search_request(Request::Step));
    assert!(matches!(
        full.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(AsyncGhosttyPumpError::Transport(
            TerminalError::QueueFull
        )))
    ));
    drop(full);
    assert!(!endpoint.control.shutdown.load(Ordering::Acquire));
    drop(searches.remove(0));
    assert!(endpoint.control.shutdown.load(Ordering::Acquire));
    for mut search in searches {
        until(|| {
            match search
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
            {
                Poll::Pending => false,
                Poll::Ready(result) => {
                    assert!(matches!(result, Err(AsyncGhosttyPumpError::Closed)));
                    true
                }
            }
        });
    }
    assert_eq!(
        wait(endpoint).unwrap().lifecycle,
        LifecycleDelivery::Cancelled
    );
}

#[test]
fn native_local_owner_modes_mouse_fifo_resize_failure_and_publication_fatal_ack() {
    use oneterm_terminal::ghostty::input::KeyCode;
    use oneterm_terminal::ghostty::render::Rgb;
    for colors_update in [false, true] {
        let poll = Arc::new(Poller::new().unwrap());
        let control = Arc::new(ShellControl::default());
        let (sender, input) = mpsc::sync_channel(LOCAL_COMMAND_QUEUE_CAPACITY);
        let transport = LocalTransport::new();
        transport.set_notifier(ShellNotifier {
            sender,
            poller: poll.clone(),
            control: control.clone(),
        });
        let mut io = OwnerIo {
            pty: Some(MockPty {
                reader: io::Cursor::new(Vec::new()),
                writer: PartialWriter {
                    bytes: Vec::new(),
                    allowance: usize::MAX,
                    fail: false,
                },
                sizes: Vec::new(),
                fail_resize: false,
            }),
            registered: false,
            input,
            pending: None,
            offset: 0,
            poll,
            control,
            admission: Arc::new(Admission {
                gate: Mutex::new(()),
                requested: AtomicBool::new(false),
                sealed: AtomicBool::new(false),
            }),
            exit: None,
            cancelled: false,
            controls: async_channel::bounded(64).1,
        };
        let (tx, rx) = async_channel::bounded(4096);
        let router = OscRouter::new(
            transport.clone(),
            SessionEventSink::new(tx),
            SharedSessionState::new_alive(),
            ClipboardOrigin::Local,
        );
        let mut pump = AsyncGhosttyPump::spawn(router, 80, 24).unwrap();
        assert!(
            io.operation(pump.process_chunk(b"ready\x1b[?1h\x1b[?1000h\x1b[?1006h"))
                .unwrap()
        );
        assert!(io.operation(pump.finish_batch()).unwrap());
        transport.pty_write(b"raw").unwrap();
        assert!(
            io.input(&mut pump, Input::Key(KeyEvent::new(KeyCode::ARROW_UP)))
                .unwrap()
        );
        assert!(io.input(&mut pump, Input::Mouse(mouse_press())).unwrap());
        assert!(
            io.input(&mut pump, Input::Key(KeyEvent::new(KeyCode::ENTER)))
                .unwrap()
        );
        io.writes().unwrap();
        assert_eq!(
            io.pty.as_ref().unwrap().writer.bytes,
            b"raw\x1bOA\x1b[<0;2;2M\r"
        );
        let size = WindowSize {
            cols: 100,
            rows: 30,
            cell_width: 10,
            cell_height: 20,
        };
        assert!(io.input(&mut pump, Input::Resize(size)).unwrap());
        assert_eq!(io.pty.as_ref().unwrap().sizes, vec![size]);
        let colors = Box::new(DefaultColors {
            foreground: Rgb {
                r: 17,
                g: 34,
                b: 51,
            },
            background: Rgb::default(),
            cursor: None,
            palette: [Rgb::default(); 256],
        });
        let (reply, ack) = async_channel::bounded(1);
        assert!(
            io.input(&mut pump, Input::DefaultColors(colors.clone(), reply))
                .unwrap()
        );
        ack.try_recv().unwrap().unwrap();
        let model = pump.model();
        let good = model.reader().poll().unwrap().frame.unwrap();
        assert_eq!((good.source().cols, good.source().rows.len()), (100, 30));
        assert_eq!(good.source().foreground, colors.foreground);
        io.pty.as_mut().unwrap().fail_resize = true;
        assert!(matches!(
            io.input(&mut pump, Input::Resize(WindowSize { cols: 120, ..size })),
            Err(Error::Io(_))
        ));
        assert!(Arc::ptr_eq(
            model.reader().poll().unwrap().frame.as_ref().unwrap(),
            &good
        ));
        assert!(
            io.operation(pump.process_chunk(b"\x1b]2;pending-selection\x07"))
                .unwrap()
        );
        drop(rx);
        let (reply, ack) = async_channel::bounded(1);
        let update = if colors_update {
            Input::DefaultColors(colors, reply)
        } else {
            Input::Selection(SelectionUpdate::Clear, reply)
        };
        assert!(matches!(
            io.input(&mut pump, update),
            Err(Error::Native(AsyncGhosttyPumpError::EventsClosed))
        ));
        assert!(matches!(
            ack.try_recv().unwrap(),
            Err(AsyncGhosttyPumpError::EventsClosed)
        ));
        assert!(Arc::ptr_eq(
            &good,
            model.reader().poll().unwrap().frame.as_ref().unwrap()
        ));
        let shutdown = pump.shutdown();
        io.dispose().unwrap();
        assert!(io.drive(shutdown, false).unwrap().unwrap().is_err());
        assert_eq!(io.control.queued_input_bytes.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn native_local_owner_continuous_output_does_not_starve_resize_or_close() {
    let _spawns = crate::spawn_guard::lock_spawns();
    let (endpoint, _, _events) = start(
        "[Console]::WriteLine('OWNER_READY'); $a=[Console]::ReadLine(); while ($true) { [Console]::WriteLine('FLOOD:' + ('x' * 80)) }",
        4096,
    );
    let model = endpoint.model();
    until(|| text(&model).contains("OWNER_READY"));
    endpoint.write(b"go\r").unwrap();
    until(|| text(&model).contains("FLOOD:"));
    endpoint
        .resize(WindowSize {
            cols: 100,
            rows: 30,
            ..Default::default()
        })
        .unwrap();
    until(|| {
        model
            .reader()
            .poll()
            .unwrap()
            .frame
            .is_some_and(|frame| frame.source().cols == 100 && frame.source().rows.len() == 30)
    });
    endpoint.close().unwrap();
    assert_eq!(wait(endpoint).unwrap().stop, Stop::Requested);
}

#[test]
fn native_local_owner_paste_active_budget_and_transport_failure_ack() {
    let poll = Arc::new(Poller::new().unwrap());
    let control = Arc::new(ShellControl::default());
    let (sender, input) = mpsc::sync_channel(LOCAL_COMMAND_QUEUE_CAPACITY);
    let transport = LocalTransport::new();
    transport.set_notifier(ShellNotifier {
        sender,
        poller: poll.clone(),
        control: control.clone(),
    });
    let mut io = OwnerIo {
        pty: Some(MockPty {
            reader: io::Cursor::new(Vec::new()),
            writer: PartialWriter {
                bytes: Vec::new(),
                allowance: 0,
                fail: false,
            },
            sizes: Vec::new(),
            fail_resize: false,
        }),
        registered: false,
        input,
        pending: None,
        offset: 0,
        poll,
        control,
        admission: Arc::new(Admission {
            gate: Mutex::new(()),
            requested: AtomicBool::new(false),
            sealed: AtomicBool::new(false),
        }),
        exit: None,
        cancelled: false,
        controls: async_channel::bounded(64).1,
    };
    let (tx, _rx) = async_channel::bounded(4096);
    let router = OscRouter::new(
        transport.clone(),
        SessionEventSink::new(tx),
        SharedSessionState::new_alive(),
        ClipboardOrigin::Local,
    );
    let mut pump = AsyncGhosttyPump::spawn(router, 80, 24).unwrap();
    assert!(io.operation(pump.finish_batch()).unwrap());
    let model = pump.model();
    let good = model.reader().poll().unwrap().frame.unwrap();
    let budget = Arc::new(ByteBudget::<PASTE_BYTE_BUDGET>::default());
    let make = |text: &str| {
        assert!(budget.reserve(text.len()));
        let (tx, rx) = async_channel::bounded(1);
        (
            Input::Paste(Paste {
                reveal: false,
                text: text.to_owned(),
                result: tx,
                _reservation: PasteReservation {
                    budget: budget.clone(),
                    bytes: text.len(),
                },
            }),
            rx,
        )
    };
    let (paste, ack) = make("accepted");
    assert_eq!(budget.load(Ordering::Acquire), 8);
    assert!(io.input(&mut pump, paste).unwrap());
    ack.try_recv().unwrap().unwrap();
    assert_eq!(budget.load(Ordering::Acquire), 0);
    // The original FIFO includes this accepted paste. Saturate the remainder
    // with raw bytes; the blocked writer cannot release transport reservations.
    transport
        .pty_write(&vec![b'x'; LOCAL_COMMAND_BYTE_BUDGET - 8])
        .unwrap();
    let (paste, ack) = make("rejected");
    assert!(matches!(
        io.input(&mut pump, paste),
        Err(Error::Native(AsyncGhosttyPumpError::Transport(
            TerminalError::QueueFull
        )))
    ));
    assert!(matches!(
        ack.try_recv().unwrap(),
        Err(AsyncGhosttyPumpError::Transport(TerminalError::QueueFull))
    ));
    assert_eq!(budget.load(Ordering::Acquire), 0);
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));
    let shutdown = pump.shutdown();
    io.dispose().unwrap();
    assert!(io.drive(shutdown, false).unwrap().unwrap().is_err());
    assert_eq!(io.control.queued_input_bytes.load(Ordering::Acquire), 0);
}

#[test]
fn native_local_raw_visible_shares_paste_budget_and_cancels_without_write() {
    let _spawns = crate::spawn_guard::lock_spawns();
    let (endpoint, tx, events) = start(
        "[Console]::WriteLine('OWNER_READY'); $a=[Console]::ReadLine(); [Console]::Write([char]27 + ']2;blocked' + [char]7); $b=[Console]::ReadLine(); exit 0",
        1,
    );
    let model = endpoint.model();
    until(|| {
        while events.try_recv().is_ok() {}
        text(&model).contains("OWNER_READY")
    });
    drop(endpoint.write_visible(b"unpolled"));
    assert!(endpoint.inputs.is_empty());
    assert!(matches!(
        complete(endpoint.write_visible(&vec![0; 1024 * 1024 + 1])),
        Err(AsyncGhosttyPumpError::Paste(_))
    ));
    while events.try_recv().is_ok() {}
    tx.try_send(SessionEvent::Bell).unwrap();
    endpoint.write(b"title\r").unwrap();
    until(|| endpoint.state().title().as_deref() == Some("blocked"));
    let good = model.reader().poll().unwrap().frame.unwrap();
    let payload = "x".repeat(1024 * 1024);
    let mut inputs: Vec<_> = (0..3)
        .map(|_| Box::pin(endpoint.write_visible(payload.as_bytes())))
        .collect();
    for input in &mut inputs {
        assert!(
            input
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
    }
    let mut paste = Box::pin(endpoint.paste(&payload));
    assert!(
        paste
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    assert_eq!(
        endpoint.paste_budget.load(Ordering::Acquire),
        PASTE_BYTE_BUDGET
    );
    assert!(matches!(
        complete(endpoint.write_visible(b"full")),
        Err(AsyncGhosttyPumpError::Transport(TerminalError::QueueFull))
    ));
    assert!(!endpoint.control.shutdown.load(Ordering::Acquire));
    drop(inputs.remove(0));
    assert!(endpoint.control.shutdown.load(Ordering::Acquire));
    drop((inputs, paste));
    let budget = endpoint.paste_budget.clone();
    assert_eq!(wait(endpoint).unwrap().stop, Stop::Requested);
    assert_eq!(budget.load(Ordering::Acquire), 0);
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));
}

#[test]
fn native_local_owner_paste_budget_cancel_and_unpolled_cleanup() {
    let _spawns = crate::spawn_guard::lock_spawns();
    let (endpoint, tx, events) = start(
        "[Console]::WriteLine('OWNER_READY'); $a=[Console]::ReadLine(); [Console]::Write([char]27 + ']2;blocked' + [char]7); $b=[Console]::ReadLine(); exit 0",
        1,
    );
    let model = endpoint.model();
    until(|| {
        while events.try_recv().is_ok() {}
        text(&model).contains("OWNER_READY")
    });
    let never = endpoint.paste("never queued");
    drop(never);
    assert!(endpoint.inputs.is_empty());
    let oversized = "x".repeat(1024 * 1024 + 1);
    let mut invalid = Box::pin(endpoint.paste(&oversized));
    assert!(matches!(
        invalid
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(AsyncGhosttyPumpError::Paste(_)))
    ));
    drop(invalid);
    assert_eq!(endpoint.paste_budget.load(Ordering::Acquire), 0);
    assert!(!endpoint.control.shutdown.load(Ordering::Acquire));
    while events.try_recv().is_ok() {}
    tx.try_send(SessionEvent::Bell).unwrap();
    endpoint.write(b"title\r").unwrap();
    until(|| endpoint.state().title().as_deref() == Some("blocked"));
    let good = model.reader().poll().unwrap().frame.unwrap();
    let text = "x".repeat(1024 * 1024);
    let mut pastes: Vec<_> = (0..4).map(|_| Box::pin(endpoint.paste(&text))).collect();
    for paste in &mut pastes {
        assert!(
            paste
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
    }
    assert_eq!(
        endpoint.paste_budget.load(Ordering::Acquire),
        PASTE_BYTE_BUDGET
    );
    endpoint.scroll(Scroll::Top).unwrap();
    let mut full = Box::pin(endpoint.paste("x"));
    assert!(matches!(
        full.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(AsyncGhosttyPumpError::Transport(
            TerminalError::QueueFull
        )))
    ));
    drop(full);
    assert!(!endpoint.control.shutdown.load(Ordering::Acquire));
    drop(pastes.remove(0));
    assert!(endpoint.control.shutdown.load(Ordering::Acquire));
    drop(pastes);
    let budget = endpoint.paste_budget.clone();
    assert_eq!(wait(endpoint).unwrap().stop, Stop::Requested);
    assert_eq!(budget.load(Ordering::Acquire), 0);
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));
}
