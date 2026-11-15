use std::sync::{Mutex, PoisonError};

use crate::ghostty::owner::{Damage, Failure, State};

use super::*;

#[test]
fn incremental_search_is_ordered_recoverable_and_never_publishes_or_writes() {
    use crate::ghostty::search::{Options, SearchError, Status};
    let (mut pump, transport, mut reader) = pump();
    pump.process_chunk(b"needle\r\nNEEDLE").unwrap();
    pump.finish_batch().unwrap();
    reader.poll().unwrap();
    let found = pump.search("needle".into(), Options::default()).unwrap();
    assert_eq!(found.matches.len(), 2);
    assert_eq!(pump.search_step().unwrap().status, Status::Complete);
    assert!(matches!(
        pump.search(
            "needle".into(),
            Options {
                case_sensitive: true,
                whole_word: false
            }
        ),
        Err(Error::Engine(OwnerError::Search(
            SearchError::UnsupportedOptions
        )))
    ));
    assert_eq!(pump.search_step().unwrap().matches.len(), 2);
    assert_eq!(reader.poll().unwrap().damage, Damage::None);
    assert!(transport.lock().writes.is_empty());
    assert_eq!(transport.lock().closes, 0);
    pump.close().unwrap();
}

#[test]
fn selection_rejections_are_recoverable_and_publish_only_at_batch_boundary() {
    use crate::ghostty::anchors::{Coordinate, Point, Space};
    let (mut pump, transport, mut reader) = pump();
    pump.process_chunk(b"abcdef").unwrap();
    pump.finish_batch().unwrap();
    reader.poll().unwrap();
    let a = pump
        .create_anchor(Point {
            space: Space::Active,
            coordinate: Coordinate { column: 1, row: 0 },
        })
        .unwrap();
    let b = pump
        .create_anchor(Point {
            space: Space::Active,
            coordinate: Coordinate { column: 3, row: 0 },
        })
        .unwrap();
    pump.set_selection(&a, &b, false).unwrap();
    assert_eq!(pump.selection_text().unwrap().as_deref(), Some("bcd"));
    assert_eq!(reader.poll().unwrap().damage, Damage::None);
    pump.release_anchor(&b).unwrap();
    assert!(matches!(
        pump.set_selection(&a, &b, false),
        Err(Error::Engine(OwnerError::Selection(_)))
    ));
    assert_eq!(pump.selection_text().unwrap().as_deref(), Some("bcd"));
    pump.finish_batch().unwrap();
    assert_eq!(reader.poll().unwrap().damage, Damage::Rows(vec![0]));
    pump.clear_selection().unwrap();
    assert_eq!(pump.selection_text().unwrap(), None);
    assert_eq!(reader.poll().unwrap().damage, Damage::None);
    pump.finish_batch().unwrap();
    assert_eq!(reader.poll().unwrap().damage, Damage::Rows(vec![0]));
    assert!(transport.lock().writes.is_empty());
    assert_eq!(transport.lock().closes, 0);
    pump.close().unwrap();
}

#[test]
fn viewport_command_publishes_with_the_next_batch_not_before_reliable_events() {
    use crate::ghostty::state::Scroll;
    let (mut pump, transport, mut reader) = pump();
    pump.process_chunk(b"zero\r\none\r\ntwo\r\nthree\r\nfour")
        .unwrap();
    let bottom = pump.finish_batch().unwrap();
    reader.poll().unwrap();
    pump.process_chunk(b"\x1b]2;scroll\x07").unwrap();
    pump.scroll(Scroll::Top).unwrap();
    assert_eq!(reader.poll().unwrap().damage, Damage::None);
    let top = pump.finish_batch().unwrap();
    assert_eq!(top.metadata.scrollbar.offset, 0);
    assert_ne!(
        top.metadata.scrollbar.offset,
        bottom.metadata.scrollbar.offset
    );
    assert_eq!(reader.poll().unwrap().damage, Damage::Full);
    let rx = transport.lock().events.as_ref().unwrap().clone();
    assert!(
        std::iter::from_fn(|| rx.try_recv().ok())
            .any(|event| event == crate::SessionEvent::Title("scroll".into()))
    );
    pump.close().unwrap();
}

