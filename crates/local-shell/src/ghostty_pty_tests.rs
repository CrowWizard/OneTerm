//! Real ConPTY fixture, not the shipping ShellEventLoop or session contract.

use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use oneterm_terminal::backend::AsyncGhosttyPump;
use oneterm_terminal::{
    ClipboardOrigin, OscRouter, PtyTransport, SessionEvent, SessionEventSink, SharedSessionState,
    TerminalError,
};
use oneterm_vt::pty::{EventedReadWrite, Shell};

use super::*;

struct ThreadWake(std::thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}

struct RegisteredPty {
    pty: PseudoConsole,
    poll: Arc<Poller>,
}

impl Drop for RegisteredPty {
    fn drop(&mut self) {
        // Deregister before either source or poller dies, including assertions.
        self.pty.deregister(&self.poll).unwrap();
    }
}

struct Fixture {
    console: RegisteredPty,
    pump: AsyncGhosttyPump<LocalTransport>,
    input: mpsc::Receiver<Cow<'static, [u8]>>,
    control: Arc<ShellControl>,
    text: String,
    exit: Option<std::process::ExitStatus>,
    output_bytes: usize,
}

impl Fixture {
    fn step(&mut self) {
        while let Ok(bytes) = self.input.try_recv() {
            // Writer acceptance is not physical child receipt; ACK is checked
            // separately through the real console output and native frame.
            self.console.pty.writer().write_all(&bytes).unwrap();
            self.control.queued_input_bytes.release(bytes.len());
        }
        let mut events = Events::with_capacity(POLL_EVENT_CAPACITY);
        self.console
            .poll
            .wait(&mut events, Some(Duration::from_millis(50)))
            .unwrap();
        let mut bytes = [0; 8192];
        loop {
            match self.console.pty.reader().read(&mut bytes) {
                Ok(0) => break,
                Ok(n) => {
                    self.output_bytes += n;
                    assert!(self.output_bytes <= 128 * 1024, "unexpected console flood");
                    block_on(self.pump.process_chunk(&bytes[..n])).unwrap();
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => break,
                Err(error) => panic!("PTY read: {error}"),
            }
        }
        // Drain completely before rearming. A parked reader with buffered data
        // would lose the next notification on the Windows ring reader.
        self.console
            .pty
            .reregister(
                &self.console.poll,
                PollEvent::readable(PTY_READ_WRITE_TOKEN),
                PollMode::Level,
            )
            .unwrap();
        if let Some(ChildEvent::Exited(status)) = self.console.pty.next_child_event() {
            self.exit = Some(status.expect("child exit status"));
        }
        let frame = block_on(self.pump.finish_batch()).unwrap();
        self.text = frame
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
            .join("\n");
    }

    fn until(&mut self, predicate: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !predicate(self) {
            assert!(
                Instant::now() < deadline,
                "PTY fixture timed out; native paint: {:?}; exit: {:?}",
                self.text,
                self.exit
            );
            self.step();
        }
    }
}

#[test]
fn native_local_pty_input_resize_and_child_exit() {
    let _spawns = crate::spawn_guard::lock_spawns();
    let program = format!(
        "{}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
        std::env::var("SystemRoot").unwrap()
    );
    let script = "[Console]::WriteLine('NATIVE_READY'); $a=[Console]::ReadLine(); [Console]::WriteLine('ACK:'+ $a); $b=[Console]::ReadLine(); [Console]::WriteLine('SIZE:'+ [Console]::WindowWidth + ':' + [Console]::WindowHeight); $c=[Console]::ReadLine(); exit 0";
    let opts = Options {
        shell: Some(Shell::new(
            program,
            vec![
                "-NoLogo".into(),
                "-NoProfile".into(),
                "-Command".into(),
                script.into(),
            ],
        )),
        escape_args: true,
        ..Default::default()
    };
    let poll = Arc::new(Poller::new().unwrap());
    let mut pty = PseudoConsole::spawn(
        &opts,
        WindowSize {
            cols: 80,
            rows: 24,
            ..Default::default()
        },
    )
    .unwrap();
    // SAFETY: RegisteredPty retains both sources and poller and deregisters in
    // Drop before destroying either, on this same owner test thread.
    unsafe {
        pty.register(
            &poll,
            PollEvent::readable(PTY_READ_WRITE_TOKEN),
            PollMode::Level,
        )
        .unwrap();
    }
    let console = RegisteredPty {
        pty,
        poll: poll.clone(),
    };
    let (sender, input) = mpsc::sync_channel(LOCAL_COMMAND_QUEUE_CAPACITY);
    let control = Arc::new(ShellControl::default());
    let transport = LocalTransport::new();
    transport.set_notifier(ShellNotifier {
        sender,
        poller: poll,
        control: control.clone(),
    });
    let (tx, _events) = async_channel::bounded::<SessionEvent>(4096);
    let router = OscRouter::new(
        transport.clone(),
        SessionEventSink::new(tx),
        SharedSessionState::new_alive(),
        ClipboardOrigin::Local,
    );
    let pump = AsyncGhosttyPump::spawn(router, 80, 24).unwrap();
    let mut fixture = Fixture {
        console,
        pump,
        input,
        control,
        text: String::new(),
        exit: None,
        output_bytes: 0,
    };
    fixture.until(|f| f.text.contains("NATIVE_READY"));
    transport.pty_write(b"pi").unwrap();
    transport.pty_write(b"ng\r").unwrap();
    assert_eq!(
        fixture.control.queued_input_bytes.load(Ordering::Relaxed),
        5
    );
    fixture.until(|f| f.text.contains("ACK:ping"));
    assert_eq!(
        fixture.control.queued_input_bytes.load(Ordering::Relaxed),
        0
    );
    block_on(fixture.pump.resize(90, 26, 10, 20)).unwrap();
    block_on(fixture.pump.resize(100, 30, 10, 20)).unwrap();
    let size = fixture
        .control
        .pending_resize
        .lock()
        .unwrap()
        .take()
        .unwrap();
    assert_eq!((size.cols, size.rows), (100, 30));
    fixture.console.pty.on_resize(size).unwrap();
    transport.pty_write(b"size\r").unwrap();
    fixture.until(|f| f.text.contains("SIZE:100:30"));
    let frame = block_on(fixture.pump.finish_batch()).unwrap();
    assert_eq!((frame.source().cols, frame.source().rows.len()), (100, 30));
    transport.pty_write(b"exit\r").unwrap();
    fixture.until(|f| f.exit.is_some());
    assert!(fixture.exit.unwrap().success());
    block_on(fixture.pump.shutdown()).unwrap();
    assert!(matches!(
        fixture.pump.model().reader().poll().unwrap().state,
        oneterm_terminal::ghostty_bridge::State::Closed
    ));
    assert!(fixture.control.shutdown.load(Ordering::Acquire));
    assert_eq!(transport.pty_write(b"never"), Err(TerminalError::Closed));
    assert_eq!(
        fixture.control.queued_input_bytes.load(Ordering::Relaxed),
        0
    );
}
