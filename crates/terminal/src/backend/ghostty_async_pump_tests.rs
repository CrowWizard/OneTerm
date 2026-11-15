use std::future::Future;
use std::sync::{Mutex, PoisonError};
use std::task::{Context, Poll, Wake, Waker};

use crate::ghostty::owner::Damage;
use crate::ghostty_bridge::State;
use crate::ghostty_model::ModelReader;
use crate::{ClipboardOrigin, SessionEvent};

use super::super::{SessionEventSink, SharedSessionState};
use super::*;

struct ThreadWake(std::thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}

#[derive(Default)]
struct Observations {
    writes: Vec<Vec<u8>>,
    sizes: Vec<(u16, u16)>,
    closes: usize,
    write_error: Option<TerminalError>,
    resize_error: Option<TerminalError>,
    close_error: Option<TerminalError>,
    reader: Option<ModelReader>,
    frames_at_write: Vec<Option<Arc<ProjectedFrame>>>,
}

#[derive(Clone, Default)]
struct Transport(Arc<Mutex<Observations>>);
impl Transport {
    fn lock(&self) -> std::sync::MutexGuard<'_, Observations> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl PtyTransport for Transport {
    fn pty_write(&self, bytes: &[u8]) -> Result<(), TerminalError> {
        let mut state = self.lock();
        let frame = state.reader.as_mut().unwrap().poll().unwrap().frame;
        state.frames_at_write.push(frame);
        state.writes.push(bytes.to_vec());
        state.write_error.clone().map_or(Ok(()), Err)
    }
    fn pty_resize(&self, rows: u16, cols: u16) -> Result<(), TerminalError> {
        let mut state = self.lock();
        state.sizes.push((rows, cols));
        state.resize_error.clone().map_or(Ok(()), Err)
    }
    fn pty_close(&self) -> Result<(), TerminalError> {
        let mut state = self.lock();
        state.closes += 1;
        state.close_error.clone().map_or(Ok(()), Err)
    }
}

fn pump(
    capacity: usize,
) -> (
    AsyncGhosttyPump<Transport>,
    Transport,
    async_channel::Sender<SessionEvent>,
    async_channel::Receiver<SessionEvent>,
) {
    let transport = Transport::default();
    let (tx, rx) = async_channel::bounded(capacity);
    let router = OscRouter::new(
        transport.clone(),
        SessionEventSink::new(tx.clone()),
        SharedSessionState::new_alive(),
        ClipboardOrigin::Remote,
    );
    let pump = AsyncGhosttyPump::spawn(router, 16, 3).unwrap();
    transport.lock().reader = Some(pump.model().reader());
    (pump, transport, tx, rx)
}

#[test]
fn native_logging_drop_flushes_with_a_retained_controller() {
    let (mut pump, _, _, _rx) = pump(32);
    let directory =
        std::env::temp_dir().join(format!("native-pump-drop-log-{}", std::process::id()));
    let logging = pump.logging();
    let path = logging
        .start(&oneterm_core::TerminalLogConfig {
            enabled: true,
            directory: directory.clone(),
            ..Default::default()
        })
        .unwrap();
    block_on(pump.process_chunk(b"partial output")).unwrap();
    drop(pump);
    assert_eq!(logging.state(), crate::TerminalLogState::Stopped);
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("partial output")
    );
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn native_logging_strips_fragmented_controls_and_flushes_partial_output() {
    let (mut pump, _, _, _rx) = pump(32);
    let directory = std::env::temp_dir().join(format!("native-pump-log-{}", std::process::id()));
    let logging = pump.logging();
    logging.set_identity("native-test");
    let path = logging
        .start(&oneterm_core::TerminalLogConfig {
            enabled: true,
            directory: directory.clone(),
            ..Default::default()
        })
        .unwrap();
    for chunk in [b"visible\x1b]2;sec".as_slice(), b"ret\x07\x1b[31m tail"] {
        block_on(pump.process_chunk(chunk)).unwrap();
    }
    block_on(pump.shutdown()).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("visible tail"));
    assert!(!text.contains("secret"));
    assert!(!text.contains('\x1b'));
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn raw_visible_preserves_exact_bytes_reveals_before_write_and_never_paste_encodes() {
    let (mut pump, transport, _, _rx) = pump(32);
    block_on(pump.process_chunk(b"first\r\nsecond\r\nthird\r\nfourth\x1b[?2004h")).unwrap();
    let bottom = block_on(pump.finish_batch()).unwrap();
    block_on(pump.scroll_viewport(state::Scroll::Top)).unwrap();
    assert_ne!(
        pump.model().query().viewport.unwrap().metadata().scrollbar,
        bottom.source().metadata.scrollbar
    );
    let bytes = b"\x1bOA\0\xff\r\n\x03";
    block_on(pump.write_visible(bytes)).unwrap();
    let observed = transport.lock();
    assert_eq!(observed.writes, [bytes.to_vec()]);
    assert_eq!(
        observed.frames_at_write[0]
            .as_ref()
            .unwrap()
            .source()
            .metadata
            .scrollbar
            .offset,
        bottom.source().metadata.scrollbar.offset
    );
    drop(observed);
    block_on(pump.write_visible(b"")).unwrap();
    assert_eq!(transport.lock().writes.len(), 1);
    assert!(matches!(
        block_on(pump.write_visible(&vec![0; PastePolicy::default().max_bytes + 1])),
        Err(Error::Paste(_))
    ));
    assert_eq!(transport.lock().closes, 0);
    transport.lock().write_error = Some(TerminalError::QueueFull);
    assert!(matches!(
        block_on(pump.write_visible(b"failure")),
        Err(Error::Transport(TerminalError::QueueFull))
    ));
    assert_eq!(transport.lock().closes, 1);
    assert!(matches!(
        block_on(pump.write_visible(b"retry")),
        Err(Error::Closed)
    ));
}

