use std::time::Duration;

use super::*;

const TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn preparation_failure_does_not_commit_the_watermark_or_hold_the_lock() {
    let mut owner = EngineOwner::new(8, 3).unwrap();
    let mut reader = owner.reader();
    owner.capture().unwrap();
    assert!(
        reader
            .poll_prepared(|frame| {
                assert!(frame.is_some());
                // Acquiring lifecycle state here would deadlock under the shared lock.
                assert_eq!(owner.shared.state(), State::Running);
                Err(NativeError::Allocation)
            })
            .is_err()
    );
    assert_eq!(reader.poll().unwrap().damage, Damage::Full);
    owner.close().unwrap();
}

#[test]
fn callback_events_cross_the_worker_as_owned_data() {
    use super::super::events::Event;
    let mut owner = EngineOwner::new(16, 3).unwrap();
    owner
        .feed(b"\x1b]2;owned\x07\x1b]7;/tmp/a\x07\x07")
        .unwrap();
    let events = owner.take_events().unwrap();
    assert_eq!(events.iter().map(Event::text_len).sum::<usize>(), 11);
    assert!(owner.take_events().unwrap().is_empty());
    owner.close().unwrap();
    assert_eq!(
        events,
        [
            Event::Title("owned".into()),
            Event::WorkingDirectory("/tmp/a".into()),
            Event::Bell
        ]
    );
}

#[test]
fn abandoned_response_preserves_a_native_failure_over_delivery_failure() {
    for fatal in [false, true] {
        let owner = EngineOwner::new(8, 2).unwrap();
        let mut reader = owner.reader();
        let (reply, receiver) = mpsc::sync_channel(1);
        drop(receiver);
        let bytes = if fatal {
            b"\x1b[6n".repeat(super::super::REPLY_CAPACITY)
        } else {
            b"x".to_vec()
        };
        owner
            .sender
            .as_ref()
            .unwrap()
            .send(Request {
                operation: Operation::Feed(bytes),
                reply,
            })
            .unwrap();
        if fatal {
            assert!(matches!(
                owner.close(),
                Err(Error::Native(NativeError::ReplyOverflow))
            ));
            assert_eq!(
                reader.poll().unwrap().state,
                State::Failed(Failure::Native(NativeError::ReplyOverflow))
            );
        } else {
            assert!(matches!(owner.close(), Err(Error::Abandoned)));
            assert_eq!(
                reader.poll().unwrap().state,
                State::Failed(Failure::Abandoned)
            );
        }
    }
}

#[test]
fn repeated_empty_shutdown_preserves_reader_lifecycle() {
    for _ in 0..32 {
        let owner = EngineOwner::new(8, 2).unwrap();
        let mut reader = owner.reader();
        owner.close().unwrap();
        let update = reader.poll().unwrap();
        assert_eq!(update.state, State::Closed);
        assert!(update.frame.is_none());
        assert_eq!(update.damage, Damage::None);
    }
}

#[test]
fn endpoint_moves_threads_but_native_state_stays_on_worker() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<EngineOwner>();
    assert_send_sync::<Frame>();
    let owner = EngineOwner::new(16, 3).unwrap();
    let worker_id = owner.worker.as_ref().unwrap().thread().id();
    assert_ne!(worker_id, thread::current().id());
    let (mut owner, frame) = thread::spawn(move || {
        let mut owner = owner;
        assert_ne!(worker_id, thread::current().id());
        assert_eq!(owner.feed("A中\x1b[6n".as_bytes()).unwrap(), b"\x1b[1;4R");
        let frame = owner.capture().unwrap();
        (owner, frame)
    })
    .join()
    .unwrap();
    assert_eq!(frame.rows[0].cells[1].text, "中");
    assert_eq!(owner.feed(b"B\x1b[6n").unwrap(), b"\x1b[1;5R");
    let mut reader = owner.reader();
    assert_eq!(reader.poll().unwrap().state, State::Running);
    owner.close().unwrap();
    let closed = reader.poll().unwrap();
    assert_eq!(closed.state, State::Closed);
    assert!(Arc::ptr_eq(closed.frame.as_ref().unwrap(), &frame));
    assert_eq!(frame.cursor.position, Some((3, 0)));
}