#[derive(Default)]
struct Observations {
    events: Option<async_channel::Receiver<crate::session::SessionEvent>>,
    operations: Vec<&'static str>,
    writes: Vec<Vec<u8>>,
    sizes: Vec<(u16, u16)>,
    closes: usize,
    write_error: Option<TerminalError>,
    resize_error: Option<TerminalError>,
    close_error: Option<TerminalError>,
    reader: Option<FrameReader>,
    frames_at_write: Vec<Option<Arc<Frame>>>,
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
        state.operations.push("write");
        let frame = state.reader.as_mut().unwrap().poll().unwrap().frame;
        state.frames_at_write.push(frame);
        state.writes.push(bytes.to_vec());
        match &state.write_error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    fn pty_resize(&self, rows: u16, cols: u16) -> Result<(), TerminalError> {
        let mut state = self.lock();
        state.operations.push("resize");
        state.sizes.push((rows, cols));
        match &state.resize_error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    fn pty_close(&self) -> Result<(), TerminalError> {
        let mut state = self.lock();
        state.operations.push("close");
        state.closes += 1;
        match &state.close_error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

fn pump() -> (GhosttyPump<Transport>, Transport, FrameReader) {
    let transport = Transport::default();
    let (tx, rx) = async_channel::bounded(1024);
    let router = OscRouter::new(
        transport.clone(),
        super::super::SessionEventSink::new(tx),
        super::super::SharedSessionState::new_alive(),
        crate::ClipboardOrigin::Local,
    );
    transport.lock().events = Some(rx);
    let pump = GhosttyPump::new(router, 16, 3).unwrap();
    transport.lock().reader = Some(pump.reader().unwrap());
    let reader = pump.reader().unwrap();
    (pump, transport, reader)
}

fn input_mouse() -> crate::ghostty::input::MouseEvent {
    use crate::ghostty::input::*;
    MouseEvent {
        action: MouseAction::Press,
        button: Some(MouseButton::Left),
        modifiers: Modifiers::default(),
        x: 25.0,
        y: 21.0,
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
fn tracked_anchor_queries_are_ordered_without_publishing_or_touching_transport() {
    use crate::ghostty::anchors::{AnchorError, Coordinate, Point, Position, Space};
    let (mut pump, transport, mut reader) = pump();
    pump.process_chunk(b"abcdefghijk").unwrap();
    let before = pump.finish_batch().unwrap();
    reader.poll().unwrap();
    let id = pump
        .create_anchor(Point {
            space: Space::Active,
            coordinate: Coordinate { column: 8, row: 0 },
        })
        .unwrap();
    pump.resize(6, 3, 10, 20).unwrap();
    assert_eq!(
        pump.resolve_anchor(&id, Space::Screen).unwrap(),
        Some(Position {
            coordinate: Coordinate { column: 2, row: 1 },
            alternate_screen: false,
        })
    );
    pump.release_anchor(&id).unwrap();
    assert!(matches!(
        pump.resolve_anchor(&id, Space::Screen),
        Err(Error::Engine(OwnerError::Anchor(AnchorError::Released)))
    ));
    assert!(transport.lock().writes.is_empty());
    assert_eq!(transport.lock().closes, 0);
    let update = reader.poll().unwrap();
    assert_eq!(update.damage, Damage::None);
    assert!(Arc::ptr_eq(update.frame.as_ref().unwrap(), &before));
    let after = pump.finish_batch().unwrap();
    assert_eq!(after.cols, 6);
    pump.close().unwrap();
    assert_eq!(transport.lock().closes, 1);
}

#[test]
fn generated_input_reads_unpublished_modes_and_uses_only_its_transport() {
    use crate::ghostty::input::{KeyCode, KeyEvent};
    let (mut pump, transport, mut reader) = pump();
    let previous = pump.finish_batch().unwrap();
    reader.poll().unwrap();
    pump.process_chunk(b"\x1b[?1;1000;1006;2004h\x1b[6n")
        .unwrap();
    pump.key(KeyEvent::new(KeyCode::ARROW_UP)).unwrap();
    pump.mouse(input_mouse()).unwrap();
    pump.paste("safe\x1b[201~text\x03").unwrap();
    assert_eq!(
        transport.lock().writes,
        [
            b"\x1b[1;1R".to_vec(),
            b"\x1bOA".to_vec(),
            b"\x1b[<0;3;2M".to_vec(),
            b"\x1b[200~safetext\x1b[201~".to_vec(),
        ]
    );
    let update = reader.poll().unwrap();
    assert_eq!(update.damage, Damage::None);
    assert!(Arc::ptr_eq(update.frame.as_ref().unwrap(), &previous));
    assert!(!previous.metadata.modes.cursor_keys);
    assert!(pump.finish_batch().unwrap().metadata.modes.cursor_keys);
    pump.close().unwrap();
}

#[test]
fn invalid_generated_input_does_not_write_close_or_poison_the_pump() {
    use crate::ghostty::input::{KeyCode, KeyEvent, MAX_KEY_TEXT_BYTES};
    let (mut pump, transport, _) = pump();
    let mut key = KeyEvent::new(KeyCode::UNIDENTIFIED);
    key.text = "x".repeat(MAX_KEY_TEXT_BYTES + 1);
    assert!(pump.key(key).is_err());
    let mut mouse = input_mouse();
    mouse.x = f32::NAN;
    assert!(pump.mouse(mouse).is_err());
    assert!(matches!(
        pump.paste(&"x".repeat(crate::paste::DEFAULT_MAX_PASTE_BYTES + 1)),
        Err(Error::PasteTooLarge { .. })
    ));
    assert!(transport.lock().operations.is_empty());
    pump.key(KeyEvent::new(KeyCode::ENTER)).unwrap();
    assert_eq!(transport.lock().writes, [b"\r".to_vec()]);
    pump.close().unwrap();
}

#[test]
fn generated_input_delivery_failure_stops_without_replay_for_each_input_kind() {
    use crate::ghostty::input::{KeyCode, KeyEvent};
    for kind in 0..3 {
        let (mut pump, transport, mut reader) = pump();
        let previous = pump.finish_batch().unwrap();
        reader.poll().unwrap();
        pump.process_chunk(b"\x1b[?1000;1006h").unwrap();
        transport.lock().write_error = Some(TerminalError::QueueFull);
        let result = match kind {
            0 => pump.key(KeyEvent::new(KeyCode::ENTER)),
            1 => pump.mouse(input_mouse()),
            _ => pump.paste("text"),
        };
        assert!(matches!(
            result,
            Err(Error::Transport(TerminalError::QueueFull))
        ));
        assert_eq!(transport.lock().writes.len(), 1);
        assert_eq!(transport.lock().closes, 1);
        assert!(matches!(
            pump.key(KeyEvent::new(KeyCode::ENTER)),
            Err(Error::Closed)
        ));
        assert!(matches!(pump.mouse(input_mouse()), Err(Error::Closed)));
        assert!(matches!(pump.paste("later"), Err(Error::Closed)));
        assert!(matches!(pump.finish_batch(), Err(Error::Closed)));
        let update = reader.poll().unwrap();
        assert_eq!(update.state, State::Closed);
        assert!(Arc::ptr_eq(update.frame.as_ref().unwrap(), &previous));
        drop(pump);
        assert_eq!(transport.lock().writes.len(), 1);
        assert_eq!(transport.lock().closes, 1);
    }
}

#[test]
fn paste_uses_live_native_mode_and_preserves_policy_at_the_byte_limit() {
    let (mut pump, transport, _) = pump();
    pump.paste("a\r\nb\nc").unwrap();
    assert_eq!(transport.lock().writes, [b"a\rb\rc".to_vec()]);
    pump.process_chunk(b"\x1b[?2004h").unwrap();
    let payload = "x".repeat(crate::paste::DEFAULT_MAX_PASTE_BYTES);
    pump.paste(&payload).unwrap();
    {
        let state = transport.lock();
        let bytes = &state.writes[1];
        assert_eq!(bytes.len(), payload.len() + 12);
        assert!(bytes.starts_with(b"\x1b[200~"));
        assert!(bytes.ends_with(b"\x1b[201~"));
    }
    pump.process_chunk(b"\x1b[?2004l").unwrap();
    pump.paste("a\nb").unwrap();
    assert_eq!(transport.lock().writes[2], b"a\rb");
    pump.close().unwrap();
}

#[test]
fn replies_are_enqueued_before_publication_and_batches_share_one_frame() {
    let (mut pump, transport, mut reader) = pump();
    pump.process_chunk(b"A\x1b[6n").unwrap();
    pump.process_chunk(b"B\x1b[6n").unwrap();
    assert!(reader.poll().unwrap().frame.is_none());
    {
        let state = transport.lock();
        assert_eq!(state.writes, [b"\x1b[1;2R".to_vec(), b"\x1b[1;3R".to_vec()]);
        assert!(state.frames_at_write.iter().all(Option::is_none));
    }
    let frame = pump.finish_batch().unwrap();
    let update = reader.poll().unwrap();
    assert_eq!(update.damage, Damage::Full);
    assert!(Arc::ptr_eq(update.frame.as_ref().unwrap(), &frame));
    assert_eq!(frame.rows[0].cells[0].text, "A");
    assert_eq!(frame.rows[0].cells[1].text, "B");
    pump.close().unwrap();
    assert_eq!(reader.poll().unwrap().state, State::Closed);
    assert_eq!(transport.lock().closes, 1);
}

#[test]
fn every_input_split_keeps_unicode_and_reply_order() {
    let bytes = "A中\x1b[6n\x1b[2;1HB\x1b[6n".as_bytes();
    for split in 0..=bytes.len() {
        let (mut pump, transport, _) = pump();
        pump.process_chunk(&bytes[..split]).unwrap();
        pump.process_chunk(&bytes[split..]).unwrap();
        assert_eq!(transport.lock().writes.concat(), b"\x1b[1;4R\x1b[2;2R");
        let frame = pump.finish_batch().unwrap();
        assert_eq!(frame.rows[0].cells[1].text, "中");
        assert_eq!(frame.cursor.position, Some((1, 1)));
        pump.close().unwrap();
    }
}

#[test]
fn large_reads_drain_replies_between_bounded_chunks() {
    let (mut pump, transport, _) = pump();
    let mut bytes = vec![b'\r'; MAX_FEED_BYTES * 2 + 4];
    for offset in [0, MAX_FEED_BYTES, MAX_FEED_BYTES * 2] {
        bytes[offset..offset + 4].copy_from_slice(b"\x1b[6n");
    }
    pump.process_chunk(&bytes).unwrap();
    assert_eq!(transport.lock().writes, vec![b"\x1b[1;1R".to_vec(); 3]);
    pump.finish_batch().unwrap();
    pump.close().unwrap();
}

#[test]
fn empty_and_reply_free_reads_never_write_empty_transport_payloads() {
    let (mut pump, transport, mut reader) = pump();
    pump.process_chunk(b"").unwrap();
    pump.process_chunk(b"hello").unwrap();
    assert!(transport.lock().writes.is_empty());
    assert!(reader.poll().unwrap().frame.is_none());
    pump.finish_batch().unwrap();
    pump.close().unwrap();
}

#[test]
fn reply_rejection_stops_without_retry_or_partial_publication() {
    for failure in [
        TerminalError::QueueFull,
        TerminalError::Closed,
        TerminalError::Transport("fixture".into()),
    ] {
        let (mut pump, transport, mut reader) = pump();
        let previous = pump.finish_batch().unwrap();
        reader.poll().unwrap();
        transport.lock().write_error = Some(failure.clone());
        assert!(
            matches!(pump.process_chunk(b"changed\x1b[6n"), Err(Error::Transport(error)) if error == failure)
        );
        assert!(matches!(pump.process_chunk(b"later"), Err(Error::Closed)));
        assert!(matches!(pump.finish_batch(), Err(Error::Closed)));
        assert!(matches!(pump.resize(10, 4, 9, 18), Err(Error::Closed)));
        assert!(matches!(pump.reader(), Err(Error::Closed)));
        let update = reader.poll().unwrap();
        assert_eq!(update.state, State::Closed);
        assert_eq!(update.damage, Damage::None);
        assert!(Arc::ptr_eq(update.frame.as_ref().unwrap(), &previous));
        {
            let state = transport.lock();
            assert_eq!(state.writes.len(), 1);
            assert!(Arc::ptr_eq(
                state.frames_at_write[0].as_ref().unwrap(),
                &previous
            ));
            assert_eq!(state.closes, 1);
        }
        drop(pump);
        assert_eq!(transport.lock().closes, 1);
    }
}

#[test]
fn invalid_resize_preserves_state_and_valid_resize_forwards_rows_then_columns() {
    let (mut pump, transport, _) = pump();
    assert!(matches!(
        pump.resize(0, 3, 9, 18),
        Err(Error::Engine(OwnerError::Native(NativeError::InvalidSize)))
    ));
    assert!(matches!(
        pump.resize(1025, 1024, 9, 18),
        Err(Error::Engine(OwnerError::Native(
            NativeError::SnapshotLimit
        )))
    ));
    assert!(transport.lock().sizes.is_empty());
    pump.process_chunk(b"\x1b[6n").unwrap();
    pump.resize(20, 5, 9, 18).unwrap();
    assert_eq!(transport.lock().sizes, [(5, 20)]);
    let frame = pump.finish_batch().unwrap();
    assert_eq!(frame.cols, 20);
    assert_eq!(frame.rows.len(), 5);
    pump.close().unwrap();
}

#[test]
fn failed_transport_resize_stops_without_publishing_native_only_size() {
    let (mut pump, transport, mut reader) = pump();
    let previous = pump.finish_batch().unwrap();
    reader.poll().unwrap();
    transport.lock().resize_error = Some(TerminalError::QueueFull);
    assert!(matches!(
        pump.resize(20, 5, 9, 18),
        Err(Error::Transport(TerminalError::QueueFull))
    ));
    assert!(matches!(pump.finish_batch(), Err(Error::Closed)));
    let update = reader.poll().unwrap();
    assert_eq!(update.state, State::Closed);
    assert!(Arc::ptr_eq(update.frame.as_ref().unwrap(), &previous));
    assert_eq!(transport.lock().sizes, [(5, 20)]);
    assert_eq!(transport.lock().closes, 1);
}

#[test]
fn native_reply_overflow_closes_transport_without_delivering_truncated_replies() {
    let (mut pump, transport, mut reader) = pump();
    let previous = pump.finish_batch().unwrap();
    assert!(matches!(
        pump.process_chunk(&b"\x1b[6n".repeat(4096)),
        Err(Error::Engine(OwnerError::Native(
            NativeError::ReplyOverflow
        )))
    ));
    assert!(transport.lock().writes.is_empty());
    assert_eq!(transport.lock().closes, 1);
    let update = reader.poll().unwrap();
    assert_eq!(
        update.state,
        State::Failed(Failure::Native(NativeError::ReplyOverflow))
    );
    assert!(Arc::ptr_eq(update.frame.as_ref().unwrap(), &previous));
}

#[test]
fn dropping_an_active_pump_requests_transport_close() {
    let (pump, transport, _) = pump();
    drop(pump);
    assert_eq!(transport.lock().closes, 1);
}

#[test]
fn resize_report_is_enqueued_after_transport_resize_before_new_frame() {
    let (mut pump, transport, mut reader) = pump();
    let previous = pump.finish_batch().unwrap();
    reader.poll().unwrap();
    pump.process_chunk(b"\x1b[?2048h").unwrap();
    pump.resize(20, 5, 9, 18).unwrap();
    {
        let state = transport.lock();
        assert_eq!(state.operations, ["resize", "write"]);
        assert_eq!(state.writes, [b"\x1b[48;5;20;90;180t".to_vec()]);
        assert!(Arc::ptr_eq(
            state.frames_at_write[0].as_ref().unwrap(),
            &previous
        ));
    }
    assert!(Arc::ptr_eq(
        reader.poll().unwrap().frame.as_ref().unwrap(),
        &previous
    ));
    assert_eq!(pump.finish_batch().unwrap().cols, 20);
    pump.close().unwrap();
}

#[test]
fn rejected_resize_report_cannot_publish_the_resized_frame() {
    let (mut pump, transport, mut reader) = pump();
    let previous = pump.finish_batch().unwrap();
    pump.process_chunk(b"\x1b[?2048h").unwrap();
    transport.lock().write_error = Some(TerminalError::QueueFull);
    assert!(matches!(
        pump.resize(20, 5, 9, 18),
        Err(Error::Transport(TerminalError::QueueFull))
    ));
    assert_eq!(transport.lock().operations, ["resize", "write", "close"]);
    assert!(matches!(pump.finish_batch(), Err(Error::Closed)));
    let update = reader.poll().unwrap();
    assert_eq!(update.state, State::Closed);
    assert!(Arc::ptr_eq(update.frame.as_ref().unwrap(), &previous));
}

#[test]
fn close_failure_is_returned_after_worker_teardown() {
    let (pump, transport, mut reader) = pump();
    transport.lock().close_error = Some(TerminalError::Transport("close fixture".into()));
    assert!(matches!(
        pump.close(),
        Err(Error::Transport(TerminalError::Transport(_)))
    ));
    assert_eq!(reader.poll().unwrap().state, State::Closed);
    assert_eq!(transport.lock().closes, 1);
}

#[test]
fn failed_cleanup_preserves_the_original_reply_rejection() {
    let (mut pump, transport, mut reader) = pump();
    {
        let mut state = transport.lock();
        state.write_error = Some(TerminalError::QueueFull);
        state.close_error = Some(TerminalError::Transport("close fixture".into()));
    }
    assert!(matches!(
        pump.process_chunk(b"\x1b[6n"),
        Err(Error::Transport(TerminalError::QueueFull))
    ));
    assert_eq!(reader.poll().unwrap().state, State::Closed);
    assert_eq!(transport.lock().closes, 1);
}

#[test]
fn native_events_reuse_policy_and_arrive_before_repaint() {
    use crate::SessionEvent;
    let (mut pump, transport, _) = pump();
    pump.process_chunk(b"\x1b]2;hello\x07\x1b]7;file://host/C:/a%20b\x07\x07\x1b[6n")
        .unwrap();
    let rx = transport.lock().events.as_ref().unwrap().clone();
    assert!(rx.try_recv().is_err());
    assert_eq!(transport.lock().writes.concat(), b"\x1b[1;1R");
    pump.finish_batch().unwrap();
    assert_eq!(
        std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>(),
        [
            SessionEvent::Title("hello".into()),
            SessionEvent::Cwd(std::path::PathBuf::from("C:/a b")),
            SessionEvent::Bell,
            SessionEvent::Output,
        ]
    );
    assert_eq!(pump.router.state().lock().title.as_deref(), Some("hello"));
    pump.close().unwrap();
}

#[test]
fn directory_and_title_reuse_existing_control_stripping_policy() {
    use crate::SessionEvent;
    let (mut pump, transport, _) = pump();
    pump.process_chunk("\x1b]2;ab\u{202e}cd\x07\x1b]7;file://host/tmp/a%0Ab\x07".as_bytes())
        .unwrap();
    pump.finish_batch().unwrap();
    let rx = transport.lock().events.as_ref().unwrap().clone();
    assert_eq!(
        std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>(),
        [
            SessionEvent::Title("abcd".into()),
            SessionEvent::Cwd(std::path::PathBuf::from("/tmp/ab")),
            SessionEvent::Output
        ]
    );
    assert_eq!(
        pump.router.state().lock().cwd,
        Some(std::path::PathBuf::from("/tmp/ab"))
    );
    pump.close().unwrap();
}

#[test]
fn accumulated_event_count_is_bounded_across_reads() {
    let (mut pump, transport, mut reader) = pump();
    let previous = pump.finish_batch().unwrap();
    for _ in 0..MAX_EVENTS {
        pump.process_chunk(b"\x07").unwrap();
    }
    assert!(matches!(
        pump.process_chunk(b"\x07"),
        Err(Error::Engine(OwnerError::Native(
            NativeError::EventOverflow
        )))
    ));
    assert!(matches!(pump.finish_batch(), Err(Error::Closed)));
    let update = reader.poll().unwrap();
    assert!(Arc::ptr_eq(update.frame.as_ref().unwrap(), &previous));
    let rx = transport.lock().events.as_ref().unwrap().clone();
    assert_eq!(
        std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>(),
        [crate::SessionEvent::Output]
    );
}

#[test]
fn accumulated_event_text_is_bounded_across_reads() {
    use base64::Engine;
    let (mut pump, transport, _) = pump();
    let encoded = base64::engine::general_purpose::STANDARD.encode(vec![b'x'; MAX_EVENT_BYTES / 2]);
    let clipboard = format!("\x1b]52;c;{encoded}\x07");
    for _ in 0..2 {
        pump.process_chunk(clipboard.as_bytes()).unwrap();
    }
    assert_eq!(pump.pending_bytes, MAX_EVENT_BYTES);
    assert!(matches!(
        pump.process_chunk(b"\x1b]2;over\x07"),
        Err(Error::Engine(OwnerError::Native(
            NativeError::EventOverflow
        )))
    ));
    assert!(
        transport
            .lock()
            .events
            .as_ref()
            .unwrap()
            .try_recv()
            .is_err()
    );
}

#[test]
fn explicit_close_flushes_pending_reliable_events_without_a_new_frame() {
    let (mut pump, transport, mut reader) = pump();
    pump.process_chunk(b"\x1b]2;last\x07").unwrap();
    pump.close().unwrap();
    let rx = transport.lock().events.as_ref().unwrap().clone();
    assert_eq!(
        std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>(),
        [crate::SessionEvent::Title("last".into())]
    );
    assert!(reader.poll().unwrap().frame.is_none());
}

#[test]
fn reliable_backpressure_does_not_hold_native_or_publication_locks() {
    use super::super::{SessionEventSink, SharedSessionState};
    use crate::{ClipboardOrigin, SessionEvent};
    let transport = Transport::default();
    let (tx, rx) = async_channel::bounded(1);
    tx.send_blocking(SessionEvent::Bell).unwrap();
    let router = OscRouter::new(
        transport.clone(),
        SessionEventSink::new(tx),
        SharedSessionState::new_alive(),
        ClipboardOrigin::Remote,
    );
    let mut pump = GhosttyPump::new(router, 16, 3).unwrap();
    let mut reader = pump.reader().unwrap();
    transport.lock().reader = Some(pump.reader().unwrap());
    pump.process_chunk(b"\x1b]2;queued\x07\x1b[6n").unwrap();
    assert_eq!(transport.lock().writes.concat(), b"\x1b[1;1R");
    let (done_tx, done_rx) = std::sync::mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        pump.finish_batch().unwrap();
        done_tx.send(()).unwrap();
        pump.close().unwrap();
    });
    assert!(matches!(
        done_rx.recv_timeout(std::time::Duration::from_millis(50)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    ));
    assert!(reader.poll().unwrap().frame.is_none());
    assert_eq!(rx.recv_blocking().unwrap(), SessionEvent::Bell);
    assert_eq!(
        rx.recv_blocking().unwrap(),
        SessionEvent::Title("queued".into())
    );
    done_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    worker.join().unwrap();
    assert_eq!(reader.poll().unwrap().state, State::Closed);
}