#[test]
fn raw_visible_reliable_event_backpressure_cancel_never_writes() {
    for cancel in [false, true] {
        let (mut pump, transport, tx, rx) = pump(1);
        block_on(pump.finish_batch()).unwrap();
        rx.try_recv().unwrap();
        tx.try_send(SessionEvent::Bell).unwrap();
        block_on(pump.process_chunk(b"\x1b]2;before-raw\x07")).unwrap();
        let mut raw = Box::pin(pump.write_visible(b"must-not-write"));
        assert!(
            raw.as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        if cancel {
            drop(raw);
        } else {
            drop(rx);
            assert!(matches!(block_on(raw), Err(Error::EventsClosed)));
        }
        assert!(transport.lock().writes.is_empty());
        assert_eq!(transport.lock().closes, 1);
    }
}

#[test]
fn key_receipt_matches_single_origin_write_and_live_native_modes() {
    use crate::ghostty::input::{KeyAction, KeyCode, KeyEvent};
    let (mut pump, transport, _, _rx) = pump(32);
    block_on(pump.finish_batch()).unwrap();
    for (mode, expected) in [
        (b"\x1b[?1l".as_slice(), b"\x1b[A".as_slice()),
        (b"\x1b[?1h".as_slice(), b"\x1bOA".as_slice()),
    ] {
        block_on(pump.process_chunk(mode)).unwrap();
        let receipt = block_on(pump.key_receipt(KeyEvent::new(KeyCode::ARROW_UP))).unwrap();
        assert_eq!(receipt, expected);
        assert_eq!(transport.lock().writes.last().unwrap(), &receipt);
    }
    assert_eq!(transport.lock().writes.len(), 2);
    let mut release = KeyEvent::new(KeyCode::ARROW_UP);
    release.action = KeyAction::Release;
    assert!(block_on(pump.key_receipt(release)).unwrap().is_empty());
    assert_eq!(
        transport.lock().writes.len(),
        2,
        "empty encoding never writes"
    );
    transport.lock().write_error = Some(TerminalError::QueueFull);
    assert!(matches!(
        block_on(pump.key_receipt(KeyEvent::new(KeyCode::ENTER))),
        Err(Error::Transport(TerminalError::QueueFull))
    ));
    assert_eq!(
        transport.lock().writes.len(),
        3,
        "failed receipt never retries"
    );
    assert_eq!(transport.lock().closes, 1);
    assert!(matches!(
        block_on(pump.key_receipt(KeyEvent::new(KeyCode::ENTER))),
        Err(Error::Closed)
    ));
}

#[test]
fn key_receipt_unpolled_invalid_and_cancelled_operations_never_write() {
    use crate::ghostty::input::{KeyCode, KeyEvent, MAX_KEY_TEXT_BYTES};
    let (mut pump, transport, _, _rx) = pump(8);
    block_on(pump.finish_batch()).unwrap();
    drop(pump.key_receipt(KeyEvent::new(KeyCode::ENTER)));
    let mut invalid = KeyEvent::new(KeyCode::UNIDENTIFIED);
    invalid.text = "x".repeat(MAX_KEY_TEXT_BYTES + 1);
    assert!(block_on(pump.key_receipt(invalid)).is_err());
    assert_eq!(transport.lock().closes, 0);
    let (entered, receiver) = std::sync::mpsc::sync_channel(1);
    let (release, wait) = std::sync::mpsc::sync_channel(1);
    let barrier = pump.engine.request(Command::Wait(entered, wait)).unwrap();
    receiver.recv().unwrap();
    let mut receipt = Box::pin(pump.key_receipt(KeyEvent::new(KeyCode::ENTER)));
    assert!(
        receipt
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(receipt);
    assert_eq!(transport.lock().closes, 1);
    release.send(()).unwrap();
    block_on(barrier.wait()).unwrap();
    assert!(block_on(pump.shutdown()).is_err());
    assert!(transport.lock().writes.is_empty());
}

#[test]
fn clear_command_exact_bytes_preserves_history_and_clears_selection_even_on_write_error() {
    for fail in [false, true] {
        let (mut pump, transport, _, _rx) = pump(32);
        block_on(pump.process_chunk(b"first\r\nsecond\r\nthird\r\nfourth\x1b[?2004h")).unwrap();
        block_on(pump.finish_batch()).unwrap();
        let model = pump.model();
        block_on(pump.update_selection(selection::Update::All)).unwrap();
        let before = block_on(model.selection_text()).unwrap();
        assert!(before.as_deref().unwrap().contains("first"));
        if fail {
            transport.lock().write_error = Some(TerminalError::QueueFull);
        }
        let result = block_on(pump.clear_command());
        if fail {
            assert!(matches!(
                result,
                Err(Error::Transport(TerminalError::QueueFull))
            ));
            assert!(matches!(block_on(pump.clear_command()), Err(Error::Closed)));
            assert_eq!(transport.lock().closes, 1);
        } else {
            result.unwrap();
        }
        assert_eq!(transport.lock().writes, [b"clear\r".to_vec()]);
        // Publication remains readable after fail-stop transport shutdown.
        let frame = model.reader().poll().unwrap().frame.unwrap();
        assert!(
            frame
                .source()
                .rows
                .iter()
                .flat_map(|row| &row.cells)
                .all(|cell| !cell.selected)
        );
        if !fail {
            assert_eq!(block_on(model.selection_text()).unwrap(), None);
            block_on(pump.update_selection(selection::Update::All)).unwrap();
            assert_eq!(block_on(model.selection_text()).unwrap(), before);
            block_on(pump.shutdown()).unwrap();
        }
    }
}

#[test]
fn clear_command_waits_for_prior_events_and_never_writes_after_cancel_or_closed_consumer() {
    for cancel in [false, true] {
        let (mut pump, transport, tx, rx) = pump(1);
        let good = block_on(pump.finish_batch()).unwrap();
        rx.try_recv().unwrap();
        tx.try_send(SessionEvent::Bell).unwrap();
        block_on(pump.process_chunk(b"\x1b]2;before-clear\x07")).unwrap();
        let model = pump.model();
        let mut clear = Box::pin(pump.clear_command());
        assert!(
            clear
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        assert!(transport.lock().writes.is_empty());
        if cancel {
            drop(clear);
        } else {
            drop(rx);
            assert!(matches!(block_on(clear), Err(Error::EventsClosed)));
        }
        assert!(transport.lock().writes.is_empty());
        assert_eq!(transport.lock().closes, 1);
        assert!(Arc::ptr_eq(
            &good,
            model.reader().poll().unwrap().frame.as_ref().unwrap()
        ));
        assert!(matches!(block_on(pump.clear_command()), Err(Error::Closed)));
        assert!(block_on(pump.shutdown()).is_err());
    }
}

#[test]
fn search_navigation_rejects_unpublished_mutation_and_recovers_after_limits() {
    use crate::ghostty::search::{MAX_NEEDLE_BYTES, Options, Status};
    let (mut pump, transport, _, _rx) = pump(32);
    block_on(pump.process_chunk(b"needle\r\none\r\ntwo\r\nthree")).unwrap();
    block_on(pump.finish_batch()).unwrap();
    let model = pump.model();
    let mut found = block_on(pump.search("needle".into(), Options::default())).unwrap();
    for _ in 0..64 {
        if found.status == Status::Complete {
            break;
        }
        found = block_on(pump.search_step()).unwrap();
    }
    assert_eq!(found.status, Status::Complete);
    assert_eq!(found.matches.len(), 1);
    assert!(model.query().matches_search(&found));
    let expired = found.navigation(0).unwrap();
    let revealed = block_on(pump.navigate_search(found.navigation(0).unwrap())).unwrap();
    assert!(model.query().matches_search(&revealed));
    assert!(!model.query().matches_search(&found));
    assert_eq!(
        model.query().viewport.unwrap().metadata().scrollbar.offset,
        0
    );
    assert!(
        block_on(pump.navigate_search(expired))
            .unwrap_err()
            .is_search_rejection()
    );
    block_on(pump.process_chunk(b"\r\nchanged")).unwrap();
    // The retained viewport still has the old revision. Query must not accept it.
    assert!(model.query().viewport.unwrap().matches_search(&revealed));
    assert!(!model.query().matches_search(&revealed));
    assert!(
        block_on(pump.navigate_search(revealed.navigation(0).unwrap()))
            .unwrap_err()
            .is_search_rejection()
    );
    assert!(
        block_on(pump.search("x".repeat(MAX_NEEDLE_BYTES + 1), Options::default()))
            .unwrap_err()
            .is_search_rejection()
    );
    assert!(
        block_on(pump.search(
            "x".into(),
            Options {
                case_sensitive: true,
                whole_word: false
            }
        ))
        .unwrap_err()
        .is_search_rejection()
    );
    let cleared = block_on(pump.search(String::new(), Options::default())).unwrap();
    assert!(cleared.matches.is_empty());
    assert_eq!(transport.lock().closes, 0);
    block_on(pump.finish_batch()).unwrap();
    assert!(model.query().matches_search(&cleared));
    block_on(pump.resize(100, 50, 10, 20)).unwrap();
    block_on(pump.process_chunk(&vec![b'x'; 5000])).unwrap();
    assert!(
        block_on(pump.search("x".into(), Options::default()))
            .unwrap_err()
            .is_search_rejection()
    );
    assert!(
        block_on(pump.search(String::new(), Options::default()))
            .unwrap()
            .matches
            .is_empty()
    );
    assert_eq!(transport.lock().closes, 0);
    block_on(pump.close()).unwrap();
}

#[test]
fn default_colors_publish_full_damage_and_native_query_reply_without_more_output() {
    use crate::ghostty::render::Rgb;
    let (mut pump, transport, _, rx) = pump(8);
    let before = block_on(pump.finish_batch()).unwrap();
    rx.try_recv().unwrap();
    pump.router.state().release_repaint_hint();
    let model = pump.model();
    let mut first = model.reader();
    let mut second = model.reader();
    first.poll().unwrap();
    second.poll().unwrap();
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
    let after = block_on(pump.set_default_colors(Box::new(colors.clone()))).unwrap();
    assert_eq!(after.source().foreground, colors.foreground);
    assert_eq!(after.source().background, colors.background);
    assert_eq!(after.source().cursor_color, colors.cursor);
    assert_eq!(after.source().palette, colors.palette);
    assert_eq!(first.poll().unwrap().damage, Damage::Full);
    assert_eq!(second.poll().unwrap().damage, Damage::Full);
    assert_eq!(first.poll().unwrap().damage, Damage::None);
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Output);
    assert!(transport.lock().writes.is_empty());
    block_on(pump.process_chunk(b"\x1b]10;?\x07")).unwrap();
    assert_eq!(
        transport.lock().writes,
        vec![b"\x1b]10;rgb:1111/2222/3333\x07".to_vec()]
    );
    assert!(!Arc::ptr_eq(&before, &after));
    assert_ne!(before.source().foreground, colors.foreground);
    block_on(pump.shutdown()).unwrap();
}

#[test]
fn default_colors_wait_for_reliable_events_and_cancel_with_last_good() {
    use crate::ghostty::render::Rgb;
    for cancel in [false, true] {
        let (mut pump, transport, tx, rx) = pump(2);
        let good = block_on(pump.finish_batch()).unwrap();
        rx.try_recv().unwrap();
        pump.router.state().release_repaint_hint();
        tx.try_send(SessionEvent::Bell).unwrap();
        tx.try_send(SessionEvent::Bell).unwrap();
        block_on(pump.process_chunk(b"\x1b]2;before-colors\x07")).unwrap();
        let model = pump.model();
        let colors = Box::new(DefaultColors {
            foreground: Rgb { r: 1, g: 2, b: 3 },
            background: Rgb::default(),
            cursor: None,
            palette: [Rgb::default(); 256],
        });
        let mut update = Box::pin(pump.set_default_colors(colors));
        assert!(
            update
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        assert!(Arc::ptr_eq(
            &good,
            model.reader().poll().unwrap().frame.as_ref().unwrap()
        ));
        if cancel {
            drop(update);
            assert_eq!(transport.lock().closes, 1);
            assert!(Arc::ptr_eq(
                &good,
                model.reader().poll().unwrap().frame.as_ref().unwrap()
            ));
            assert!(block_on(pump.shutdown()).is_err());
        } else {
            assert_eq!(rx.try_recv().unwrap(), SessionEvent::Bell);
            assert_eq!(rx.try_recv().unwrap(), SessionEvent::Bell);
            block_on(update).unwrap();
            assert_eq!(
                rx.try_recv().unwrap(),
                SessionEvent::Title("before-colors".into())
            );
            assert_eq!(rx.try_recv().unwrap(), SessionEvent::Output);
            block_on(pump.shutdown()).unwrap();
        }
    }
}

#[test]
fn selection_publication_highlight_copy_rectangle_and_clear_without_output() {
    use crate::ghostty::anchors::{Coordinate, Point, Space};
    let (mut pump, transport, _, rx) = pump(16);
    block_on(pump.process_chunk(b"abcdef\r\nghijkl")).unwrap();
    let initial = block_on(pump.finish_batch()).unwrap();
    rx.try_recv().unwrap();
    pump.router.state().release_repaint_hint();
    let model = pump.model();
    let start = block_on(model.create_anchor(Point {
        space: Space::Active,
        coordinate: Coordinate { column: 1, row: 0 },
    }))
    .unwrap();
    let end = block_on(model.create_anchor(Point {
        space: Space::Active,
        coordinate: Coordinate { column: 3, row: 1 },
    }))
    .unwrap();
    let linear = block_on(pump.update_selection(selection::Update::Set {
        start: start.clone(),
        end: end.clone(),
        rectangle: false,
    }))
    .unwrap();
    assert_eq!(
        block_on(model.selection_text()).unwrap().as_deref(),
        Some("bcdef\nghij")
    );
    assert!(!initial.source().rows[0].cells[1].selected);
    assert!(linear.source().rows[0].cells[1].selected);
    assert!(linear.source().rows[1].cells[3].selected);
    assert!(!linear.source().rows[1].cells[4].selected);
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Output);
    let rectangle = block_on(pump.update_selection(selection::Update::Set {
        start: end.clone(),
        end: start.clone(),
        rectangle: true,
    }))
    .unwrap();
    assert_eq!(
        block_on(model.selection_text()).unwrap().as_deref(),
        Some("bcd\nhij")
    );
    assert_eq!(
        rectangle
            .source()
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .filter(|cell| cell.selected)
            .count(),
        6
    );
    assert!(
        rx.is_empty(),
        "repaint hint coalesces until consumer release"
    );
    block_on(model.release_anchor(&start)).unwrap();
    block_on(model.release_anchor(&end)).unwrap();
    pump.router.state().release_repaint_hint();
    let clear = block_on(pump.update_selection(selection::Update::Clear)).unwrap();
    assert!(
        clear
            .source()
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .all(|cell| !cell.selected)
    );
    assert_eq!(block_on(model.selection_text()).unwrap(), None);
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Output);
    assert!(transport.lock().writes.is_empty());
    block_on(pump.shutdown()).unwrap();
}

#[test]
fn selection_rejection_preserves_frame_and_selection_without_closing_pump() {
    use crate::ghostty::anchors::{Coordinate, Point, Space};
    let (mut pump, transport, _, _) = pump(16);
    block_on(pump.process_chunk(b"abc")).unwrap();
    let model = pump.model();
    let point = Point {
        space: Space::Active,
        coordinate: Coordinate { column: 0, row: 0 },
    };
    let start = block_on(model.create_anchor(point)).unwrap();
    let good = block_on(pump.update_selection(selection::Update::Set {
        start: start.clone(),
        end: start.clone(),
        rectangle: false,
    }))
    .unwrap();
    let (mut other, _, _, _) = self::pump(8);
    let foreign = block_on(other.model().create_anchor(point)).unwrap();
    let error = block_on(pump.update_selection(selection::Update::Set {
        start: start.clone(),
        end: foreign,
        rectangle: false,
    }))
    .err()
    .expect("foreign anchor rejects selection");
    assert!(error.is_selection_rejection());
    assert_eq!(transport.lock().closes, 0);
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));
    assert_eq!(
        block_on(model.selection_text()).unwrap().as_deref(),
        Some("a")
    );
    block_on(model.release_anchor(&start)).unwrap();
    let error = block_on(pump.update_selection(selection::Update::Set {
        start: start.clone(),
        end: start,
        rectangle: false,
    }))
    .err()
    .expect("released anchor rejects selection");
    assert!(error.is_selection_rejection());
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));
    block_on(pump.update_selection(selection::Update::Clear)).unwrap();
    block_on(pump.shutdown()).unwrap();
    block_on(other.shutdown()).unwrap();
}