#[test]
fn split_feeds_drain_replies_in_order_before_explicit_publication() {
    let stream = "\x1b[31mA中\x1b[6n\x1b[2;3Hx\x1b[6n".as_bytes();
    for split in 0..=stream.len() {
        let mut owner = EngineOwner::new(16, 3).unwrap();
        let mut reader = owner.reader();
        let mut transport = owner.feed(&stream[..split]).unwrap();
        transport.extend(owner.feed(&stream[split..]).unwrap());
        assert_eq!(transport, b"\x1b[1;4R\x1b[2;4R");
        assert!(
            reader.poll().unwrap().frame.is_none(),
            "feed alone does not publish"
        );
        let frame = owner.capture().unwrap();
        assert!(Arc::ptr_eq(
            reader.poll().unwrap().frame.as_ref().unwrap(),
            &frame
        ));
        assert_eq!(frame.cursor.position, Some((3, 1)));
        owner.close().unwrap();
    }
}

#[test]
fn readers_share_frames_but_not_damage_watermarks() {
    let mut owner = EngineOwner::new(8, 4).unwrap();
    let mut fast = owner.reader();
    let mut slow = owner.reader();
    assert_eq!(fast.poll().unwrap().damage, Damage::None);
    owner.feed(b"a").unwrap();
    let original = owner.capture().unwrap();
    assert_eq!(fast.poll().unwrap().damage, Damage::Full);
    assert_eq!(slow.poll().unwrap().damage, Damage::Full);
    assert_eq!(fast.poll().unwrap().damage, Damage::None);
    owner.feed(b"\x1b[2;1Hb").unwrap();
    owner.capture().unwrap();
    assert_eq!(fast.poll().unwrap().damage, Damage::Rows(vec![0, 1]));
    owner.feed(b"\x1b[3;1Hc").unwrap();
    let latest = owner.capture().unwrap();
    assert_eq!(fast.poll().unwrap().damage, Damage::Rows(vec![1, 2]));
    let slow_update = slow.poll().unwrap();
    assert_eq!(slow_update.damage, Damage::Rows(vec![0, 1, 2]));
    assert!(Arc::ptr_eq(slow_update.frame.as_ref().unwrap(), &latest));
    assert_eq!(original.rows[1].cells[0].text, "");
    owner.capture().unwrap();
    assert_eq!(fast.poll().unwrap().damage, Damage::None);
    assert_eq!(slow.poll().unwrap().damage, Damage::None);
    owner.close().unwrap();
}

#[test]
fn cursor_only_changes_damage_both_rows_and_global_colors_damage_all() {
    let mut owner = EngineOwner::new(8, 4).unwrap();
    let mut reader = owner.reader();
    owner.capture().unwrap();
    reader.poll().unwrap();
    owner.feed(b"\x1b[3;1H").unwrap();
    owner.capture().unwrap();
    assert_eq!(reader.poll().unwrap().damage, Damage::Rows(vec![0, 2]));
    owner.feed(b"\x1b[?25l").unwrap();
    owner.capture().unwrap();
    assert_eq!(reader.poll().unwrap().damage, Damage::Rows(vec![2]));
    owner.feed(b"\x1b]4;1;rgb:11/22/33\x07").unwrap();
    owner.capture().unwrap();
    assert_eq!(reader.poll().unwrap().damage, Damage::Full);
    owner.resize(10, 5, 9, 18).unwrap();
    owner.capture().unwrap();
    assert_eq!(reader.poll().unwrap().damage, Damage::Full);
    owner.close().unwrap();
}

#[test]
fn invalid_requests_are_rejected_whole_and_leave_state_usable() {
    assert!(matches!(
        EngineOwner::new(0, 2),
        Err(Error::Native(NativeError::InvalidSize))
    ));
    assert!(matches!(
        EngineOwner::new(1025, 1024),
        Err(Error::Native(NativeError::SnapshotLimit))
    ));
    let mut owner = EngineOwner::new(8, 2).unwrap();
    owner.feed(b"x").unwrap();
    assert!(matches!(
        owner.feed(&vec![b'y'; MAX_FEED_BYTES + 1]),
        Err(Error::InputLimit)
    ));
    assert!(matches!(
        owner.resize(8, 0, 9, 18),
        Err(Error::Native(NativeError::InvalidSize))
    ));
    assert!(matches!(
        owner.resize(1025, 1024, 9, 18),
        Err(Error::Native(NativeError::SnapshotLimit))
    ));
    assert_eq!(owner.feed(b"\x1b[6n").unwrap(), b"\x1b[1;2R");
    let frame = owner.capture().unwrap();
    assert_eq!(frame.cols, 8);
    assert_eq!(frame.rows.len(), 2);
    assert_eq!(frame.rows[0].cells[1].text, "");
    assert!(owner.feed(&vec![b' '; MAX_FEED_BYTES]).is_ok());
    owner.close().unwrap();
}