#[test]
fn selection_inactive_and_expired_rejections_keep_current_publication_usable() {
    use crate::ghostty::anchors::{Coordinate, Point, Space};
    let (mut pump, transport, _, _) = pump(16);
    let model = pump.model();
    let point = Point {
        space: Space::Active,
        coordinate: Coordinate { column: 0, row: 0 },
    };
    block_on(pump.process_chunk(b"primary")).unwrap();
    let primary = block_on(model.create_anchor(point)).unwrap();
    block_on(pump.process_chunk(b"\x1b[?1049h\x1b[Halt")).unwrap();
    let alt = block_on(model.create_anchor(point)).unwrap();
    let good = block_on(pump.update_selection(selection::Update::Set {
        start: alt.clone(),
        end: alt.clone(),
        rectangle: false,
    }))
    .unwrap();
    let error = block_on(pump.update_selection(selection::Update::Set {
        start: primary.clone(),
        end: alt,
        rectangle: false,
    }))
    .err()
    .expect("inactive endpoint rejects");
    assert!(error.is_selection_rejection());
    assert!(Arc::ptr_eq(
        &good,
        model.reader().poll().unwrap().frame.as_ref().unwrap()
    ));
    assert_eq!(
        block_on(model.selection_text()).unwrap().as_deref(),
        Some("a")
    );
    block_on(pump.process_chunk(b"\x1b[?1049l\x1bcnew")).unwrap();
    let fresh = block_on(model.create_anchor(point)).unwrap();
    let good = block_on(pump.update_selection(selection::Update::Set {
        start: fresh.clone(),
        end: fresh,
        rectangle: false,
    }))
    .unwrap();
    let error = block_on(pump.update_selection(selection::Update::Set {
        start: primary.clone(),
        end: primary,
        rectangle: false,
    }))
    .err()
    .expect("expired endpoint rejects");
    assert!(error.is_selection_rejection());
    assert!(Arc::ptr_eq(
        &good,
        model.reader().poll().unwrap().frame.as_ref().unwrap()
    ));
    assert_eq!(
        block_on(model.selection_text()).unwrap().as_deref(),
        Some("n")
    );
    assert_eq!(transport.lock().closes, 0);
    block_on(pump.update_selection(selection::Update::Clear)).unwrap();
    block_on(pump.shutdown()).unwrap();
}

#[test]
fn selection_delivers_prior_reliable_event_before_publication() {
    let (mut pump, _, tx, rx) = pump(2);
    let good = block_on(pump.finish_batch()).unwrap();
    rx.try_recv().unwrap();
    pump.router.state().release_repaint_hint();
    tx.try_send(SessionEvent::Bell).unwrap();
    tx.try_send(SessionEvent::Bell).unwrap();
    block_on(pump.process_chunk(b"\x1b]2;before-selection\x07")).unwrap();
    let model = pump.model();
    let mut update = Box::pin(pump.update_selection(selection::Update::Clear));
    assert!(
        update
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    assert!(Arc::ptr_eq(
        &good,
        model.reader().poll().unwrap().frame.as_ref().unwrap()
    ));
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Bell);
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Bell);
    block_on(update).unwrap();
    assert_eq!(
        rx.try_recv().unwrap(),
        SessionEvent::Title("before-selection".into())
    );
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Output);
    block_on(pump.shutdown()).unwrap();
}

#[test]
fn selection_waits_for_reliable_events_and_cancels_with_last_good() {
    let (mut pump, transport, tx, rx) = pump(1);
    let good = block_on(pump.finish_batch()).unwrap();
    rx.try_recv().unwrap();
    tx.try_send(SessionEvent::Bell).unwrap();
    block_on(pump.process_chunk(b"\x1b]2;before-selection\x07")).unwrap();
    let model = pump.model();
    let mut update = Box::pin(pump.update_selection(selection::Update::Clear));
    assert!(
        update
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));
    drop(update);
    assert_eq!(transport.lock().closes, 1);
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));
    assert!(block_on(pump.shutdown()).is_err());
}

#[test]
fn scroll_viewport_publishes_history_damage_and_repaint_without_new_output() {
    let (mut pump, transport, _, rx) = pump(16);
    block_on(pump.process_chunk(b"zero\r\none\r\ntwo\r\nthree\r\nfour")).unwrap();
    let bottom = block_on(pump.finish_batch()).unwrap();
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Output);
    let model = pump.model();
    let mut first = model.reader();
    let mut second = model.reader();
    first.poll().unwrap();
    second.poll().unwrap();
    pump.router.state().release_repaint_hint();
    let top = block_on(pump.scroll_viewport(state::Scroll::Top)).unwrap();
    assert_eq!(top.source().metadata.scrollbar.offset, 0);
    assert_eq!(top.source().rows[0].cells[0].text, "z");
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Output);
    assert!(!matches!(first.poll().unwrap().damage, Damage::None));
    assert!(!matches!(second.poll().unwrap().damage, Damage::None));
    assert_eq!(first.poll().unwrap().damage, Damage::None);
    let next = block_on(pump.scroll_viewport(state::Scroll::Delta(1))).unwrap();
    assert_eq!(next.source().metadata.scrollbar.offset, 1);
    assert!(rx.is_empty(), "already-held repaint hint coalesces");
    let previous = block_on(pump.scroll_viewport(state::Scroll::Delta(-1))).unwrap();
    assert_eq!(previous.source().metadata.scrollbar.offset, 0);
    let row = block_on(pump.scroll_viewport(state::Scroll::Row(1))).unwrap();
    assert_eq!(row.source().metadata.scrollbar.offset, 1);
    pump.router.state().release_repaint_hint();
    let restored = block_on(pump.scroll_viewport(state::Scroll::Bottom)).unwrap();
    assert_eq!(restored.source(), bottom.source());
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Output);
    assert!(transport.lock().writes.is_empty());
    block_on(pump.shutdown()).unwrap();
}

fn wheel(steps: i32) -> crate::ghostty::wheel::WheelEvent {
    crate::ghostty::wheel::WheelEvent {
        steps,
        pointer: mouse(),
    }
}

#[test]
fn wheel_publishes_history_but_keeps_byte_routes_damage_free() {
    let (mut pump, transport, _, rx) = pump(16);
    block_on(pump.process_chunk(b"zero\r\none\r\ntwo\r\nthree\r\nfour")).unwrap();
    let bottom = block_on(pump.finish_batch()).unwrap();
    rx.try_recv().unwrap();
    pump.router.state().release_repaint_hint();
    let model = pump.model();
    block_on(pump.wheel(wheel(10))).unwrap();
    assert_eq!(
        model
            .reader()
            .poll()
            .unwrap()
            .frame
            .unwrap()
            .source()
            .metadata
            .scrollbar
            .offset,
        0
    );
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Output);
    block_on(pump.wheel(wheel(-10))).unwrap();
    let restored = model.reader().poll().unwrap().frame.unwrap();
    assert_eq!(restored.source(), bottom.source());
    assert!(rx.is_empty(), "repaint coalesces");
    block_on(pump.process_chunk(b"\x1b[?1000;1006h")).unwrap();
    block_on(pump.wheel(wheel(2))).unwrap();
    assert_eq!(transport.lock().writes.len(), 1);
    assert!(transport.lock().writes[0].starts_with(b"\x1b[<64;"));
    assert!(Arc::ptr_eq(
        &model.reader().poll().unwrap().frame.unwrap(),
        &restored
    ));
    assert!(rx.is_empty(), "byte-only route does not publish");
    transport.lock().write_error = Some(TerminalError::QueueFull);
    assert!(matches!(
        block_on(pump.wheel(wheel(-1))),
        Err(Error::Transport(TerminalError::QueueFull))
    ));
    assert_eq!(transport.lock().closes, 1);
    assert!(matches!(block_on(pump.wheel(wheel(1))), Err(Error::Closed)));
    assert_eq!(
        transport.lock().writes.len(),
        2,
        "failed admission is never replayed"
    );
    assert!(block_on(pump.shutdown()).is_err());
}

#[test]
fn wheel_waits_for_reliable_events_and_cancellation_fails_closed() {
    for cancel in [false, true] {
        let (mut pump, transport, tx, rx) = pump(2);
        block_on(pump.process_chunk(b"zero\r\none\r\ntwo\r\nthree\r\nfour")).unwrap();
        let good = block_on(pump.finish_batch()).unwrap();
        rx.try_recv().unwrap();
        pump.router.state().release_repaint_hint();
        tx.try_send(SessionEvent::Bell).unwrap();
        tx.try_send(SessionEvent::Bell).unwrap();
        block_on(pump.process_chunk(b"\x1b]2;before-wheel\x07")).unwrap();
        let model = pump.model();
        let mut pending = Box::pin(pump.wheel(wheel(10)));
        assert!(
            pending
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        assert!(Arc::ptr_eq(
            &model.reader().poll().unwrap().frame.unwrap(),
            &good
        ));
        if cancel {
            drop(pending);
            assert_eq!(transport.lock().closes, 1);
            assert!(matches!(block_on(pump.wheel(wheel(1))), Err(Error::Closed)));
            assert!(block_on(pump.shutdown()).is_err());
        } else {
            rx.try_recv().unwrap();
            rx.try_recv().unwrap();
            block_on(pending).unwrap();
            assert_eq!(
                rx.try_recv().unwrap(),
                SessionEvent::Title("before-wheel".into())
            );
            assert_eq!(rx.try_recv().unwrap(), SessionEvent::Output);
            let frame = model.reader().poll().unwrap().frame.unwrap();
            assert_eq!(frame.source().metadata.scrollbar.offset, 0);
            block_on(pump.shutdown()).unwrap();
        }
        assert!(transport.lock().writes.is_empty());
    }
}

#[test]
fn scroll_viewport_waits_for_prior_reliable_events_before_mutation_and_capture() {
    let (mut pump, transport, tx, rx) = pump(2);
    block_on(pump.process_chunk(b"zero\r\none\r\ntwo\r\nthree\r\nfour")).unwrap();
    let good = block_on(pump.finish_batch()).unwrap();
    rx.try_recv().unwrap();
    pump.router.state().release_repaint_hint();
    tx.try_send(SessionEvent::Bell).unwrap();
    tx.try_send(SessionEvent::Bell).unwrap();
    block_on(pump.process_chunk(b"\x1b]2;before-scroll\x07")).unwrap();
    let model = pump.model();
    let mut scroll = Box::pin(pump.scroll_viewport(state::Scroll::Top));
    assert!(
        scroll
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Bell);
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Bell);
    let top = block_on(scroll).unwrap();
    assert_eq!(top.source().metadata.scrollbar.offset, 0);
    assert_eq!(
        rx.try_recv().unwrap(),
        SessionEvent::Title("before-scroll".into())
    );
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Output);
    assert!(transport.lock().writes.is_empty());
    block_on(pump.shutdown()).unwrap();
}