#[test]
fn fatal_reply_overflow_is_visible_and_retains_only_the_last_complete_frame() {
    let mut owner = EngineOwner::new(8, 2).unwrap();
    let mut reader = owner.reader();
    let last = owner.capture().unwrap();
    reader.poll().unwrap();
    let flood = b"\x1b[6n".repeat(super::super::REPLY_CAPACITY);
    assert!(matches!(
        owner.feed(&flood),
        Err(Error::Native(NativeError::ReplyOverflow))
    ));
    assert!(matches!(
        owner.feed(b"later"),
        Err(Error::Native(NativeError::ReplyOverflow))
    ));
    assert!(matches!(
        owner.capture(),
        Err(Error::Native(NativeError::ReplyOverflow))
    ));
    assert!(matches!(
        owner.close(),
        Err(Error::Native(NativeError::ReplyOverflow))
    ));
    let failed = reader.poll().unwrap();
    assert_eq!(
        failed.state,
        State::Failed(Failure::Native(NativeError::ReplyOverflow))
    );
    assert_eq!(failed.damage, Damage::None);
    assert!(Arc::ptr_eq(failed.frame.as_ref().unwrap(), &last));
}

#[test]
fn worker_panic_is_reported_to_readers_and_explicit_close() {
    let mut owner = EngineOwner::new(8, 2).unwrap();
    let mut reader = owner.reader();
    assert!(matches!(
        owner.request(Operation::Panic),
        Err(Error::WorkerPanicked)
    ));
    assert_eq!(
        reader.poll().unwrap().state,
        State::Failed(Failure::Panicked)
    );
    assert!(matches!(owner.close(), Err(Error::WorkerPanicked)));
    assert_eq!(
        reader.poll().unwrap().state,
        State::Failed(Failure::Panicked)
    );
}

#[test]
fn drop_disconnects_without_waiting_for_an_active_native_worker() {
    let mut owner = EngineOwner::new(8, 2).unwrap();
    let mut reader = owner.reader();
    let worker = owner.worker.take().unwrap();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    owner
        .sender
        .as_ref()
        .unwrap()
        .send(Request {
            operation: Operation::Wait {
                entered: entered_tx,
                release: release_rx,
            },
            reply: reply_tx,
        })
        .unwrap();
    entered_rx.recv_timeout(TIMEOUT).unwrap();
    let busy_update = reader.poll().unwrap();
    assert_eq!(busy_update.state, State::Running);
    assert!(busy_update.frame.is_none());
    let (dropped_tx, dropped_rx) = mpsc::sync_channel(1);
    let dropping = thread::spawn(move || {
        drop(owner);
        dropped_tx.send(()).unwrap();
    });
    let dropped = dropped_rx.recv_timeout(TIMEOUT);
    // Release even on assertion failure, so the fixture cannot strand a thread.
    release_tx.send(()).unwrap();
    dropped.unwrap();
    assert!(reply_rx.recv_timeout(TIMEOUT).unwrap().is_ok());
    dropping.join().unwrap();
    worker.join().unwrap();
    assert_eq!(reader.poll().unwrap().state, State::Closed);
}

#[test]
fn command_queue_has_one_slot_even_while_the_worker_is_busy() {
    let mut owner = EngineOwner::new(8, 2).unwrap();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    owner
        .sender
        .as_ref()
        .unwrap()
        .send(Request {
            operation: Operation::Wait {
                entered: entered_tx,
                release: release_rx,
            },
            reply: reply_tx,
        })
        .unwrap();
    entered_rx.recv_timeout(TIMEOUT).unwrap();
    let (queued_tx, queued_rx) = mpsc::sync_channel(1);
    owner
        .sender
        .as_ref()
        .unwrap()
        .try_send(Request {
            operation: Operation::Feed(b"x".to_vec()),
            reply: queued_tx,
        })
        .unwrap();
    let (refused_tx, _refused_rx) = mpsc::sync_channel(1);
    let refused = owner.sender.as_ref().unwrap().try_send(Request {
        operation: Operation::Feed(b"not accepted".to_vec()),
        reply: refused_tx,
    });
    release_tx.send(()).unwrap();
    assert!(matches!(refused, Err(mpsc::TrySendError::Full(_))));
    assert!(reply_rx.recv_timeout(TIMEOUT).unwrap().is_ok());
    assert!(queued_rx.recv_timeout(TIMEOUT).unwrap().is_ok());
    assert_eq!(owner.feed(b"\x1b[6n").unwrap(), b"\x1b[1;2R");
    owner.close().unwrap();
}