#[test]
fn cancelled_scroll_publication_retains_last_good_and_fails_closed() {
    let (mut pump, transport, tx, rx) = pump(1);
    let good = block_on(pump.finish_batch()).unwrap();
    rx.try_recv().unwrap();
    tx.try_send(SessionEvent::Bell).unwrap();
    block_on(pump.process_chunk(b"\x1b]2;pending\x07")).unwrap();
    let model = pump.model();
    let mut scroll = Box::pin(pump.scroll_viewport(state::Scroll::Top));
    assert!(
        scroll
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(scroll);
    assert_eq!(transport.lock().closes, 1);
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));
    assert!(matches!(
        block_on(pump.scroll_viewport(state::Scroll::Bottom)),
        Err(Error::Closed)
    ));
    assert!(block_on(pump.shutdown()).is_err());
}

#[test]
fn paste_uses_unpublished_modes_and_existing_filter_without_capture() {
    let (mut pump, transport, _, rx) = pump(8);
    let good = block_on(pump.finish_batch()).unwrap();
    while rx.try_recv().is_ok() {}
    let unpolled = pump.paste("not sent");
    drop(unpolled);
    block_on(pump.paste("")).unwrap();
    assert!(transport.lock().writes.is_empty());
    block_on(pump.paste("a\r\nb\nc\x1b\x03")).unwrap();
    block_on(pump.process_chunk(b"\x1b[?2004h")).unwrap();
    assert!(!good.source().metadata.modes.bracketed_paste);
    block_on(pump.paste("a\x1b[201~b\x1b[200~c\x1b[31m\x03\n")).unwrap();
    block_on(pump.paste("")).unwrap();
    block_on(pump.process_chunk(b"\x1b[?2004l")).unwrap();
    block_on(pump.paste("plain\n")).unwrap();
    assert_eq!(
        transport.lock().writes,
        vec![
            b"a\rb\rc\x1b\x03".to_vec(),
            b"\x1b[200~abc[31m\n\x1b[201~".to_vec(),
            b"\x1b[200~\x1b[201~".to_vec(),
            b"plain\r".to_vec(),
        ]
    );
    assert!(rx.is_empty());
    assert!(Arc::ptr_eq(
        pump.model()
            .reader()
            .poll()
            .unwrap()
            .frame
            .as_ref()
            .unwrap(),
        &good
    ));
    block_on(pump.shutdown()).unwrap();
}

#[test]
fn drag_selection_tracks_start_rejects_stale_samples_and_releases_all_gesture_anchors() {
    use crate::ghostty::{
        anchors::{Coordinate, Point, Space},
        selection::{Gesture, GestureId, Update},
    };
    let (mut pump, _, _, _) = pump(16);
    block_on(pump.process_chunk(b"zero")).unwrap();
    block_on(pump.finish_batch()).unwrap();
    let model = pump.model();
    let id = GestureId::default();
    let old = model.query().viewport.unwrap();
    block_on(pump.update_selection(Update::Gesture(Gesture::Begin {
        id: id.clone(),
        viewport: old.clone(),
        coordinate: Coordinate { column: 0, row: 0 },
    })))
    .unwrap();
    let current = model.query().viewport.unwrap();
    let foreign = crate::ghostty_bridge::AsyncEngine::spawn(16, 3).unwrap();
    block_on(foreign.request(Command::Capture).unwrap().wait()).unwrap();
    assert!(
        matches!(block_on(pump.update_selection(Update::Gesture(Gesture::Move {
        id: id.clone(), viewport: foreign.model().query().viewport.unwrap(), coordinate: Coordinate { column: 3, row: 0 },
    }))), Err(error) if error.is_selection_rejection())
    );
    block_on(pump.update_selection(Update::Gesture(Gesture::Finish(GestureId::default()))))
        .unwrap();
    block_on(pump.update_selection(Update::Gesture(Gesture::Move {
        id: id.clone(),
        viewport: old,
        coordinate: Coordinate { column: 3, row: 0 },
    })))
    .unwrap(); // Selection-only captures do not expire painted coordinates.
    block_on(pump.process_chunk(b"\r\none\r\ntwo\r\nthree\r\nfour\r\nfive\r\nsix")).unwrap();
    assert!(
        matches!(block_on(pump.update_selection(Update::Gesture(Gesture::Move {
        id: id.clone(), viewport: current, coordinate: Coordinate { column: 3, row: 0 },
    }))), Err(error) if error.is_selection_rejection()),
        "unpublished mutation invalidates coordinates"
    );
    block_on(pump.finish_batch()).unwrap();
    block_on(pump.update_selection(Update::Gesture(Gesture::Move {
        id: id.clone(),
        viewport: model.query().viewport.unwrap(),
        coordinate: Coordinate { column: 2, row: 2 },
    })))
    .unwrap();
    let text = block_on(model.selection_text()).unwrap().unwrap();
    assert!(
        text.starts_with("zero\none\ntwo\nthree\nfour\nfive\nsix"),
        "{text:?}"
    );
    assert!(!model.query().viewport.unwrap().selection_spans().is_empty());
    block_on(pump.update_selection(Update::Gesture(Gesture::Finish(id)))).unwrap();
    assert_eq!(
        block_on(model.selection_text()).unwrap().as_deref(),
        Some(text.as_str())
    );
    for _ in 0..300 {
        let id = GestureId::default();
        block_on(pump.update_selection(Update::Gesture(Gesture::Begin {
            id: id.clone(),
            viewport: model.query().viewport.unwrap(),
            coordinate: Coordinate { column: 0, row: 0 },
        })))
        .unwrap();
        block_on(pump.update_selection(Update::Gesture(Gesture::Move {
            id: id.clone(),
            viewport: model.query().viewport.unwrap(),
            coordinate: Coordinate { column: 1, row: 0 },
        })))
        .unwrap();
        block_on(pump.update_selection(Update::Gesture(Gesture::Finish(id)))).unwrap();
    }
    let id = GestureId::default();
    block_on(pump.update_selection(Update::Gesture(Gesture::Begin {
        id: id.clone(),
        viewport: model.query().viewport.unwrap(),
        coordinate: Coordinate { column: 0, row: 0 },
    })))
    .unwrap();
    block_on(pump.process_chunk(b"\x1b[?1049h")).unwrap();
    block_on(pump.finish_batch()).unwrap();
    assert!(
        matches!(block_on(pump.update_selection(Update::Gesture(Gesture::Move {
        id: id.clone(), viewport: model.query().viewport.unwrap(), coordinate: Coordinate { column: 1, row: 0 },
    }))), Err(error) if error.is_selection_rejection())
    );
    block_on(pump.update_selection(Update::Gesture(Gesture::Finish(id)))).unwrap();
    assert!(
        matches!(block_on(pump.update_selection(Update::Gesture(Gesture::Begin {
        id: GestureId::default(), viewport: model.query().viewport.unwrap(), coordinate: Coordinate { column: u16::MAX, row: u32::MAX },
    }))), Err(error) if error.is_selection_rejection())
    );
    let mut anchors = Vec::new();
    for _ in 0..crate::ghostty::anchors::MAX_ANCHORS {
        anchors.push(
            block_on(model.create_anchor(Point {
                space: Space::Viewport,
                coordinate: Coordinate { column: 0, row: 0 },
            }))
            .unwrap(),
        );
    }
    for anchor in anchors {
        block_on(model.release_anchor(&anchor)).unwrap();
    }
    block_on(pump.shutdown()).unwrap();
}

#[test]
fn visible_paste_rejects_without_reveal_then_filters_live_mode_and_reveals_before_write() {
    let (mut pump, transport, _, rx) = pump(16);
    block_on(pump.process_chunk(b"zero\r\none\r\ntwo\r\nthree\r\nfour")).unwrap();
    let bottom = block_on(pump.finish_batch()).unwrap();
    let top = block_on(pump.scroll_viewport(state::Scroll::Top)).unwrap();
    while rx.try_recv().is_ok() {}
    pump.router.state().release_repaint_hint();
    block_on(pump.paste_visible("")).unwrap();
    let too_large = "x".repeat(crate::paste::DEFAULT_MAX_PASTE_BYTES + 1);
    assert!(matches!(
        block_on(pump.paste_visible(&too_large)),
        Err(Error::Paste(_))
    ));
    assert!(Arc::ptr_eq(
        &top,
        &pump.model().reader().poll().unwrap().frame.unwrap()
    ));
    assert!(transport.lock().writes.is_empty());
    assert!(rx.is_empty());
    block_on(pump.process_chunk(b"\x1b[?2004h\x1b]2;before-paste\x07")).unwrap();
    assert!(!top.source().metadata.modes.bracketed_paste);
    block_on(pump.paste_visible("a\x1b[201~b\x1b\x03\n")).unwrap();
    assert_eq!(
        rx.try_recv().unwrap(),
        SessionEvent::Title("before-paste".into())
    );
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Output);
    let observed = transport.lock();
    assert_eq!(observed.writes, [b"\x1b[200~ab\n\x1b[201~".to_vec()]);
    let frame = observed.frames_at_write[0].as_ref().unwrap();
    assert_eq!(
        frame.source().metadata.scrollbar,
        bottom.source().metadata.scrollbar
    );
    assert!(frame.source().metadata.modes.bracketed_paste);
    drop(observed);
    transport.lock().write_error = Some(TerminalError::QueueFull);
    assert!(matches!(
        block_on(pump.paste_visible("failed")),
        Err(Error::Transport(TerminalError::QueueFull))
    ));
    assert!(matches!(
        block_on(pump.paste_visible("never")),
        Err(Error::Closed)
    ));
    assert_eq!(transport.lock().closes, 1);
    assert_eq!(transport.lock().writes.len(), 2);
}

#[test]
fn paste_size_rejection_recovers_but_transport_rejection_fails_stop() {
    let (mut pump, transport, _, _) = pump(8);
    let good = block_on(pump.finish_batch()).unwrap();
    let too_large = "x".repeat(crate::paste::DEFAULT_MAX_PASTE_BYTES + 1);
    assert!(matches!(
        block_on(pump.paste(&too_large)),
        Err(Error::Paste(PasteError::TooLarge { .. }))
    ));
    assert_eq!(transport.lock().closes, 0);
    block_on(pump.paste("accepted")).unwrap();
    transport.lock().write_error = Some(TerminalError::QueueFull);
    assert!(matches!(
        block_on(pump.paste("rejected")),
        Err(Error::Transport(TerminalError::QueueFull))
    ));
    assert!(matches!(block_on(pump.paste("never")), Err(Error::Closed)));
    assert_eq!(transport.lock().closes, 1);
    assert_eq!(transport.lock().writes.len(), 2);
    assert!(Arc::ptr_eq(
        pump.model()
            .reader()
            .poll()
            .unwrap()
            .frame
            .as_ref()
            .unwrap(),
        &good
    ));
    assert!(block_on(pump.shutdown()).is_err());
}

#[test]
fn visible_paste_waits_for_reliable_events_and_cancellation_never_writes_or_reveals() {
    for cancel in [false, true] {
        let (mut pump, transport, tx, rx) = pump(2);
        block_on(pump.process_chunk(b"zero\r\none\r\ntwo\r\nthree\r\nfour")).unwrap();
        block_on(pump.finish_batch()).unwrap();
        let good = block_on(pump.scroll_viewport(state::Scroll::Top)).unwrap();
        while rx.try_recv().is_ok() {}
        pump.router.state().release_repaint_hint();
        tx.try_send(SessionEvent::Bell).unwrap();
        tx.try_send(SessionEvent::Bell).unwrap();
        block_on(pump.process_chunk(b"\x1b]2;before-visible-paste\x07")).unwrap();
        let model = pump.model();
        let mut paste = Box::pin(pump.paste_visible("accepted"));
        for _ in 0..20 {
            assert!(
                paste
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(Arc::ptr_eq(
            &good,
            &model.reader().poll().unwrap().frame.unwrap()
        ));
        assert!(transport.lock().writes.is_empty());
        if cancel {
            drop(paste);
            assert_eq!(transport.lock().closes, 1);
            assert!(matches!(
                block_on(pump.paste_visible("never")),
                Err(Error::Closed)
            ));
            assert!(Arc::ptr_eq(
                &good,
                &model.reader().poll().unwrap().frame.unwrap()
            ));
            assert!(transport.lock().writes.is_empty());
        } else {
            rx.try_recv().unwrap();
            rx.try_recv().unwrap();
            block_on(paste).unwrap();
            assert_eq!(
                rx.try_recv().unwrap(),
                SessionEvent::Title("before-visible-paste".into())
            );
            assert_eq!(rx.try_recv().unwrap(), SessionEvent::Output);
            assert_eq!(transport.lock().writes, [b"accepted".to_vec()]);
            assert_ne!(
                model
                    .reader()
                    .poll()
                    .unwrap()
                    .frame
                    .unwrap()
                    .source()
                    .metadata
                    .scrollbar
                    .offset,
                0
            );
            block_on(pump.shutdown()).unwrap();
        }
    }
}

#[test]
fn cancelled_paste_mode_lookup_closes_without_write_or_capture() {
    let (mut pump, transport, _, _) = pump(8);
    let good = block_on(pump.finish_batch()).unwrap();
    let (entered, receiver) = std::sync::mpsc::sync_channel(1);
    let (release, wait) = std::sync::mpsc::sync_channel(1);
    let barrier = pump.engine.request(Command::Wait(entered, wait)).unwrap();
    receiver.recv().unwrap();
    let mut paste = Box::pin(pump.paste("never"));
    assert!(
        paste
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(paste);
    assert_eq!(transport.lock().closes, 1);
    release.send(()).unwrap();
    block_on(barrier.wait()).unwrap();
    assert!(block_on(pump.shutdown()).is_err());
    assert!(transport.lock().writes.is_empty());
    assert!(Arc::ptr_eq(
        pump.model()
            .reader()
            .poll()
            .unwrap()
            .frame
            .as_ref()
            .unwrap(),
        &good
    ));
}

#[test]
fn replies_precede_policy_events_prepared_capture_and_output() {
    let (mut pump, transport, _, rx) = pump(16);
    let model = pump.model();
    let mut first = model.reader();
    let mut second = model.reader();
    block_on(pump.process_chunk(b"abc\x1b[6n\x1b]2;title\x07\x1b]7;file:///D:/work/a%20b\x07\x07"))
        .unwrap();
    assert!(first.poll().unwrap().frame.is_none());
    assert!(rx.is_empty());
    assert_eq!(transport.lock().writes, [b"\x1b[1;4R".to_vec()]);
    assert!(transport.lock().frames_at_write[0].is_none());
    let frame = block_on(pump.finish_batch()).unwrap();
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Title("title".into()));
    assert_eq!(
        rx.try_recv().unwrap(),
        SessionEvent::Cwd("D:/work/a b".into())
    );
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Bell);
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Output);
    let update = first.poll().unwrap();
    assert!(Arc::ptr_eq(update.frame.as_ref().unwrap(), &frame));
    assert_eq!(update.damage, Damage::Full);
    assert_eq!(second.poll().unwrap().damage, Damage::Full);
    assert_eq!(first.poll().unwrap().damage, Damage::None);
    block_on(pump.close()).unwrap();
    assert_eq!(transport.lock().closes, 1);
}

#[test]
fn large_reads_drain_replies_between_chunks_without_empty_writes() {
    let (mut pump, transport, _, _) = pump(8);
    let mut bytes = vec![b'\r'; owner::MAX_FEED_BYTES - 4];
    bytes.extend_from_slice(b"\x1b[6n");
    bytes.extend_from_slice(b"\x1b[6n");
    block_on(pump.process_chunk(&bytes)).unwrap();
    block_on(pump.process_chunk(b"")).unwrap();
    block_on(pump.process_chunk(b"no reply")).unwrap();
    assert_eq!(
        transport.lock().writes,
        [b"\x1b[1;1R".to_vec(), b"\x1b[1;1R".to_vec()]
    );
    block_on(pump.shutdown()).unwrap();
}

#[test]
fn reliable_backpressure_is_pending_and_prevents_early_publication() {
    let (mut pump, _, tx, rx) = pump(1);
    let model = pump.model();
    tx.try_send(SessionEvent::Bell).unwrap();
    block_on(pump.process_chunk(b"\x1b]2;one\x07\x1b]2;two\x07")).unwrap();
    let mut future = Box::pin(pump.finish_batch());
    let mut context = Context::from_waker(Waker::noop());
    assert!(future.as_mut().poll(&mut context).is_pending());
    assert!(model.reader().poll().unwrap().frame.is_none());
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Bell);
    assert!(future.as_mut().poll(&mut context).is_pending());
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Title("one".into()));
    assert!(future.as_mut().poll(&mut context).is_pending());
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Title("two".into()));
    block_on(future).unwrap();
    block_on(pump.shutdown()).unwrap();
}

#[test]
fn cancelling_backpressured_batch_closes_immediately_and_retains_last_good() {
    let (mut pump, transport, tx, rx) = pump(1);
    let model = pump.model();
    let good = block_on(pump.finish_batch()).unwrap();
    rx.try_recv().unwrap();
    tx.try_send(SessionEvent::Bell).unwrap();
    block_on(pump.process_chunk(b"changed\x1b]2;blocked\x07")).unwrap();
    let mut future = Box::pin(pump.finish_batch());
    assert!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(future);
    assert_eq!(transport.lock().closes, 1);
    let failed = model.reader().poll().unwrap();
    assert!(matches!(
        failed.state,
        State::Failed(BridgeError::AbandonedDelivery)
    ));
    assert!(Arc::ptr_eq(failed.frame.as_ref().unwrap(), &good));
    assert!(matches!(
        block_on(pump.process_chunk(b"never")),
        Err(Error::Closed)
    ));
    assert!(block_on(pump.shutdown()).is_err());
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Bell);
    assert!(rx.is_empty());
    assert_eq!(transport.lock().closes, 1);
}

#[test]
fn reliable_channel_closure_stops_without_a_new_frame() {
    let (mut pump, transport, _, rx) = pump(1);
    let model = pump.model();
    block_on(pump.process_chunk(b"\x1b]2;lost\x07")).unwrap();
    rx.close();
    assert!(matches!(
        block_on(pump.finish_batch()),
        Err(Error::EventsClosed)
    ));
    assert_eq!(pump.router.events().diagnostics().event_closed, 1);
    assert_eq!(transport.lock().closes, 1);
    assert!(model.reader().poll().unwrap().frame.is_none());
    assert!(block_on(pump.shutdown()).is_err());
}

#[test]
fn transport_rejection_is_not_retried_and_cleanup_does_not_mask_cause() {
    let (mut pump, transport, _, _) = pump(8);
    let good = block_on(pump.finish_batch()).unwrap();
    let model = pump.model();
    transport.lock().write_error = Some(TerminalError::QueueFull);
    transport.lock().close_error = Some(TerminalError::Closed);
    assert!(matches!(
        block_on(pump.process_chunk(b"changed\x1b[6n")),
        Err(Error::Transport(TerminalError::QueueFull))
    ));
    assert_eq!(transport.lock().writes.len(), 1);
    assert_eq!(transport.lock().closes, 1);
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));
    assert!(block_on(pump.shutdown()).is_err());
    assert_eq!(transport.lock().writes.len(), 1);
}

#[test]
fn shutdown_signals_before_poll_and_discards_unflushed_events() {
    let (mut pump, transport, _, rx) = pump(8);
    let model = pump.model();
    block_on(pump.process_chunk(b"\x1b]2;not published\x07")).unwrap();
    let shutdown = pump.shutdown();
    assert_eq!(transport.lock().closes, 1);
    assert!(matches!(
        block_on(model.clear_selection()),
        Err(BridgeError::Closed)
    ));
    block_on(shutdown).unwrap();
    assert!(matches!(
        model.reader().poll().unwrap().state,
        State::Closed
    ));
    assert!(model.reader().poll().unwrap().frame.is_none());
    assert!(rx.is_empty());
    block_on(pump.close()).unwrap();
    assert_eq!(transport.lock().closes, 1);
}

#[test]
fn graceful_close_flushes_without_capture_or_repaint() {
    let (mut pump, transport, _, rx) = pump(8);
    let model = pump.model();
    block_on(pump.process_chunk(b"\x1b]2;closing\x07\x07")).unwrap();
    block_on(pump.close()).unwrap();
    assert_eq!(
        rx.try_recv().unwrap(),
        SessionEvent::Title("closing".into())
    );
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Bell);
    assert!(rx.is_empty());
    assert!(model.reader().poll().unwrap().frame.is_none());
    assert_eq!(transport.lock().closes, 1);
}

#[test]
fn exit_flushes_prior_events_without_capture_and_allows_late_output() {
    let (mut pump, _, _, rx) = pump(8);
    let state = pump.router.state().clone();
    block_on(pump.process_chunk(b"before\x1b]2;exiting\x07\x07")).unwrap();
    block_on(pump.publish_exit(Some(7))).unwrap();
    assert_eq!(
        rx.try_recv().unwrap(),
        SessionEvent::Title("exiting".into())
    );
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Bell);
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Exited(Some(7)));
    assert!(rx.is_empty());
    assert!(!state.alive());
    assert_eq!(state.exit_code(), Some(7));
    assert!(pump.model().reader().poll().unwrap().frame.is_none());
    block_on(pump.process_chunk(b" after")).unwrap();
    let frame = block_on(pump.finish_batch()).unwrap();
    let text: String = frame.source().rows[0]
        .cells
        .iter()
        .map(|cell| cell.text.as_str())
        .collect();
    assert!(text.starts_with("before after"), "{text:?}");
    block_on(pump.shutdown()).unwrap();
}

#[test]
fn cancelled_exit_delivery_keeps_cache_last_good_and_closes_admission() {
    let (mut pump, transport, tx, rx) = pump(1);
    let good = block_on(pump.finish_batch()).unwrap();
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Output);
    let state = pump.router.state().clone();
    let model = pump.model();
    tx.try_send(SessionEvent::Bell).unwrap();
    let mut exit = Box::pin(pump.publish_exit(Some(9)));
    assert!(
        exit.as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    assert!(!state.alive());
    assert_eq!(state.exit_code(), Some(9));
    drop(exit);
    assert_eq!(transport.lock().closes, 1);
    assert!(matches!(
        model.reader().poll().unwrap().state,
        State::Failed(BridgeError::AbandonedDelivery)
    ));
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));
    assert_eq!(rx.try_recv().unwrap(), SessionEvent::Bell);
    assert!(rx.is_empty());
    assert!(block_on(pump.shutdown()).is_err());
}

#[test]
fn exit_event_loss_is_typed_and_stops_native_without_capture() {
    let (mut pump, transport, _, rx) = pump(1);
    rx.close();
    assert!(matches!(
        block_on(pump.publish_exit(None)),
        Err(Error::EventsClosed)
    ));
    assert!(!pump.router.state().alive());
    assert_eq!(pump.router.state().exit_code(), None);
    assert_eq!(pump.router.events().diagnostics().event_closed, 1);
    assert_eq!(transport.lock().closes, 1);
    assert!(pump.model().reader().poll().unwrap().frame.is_none());
    assert!(block_on(pump.shutdown()).is_err());
}

#[test]
fn invalid_resize_is_recoverable_but_transport_resize_failure_stops() {
    let (mut pump, transport, _, _) = pump(8);
    assert!(block_on(pump.resize(0, 3, 0, 0)).is_err());
    assert_eq!(transport.lock().closes, 0);
    block_on(pump.resize(20, 4, 10, 20)).unwrap();
    let good = block_on(pump.finish_batch()).unwrap();
    assert_eq!(good.source().cols, 20);
    assert_eq!(transport.lock().sizes, [(4, 20)]);
    transport.lock().resize_error = Some(TerminalError::Closed);
    assert!(matches!(
        block_on(pump.resize(21, 5, 10, 20)),
        Err(Error::Transport(TerminalError::Closed))
    ));
    assert_eq!(transport.lock().closes, 1);
    assert!(Arc::ptr_eq(
        pump.model()
            .reader()
            .poll()
            .unwrap()
            .frame
            .as_ref()
            .unwrap(),
        &good
    ));
    assert!(block_on(pump.shutdown()).is_err());
}

#[test]
fn batch_event_overflow_stops_without_partial_publication() {
    let (mut pump, transport, _, _) = pump(8);
    block_on(pump.process_chunk(&vec![7; MAX_EVENTS])).unwrap();
    assert!(block_on(pump.process_chunk(b"\x07")).is_err());
    assert!(pump.model().reader().poll().unwrap().frame.is_none());
    assert_eq!(transport.lock().closes, 1);
    assert!(block_on(pump.shutdown()).is_err());
}

#[test]
fn dropping_pump_closes_transport_even_with_live_models() {
    let (pump, transport, _, _) = pump(8);
    let model = pump.model();
    drop(pump);
    assert_eq!(transport.lock().closes, 1);
    assert!(matches!(
        model.reader().poll().unwrap().state,
        State::Failed(_)
    ));
    assert!(matches!(
        block_on(model.clear_selection()),
        Err(BridgeError::Closed)
    ));
}

#[test]
fn cancelling_queued_feed_stops_before_owner_can_consume_it() {
    let (mut pump, transport, _, _) = pump(8);
    let good = block_on(pump.finish_batch()).unwrap();
    let model = pump.model();
    let (entered, receiver) = std::sync::mpsc::sync_channel(1);
    let (release, wait) = std::sync::mpsc::sync_channel(1);
    let barrier = pump.engine.request(Command::Wait(entered, wait)).unwrap();
    receiver.recv().unwrap();
    let mut feed = Box::pin(pump.process_chunk(b"never consumed\x1b[6n"));
    assert!(
        feed.as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(feed);
    assert_eq!(transport.lock().closes, 1);
    release.send(()).unwrap();
    block_on(barrier.wait()).unwrap();
    assert!(block_on(pump.shutdown()).is_err());
    assert!(transport.lock().writes.is_empty());
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));
}

#[test]
fn cancelled_queued_capture_never_replaces_last_good() {
    let (mut pump, transport, _, _) = pump(8);
    let good = block_on(pump.finish_batch()).unwrap();
    block_on(pump.process_chunk(b"changed")).unwrap();
    let model = pump.model();
    let (entered, receiver) = std::sync::mpsc::sync_channel(1);
    let (release, wait) = std::sync::mpsc::sync_channel(1);
    let barrier = pump.engine.request(Command::Wait(entered, wait)).unwrap();
    receiver.recv().unwrap();
    let mut capture = Box::pin(pump.finish_batch());
    assert!(
        capture
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(capture);
    release.send(()).unwrap();
    block_on(barrier.wait()).unwrap();
    assert!(block_on(pump.shutdown()).is_err());
    assert_eq!(transport.lock().closes, 1);
    assert!(Arc::ptr_eq(
        model.reader().poll().unwrap().frame.as_ref().unwrap(),
        &good
    ));
}

fn mouse() -> input::MouseEvent {
    input::MouseEvent {
        action: input::MouseAction::Press,
        button: Some(input::MouseButton::Left),
        modifiers: input::Modifiers::default(),
        x: 25.0,
        y: 21.0,
        any_button_pressed: true,
        geometry: input::Geometry {
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
fn live_input_modes_and_model_interactions_share_the_owner_without_publishing() {
    use crate::ghostty::anchors::{Coordinate, Point, Space};
    let (mut pump, transport, _, _) = pump(8);
    let model = pump.model();
    let mut reader = model.reader();
    let good = block_on(pump.finish_batch()).unwrap();
    reader.poll().unwrap();
    block_on(pump.process_chunk(b"abcdef\x1b[?1;1000;1006h")).unwrap();
    let mut invalid = input::KeyEvent::new(input::KeyCode::UNIDENTIFIED);
    invalid.text = "x".repeat(input::MAX_KEY_TEXT_BYTES + 1);
    assert!(block_on(pump.key(invalid)).is_err());
    let mut invalid = mouse();
    invalid.x = f32::NAN;
    assert!(block_on(pump.mouse(invalid)).is_err());
    assert_eq!(transport.lock().closes, 0);
    block_on(pump.key(input::KeyEvent::new(input::KeyCode::ARROW_UP))).unwrap();
    block_on(pump.mouse(mouse())).unwrap();
    assert_eq!(
        transport.lock().writes,
        [b"\x1bOA".to_vec(), b"\x1b[<0;3;2M".to_vec()]
    );
    let a = block_on(model.create_anchor(Point {
        space: Space::Active,
        coordinate: Coordinate { column: 1, row: 0 },
    }))
    .unwrap();
    let b = block_on(model.create_anchor(Point {
        space: Space::Active,
        coordinate: Coordinate { column: 3, row: 0 },
    }))
    .unwrap();
    block_on(model.set_selection(&a, &b, false)).unwrap();
    assert_eq!(
        block_on(model.selection_text()).unwrap().as_deref(),
        Some("bcd")
    );
    let found =
        block_on(model.search("bcd".into(), crate::ghostty::search::Options::default())).unwrap();
    assert_eq!(found.matches.len(), 1);
    assert_eq!(reader.poll().unwrap().damage, Damage::None);
    assert!(!good.source().metadata.modes.cursor_keys);
    let selected = block_on(pump.finish_batch()).unwrap();
    assert!(selected.source().metadata.modes.cursor_keys);
    assert_eq!(selected.selection_spans()[0].columns, 1..4);
    block_on(pump.scroll(state::Scroll::Top)).unwrap();
    block_on(pump.shutdown()).unwrap();
}

#[test]
fn native_failure_stays_causal_and_preserves_the_last_frame() {
    let (mut pump, transport, _, _) = pump(8);
    let good = block_on(pump.finish_batch()).unwrap();
    let model = pump.model();
    let result = block_on(pump.process_chunk(&b"\x1b[6n".repeat(1000)));
    assert!(matches!(result, Err(Error::Engine(BridgeError::Owner(_)))));
    assert!(matches!(
        block_on(pump.shutdown()),
        Err(Error::Engine(BridgeError::Owner(_)))
    ));
    let failed = model.reader().poll().unwrap();
    assert!(matches!(failed.state, State::Failed(BridgeError::Owner(_))));
    assert!(Arc::ptr_eq(failed.frame.as_ref().unwrap(), &good));
    assert_eq!(transport.lock().closes, 1);
}

#[test]
fn unpolled_operation_has_no_effect_and_close_cancellation_is_fail_stop() {
    let (mut pump, transport, tx, _rx) = pump(1);
    drop(pump.process_chunk(b"never"));
    assert_eq!(transport.lock().closes, 0);
    tx.try_send(SessionEvent::Bell).unwrap();
    block_on(pump.process_chunk(b"\x1b]2;blocked close\x07")).unwrap();
    let mut closing = Box::pin(pump.close());
    assert!(
        closing
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(closing);
    assert_eq!(transport.lock().closes, 1);
    assert!(block_on(pump.shutdown()).is_err());
}

#[test]
fn shutdown_future_waits_without_blocking_poll_and_reports_transport_failure() {
    let (mut pump, transport, _, _) = pump(8);
    let model = pump.model();
    let (entered, receiver) = std::sync::mpsc::sync_channel(1);
    let (release, wait) = std::sync::mpsc::sync_channel(1);
    let barrier = pump.engine.request(Command::Wait(entered, wait)).unwrap();
    receiver.recv().unwrap();
    transport.lock().close_error = Some(TerminalError::QueueFull);
    let mut shutdown = Box::pin(pump.shutdown());
    assert_eq!(transport.lock().closes, 1);
    assert!(
        shutdown
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    release.send(()).unwrap();
    block_on(barrier.wait()).unwrap();
    assert!(matches!(
        block_on(shutdown),
        Err(Error::Transport(TerminalError::QueueFull))
    ));
    assert!(matches!(
        model.reader().poll().unwrap().state,
        State::Closed
    ));
    assert_eq!(transport.lock().closes, 1);
}
