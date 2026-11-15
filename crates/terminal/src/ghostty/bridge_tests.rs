use super::*;
use crate::ghostty::render;
use std::future::Future;
use std::sync::mpsc;
use std::task::{Context, Poll, Wake, Waker};

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

fn call(engine: &AsyncEngine, command: Command) -> Reply {
    block_on(engine.request(command).unwrap().wait()).unwrap()
}

fn barrier(engine: &AsyncEngine) -> (Pending, mpsc::SyncSender<()>) {
    let (entered, receiver) = mpsc::sync_channel(1);
    let (release, wait) = mpsc::sync_channel(1);
    let pending = engine.request(Command::Wait(entered, wait)).unwrap();
    receiver.recv().unwrap();
    (pending, release)
}

#[test]
fn configured_async_engine_disables_history_before_first_feed() {
    let engine = AsyncEngine::spawn_with_scrollback(10, 3, Some(0)).unwrap();
    call(&engine, Command::Feed([b'x', 13, 10].repeat(10)));
    call(&engine, Command::Capture);
    assert_eq!(
        engine
            .model()
            .query()
            .viewport
            .unwrap()
            .metadata()
            .scrollbar
            .total,
        3
    );
    block_on(engine.close()).unwrap();
}

#[test]
fn native_query_does_not_capture_or_consume_damage() {
    use crate::ghostty_content::Content;
    let engine = AsyncEngine::spawn(12, 3).unwrap();
    let model = engine.model();
    assert!(model.query().viewport.is_none());
    call(&engine, Command::Feed(b"abc".to_vec()));
    assert!(model.query().viewport.is_none(), "query cannot capture");
    call(&engine, Command::Capture);
    let mut content = Content::new(&model);
    let retained = model.query().viewport.unwrap();
    for _ in 0..10 {
        assert_eq!(
            model.query().viewport.unwrap().cursor().position,
            Some((3, 0))
        );
    }
    content.poll().unwrap();
    assert_eq!(content.damage(), &owner::Damage::Full);
    call(&engine, Command::Feed(b"\x1b[?1049hZ".to_vec()));
    call(&engine, Command::Capture);
    let query = model.query();
    assert!(query.viewport.unwrap().metadata().alternate_screen);
    assert!(!retained.metadata().alternate_screen);
    assert_eq!(retained.cursor().position, Some((3, 0)));
    content.poll().unwrap();
    assert_eq!(content.damage(), &owner::Damage::Full);
    block_on(engine.close()).unwrap();
    let query = model.query();
    assert!(matches!(query.state, State::Closed));
    assert!(query.viewport.unwrap().metadata().alternate_screen);
    content.poll().unwrap();
    assert_eq!(content.damage(), &owner::Damage::None);
    let empty = AsyncEngine::spawn(12, 3).unwrap();
    block_on(empty.close()).unwrap();
    let query = empty.model().query();
    assert!(matches!(query.state, State::Closed));
    assert!(query.viewport.is_none());
}

#[test]
fn native_content_has_atomic_metadata_windows_and_independent_damage() {
    use crate::ghostty_content::{Content, RenderSource};
    let engine = AsyncEngine::spawn(12, 3).unwrap();
    let model = engine.model();
    let source: &dyn RenderSource = &model;
    let mut first = Content::new(source);
    let mut second = Content::new(source);
    first.poll().unwrap();
    assert!(first.viewport().is_none());
    assert_eq!(first.damage(), &owner::Damage::None);
    call(
        &engine,
        Command::Feed(b"abcdef\x1b[?2004h\x1b[?1h\x1b[?1003h\x1b[?1006h\x1b[5 q".to_vec()),
    );
    call(&engine, Command::Capture);
    first.poll().unwrap();
    assert_eq!(first.damage(), &owner::Damage::Full);
    let retained = first.viewport().unwrap().clone();
    assert_eq!((retained.columns(), retained.lines()), (12, 3));
    assert!(retained.metadata().modes.bracketed_paste);
    assert!(retained.metadata().modes.cursor_keys);
    assert!(retained.metadata().modes.mouse_any);
    assert!(retained.metadata().modes.mouse_sgr);
    assert_eq!(retained.cursor().position, Some((6, 0)));
    assert_eq!(retained.cursor().shape, render::CursorShape::Bar);
    let key = retained.row(0).unwrap().key();
    assert!(retained.row(3).is_none());
    assert_eq!(retained.rows(1, usize::MAX).len(), 2);
    assert_eq!(retained.rows(usize::MAX, usize::MAX).len(), 0);
    assert_eq!(retained.rows(0, 0).len(), 0);
    assert_eq!(retained.row(0).unwrap().key(), key);
    assert_eq!(first.damage(), &owner::Damage::Full);
    second.poll().unwrap();
    assert_eq!(second.damage(), &owner::Damage::Full);
    first.poll().unwrap();
    assert_eq!(first.damage(), &owner::Damage::None);
    call(&engine, Command::Feed(b"\rZ".to_vec()));
    call(&engine, Command::Capture);
    first.poll().unwrap();
    assert_eq!(first.damage(), &owner::Damage::Rows(vec![0]));
    assert_ne!(first.viewport().unwrap().row(0).unwrap().key(), key);
    assert_eq!(retained.row(0).unwrap().key(), key);
    second.poll().unwrap();
    assert_eq!(second.damage(), &owner::Damage::Rows(vec![0]));
    block_on(engine.close()).unwrap();
    first.poll().unwrap();
    assert!(matches!(first.state(), State::Closed));
    assert!(first.viewport().is_some());
    assert_eq!(first.damage(), &owner::Damage::None);
    let mut late = Content::new(source);
    late.poll().unwrap();
    assert!(matches!(late.state(), State::Closed));
    assert_eq!(late.damage(), &owner::Damage::Full);
}

#[test]
fn native_content_tracks_selection_colors_resize_scroll_and_retains_old_viewport() {
    use crate::ghostty_content::Content;
    use anchors::{Coordinate, Point, Space};
    let engine = AsyncEngine::spawn(12, 3).unwrap();
    let model = engine.model();
    let mut content = Content::new(&model);
    call(&engine, Command::Feed(b"abcdef\r\nghijkl".to_vec()));
    call(&engine, Command::Capture);
    content.poll().unwrap();
    let before = content.viewport().unwrap().clone();
    let point = |column, row| Point {
        space: Space::Active,
        coordinate: Coordinate { column, row },
    };
    let a = block_on(model.create_anchor(point(1, 0))).unwrap();
    let b = block_on(model.create_anchor(point(3, 1))).unwrap();
    block_on(model.set_selection(&a, &b, true)).unwrap();
    content.poll().unwrap();
    assert_eq!(
        content.damage(),
        &owner::Damage::None,
        "mutation is not publication"
    );
    call(&engine, Command::Capture);
    content.poll().unwrap();
    let selected = content.viewport().unwrap().clone();
    assert_eq!(selected.selection_spans().len(), 2);
    assert_eq!(selected.selection_spans()[0].columns, 1..4);
    assert_eq!(selected.selection_spans()[1].row, 1);
    assert!(before.selection_spans().is_empty());
    let rgb = render::Rgb {
        r: 17,
        g: 34,
        b: 51,
    };
    call(
        &engine,
        Command::DefaultColors(Box::new(ghostty::colors::DefaultColors {
            foreground: rgb,
            background: rgb,
            cursor: Some(rgb),
            palette: [rgb; 256],
        })),
    );
    call(&engine, Command::Capture);
    content.poll().unwrap();
    assert_eq!(content.damage(), &owner::Damage::Full);
    let colored = content.viewport().unwrap();
    assert_eq!(colored.foreground(), rgb);
    assert_eq!(colored.background(), rgb);
    assert_eq!(colored.cursor_color(), Some(rgb));
    assert_eq!(colored.palette(), &[rgb; 256]);
    call(
        &engine,
        Command::Resize {
            cols: 8,
            rows: 2,
            cell_width_px: 8,
            cell_height_px: 16,
        },
    );
    call(
        &engine,
        Command::Feed(b"\r\n1\r\n2\r\n3\r\n4\r\n5".to_vec()),
    );
    call(&engine, Command::Scroll(state::Scroll::Top));
    call(&engine, Command::Capture);
    content.poll().unwrap();
    assert_eq!(content.damage(), &owner::Damage::Full);
    let scrolled = content.viewport().unwrap();
    assert_eq!((scrolled.columns(), scrolled.lines()), (8, 2));
    assert_eq!(scrolled.metadata().scrollbar.offset, 0);
    assert!(scrolled.metadata().scrollbar.total > scrolled.metadata().scrollbar.len);
    assert_eq!(scrolled.cursor().position, None);
    assert_eq!((before.columns(), before.lines()), (12, 3));
    assert_eq!(selected.selection_spans().len(), 2);
    block_on(engine.close()).unwrap();
}

#[test]
fn native_content_retains_last_good_on_failure_and_closed_without_frame_is_explicit() {
    use crate::ghostty_content::Content;
    let empty = AsyncEngine::spawn(12, 3).unwrap();
    let mut content = Content::new(&empty.model());
    block_on(empty.close()).unwrap();
    content.poll().unwrap();
    assert!(matches!(content.state(), State::Closed));
    assert!(content.viewport().is_none());
    let engine = AsyncEngine::spawn(12, 3).unwrap();
    let mut content = Content::new(&engine.model());
    call(&engine, Command::Feed(b"last-good".to_vec()));
    call(&engine, Command::Capture);
    content.poll().unwrap();
    let old = content.viewport().unwrap().clone();
    let key = old.row(0).unwrap().key();
    assert!(
        block_on(
            engine
                .request(Command::Feed(b"\x1b[6n".repeat(1000)))
                .unwrap()
                .wait()
        )
        .is_err()
    );
    assert!(block_on(engine.close()).is_err());
    content.poll().unwrap();
    assert!(matches!(content.state(), State::Failed(_)));
    assert_eq!(content.viewport().unwrap().row(0).unwrap().key(), key);
    assert_eq!(content.damage(), &owner::Damage::None);
    assert_eq!(old.row(0).unwrap().key(), key);
}

#[test]
fn abort_rejects_prepared_publication_and_cannot_be_overwritten_by_startup() {
    let engine = AsyncEngine::spawn(12, 3).unwrap();
    let frame = match call(&engine, Command::Capture) {
        Reply::Frame(frame) => frame,
        _ => panic!("expected frame"),
    };
    let shared = &engine.client.shared;
    engine.abandon_delivery();
    assert!(matches!(
        shared.publish(frame),
        Err(Error::AbandonedDelivery)
    ));
    shared.state(State::Running);
    assert!(matches!(
        shared.read().state,
        State::Failed(Error::AbandonedDelivery)
    ));
    assert!(matches!(
        block_on(engine.close()),
        Err(Error::AbandonedDelivery)
    ));
}

#[test]
fn bounded_admission_and_close_are_nonblocking_and_cancel_queued_work() {
    let engine = AsyncEngine::spawn(12, 3).unwrap();
    let model = engine.model();
    let mut reader = model.reader();
    let (running, release) = barrier(&engine);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut waiting = std::pin::pin!(running.wait());
    assert!(waiting.as_mut().poll(&mut context).is_pending());
    let queued: Vec<_> = (0..COMMAND_CAPACITY)
        .map(|_| engine.request(Command::Capture).unwrap())
        .collect();
    assert!(matches!(engine.request(Command::Capture), Err(Error::Busy)));
    let closing = engine.close(); // Must signal even before polling.
    assert!(matches!(
        engine.request(Command::Capture),
        Err(Error::Closed)
    ));
    release.send(()).unwrap();
    block_on(closing).unwrap();
    for pending in queued {
        assert!(matches!(block_on(pending.wait()), Err(Error::Closed)));
    }
    let closed = reader.poll().unwrap();
    assert!(matches!(closed.state, State::Closed));
    assert!(closed.frame.is_none());
    assert!(matches!(
        block_on(model.clear_selection()),
        Err(Error::Closed)
    ));
}

#[test]
fn cancelled_interaction_runs_once_without_poisoning_the_owner() {
    let engine = AsyncEngine::spawn(12, 3).unwrap();
    let (running, release) = barrier(&engine);
    let interaction = engine.request(Command::Scroll(state::Scroll::Top)).unwrap();
    drop(interaction);
    let capture = engine.request(Command::Capture).unwrap();
    release.send(()).unwrap();
    block_on(running.wait()).unwrap();
    assert!(matches!(block_on(capture.wait()).unwrap(), Reply::Frame(_)));
    block_on(engine.close()).unwrap();
}

#[test]
fn abandoned_buffered_transport_reply_fail_stops_without_publishing() {
    let engine = AsyncEngine::spawn(12, 3).unwrap();
    let model = engine.model();
    let mut reader = model.reader();
    let reply = engine.request(Command::Feed(b"\x1b[6n".to_vec())).unwrap();
    // This barrier proves the DSR result was already placed into its reply slot.
    let (running, release) = barrier(&engine);
    drop(reply);
    release.send(()).unwrap();
    block_on(running.wait()).unwrap();
    assert!(matches!(
        block_on(engine.close()),
        Err(Error::AbandonedDelivery)
    ));
    let failed = reader.poll().unwrap();
    assert!(matches!(
        failed.state,
        State::Failed(Error::AbandonedDelivery)
    ));
    assert!(failed.frame.is_none());
}

#[test]
fn shared_model_uses_prepared_atomic_frames_and_independent_damage() {
    use anchors::{Coordinate, Point, Space};
    let engine = AsyncEngine::spawn(12, 3).unwrap();
    let model = engine.model();
    let clone = model.clone();
    let mut first = model.reader();
    let mut second = clone.reader();
    match call(&engine, Command::Feed(b"abcdef\x1b[6n".to_vec())) {
        Reply::Bytes(bytes) => assert_eq!(bytes, b"\x1b[1;7R"),
        _ => panic!("expected DSR delivery before capture"),
    }
    assert!(first.poll().unwrap().frame.is_none());
    let source = match call(&engine, Command::Capture) {
        Reply::Frame(frame) => frame,
        _ => panic!("expected frame"),
    };
    let before = first.poll().unwrap();
    assert_eq!(before.damage, owner::Damage::Full);
    assert!(Arc::ptr_eq(before.frame.as_ref().unwrap(), &source));
    second.poll().unwrap();
    let make = |column| Point {
        space: Space::Active,
        coordinate: Coordinate { column, row: 0 },
    };
    let a = block_on(model.create_anchor(make(1))).unwrap();
    let b = block_on(clone.create_anchor(make(3))).unwrap();
    block_on(model.set_selection(&a, &b, false)).unwrap();
    assert_eq!(
        block_on(clone.selection_text()).unwrap().as_deref(),
        Some("bcd")
    );
    assert_eq!(first.poll().unwrap().damage, owner::Damage::None);
    call(&engine, Command::Capture);
    let selected = first.poll().unwrap();
    assert_eq!(selected.damage, owner::Damage::Rows(vec![0]));
    assert_eq!(
        selected.frame.as_ref().unwrap().selection_spans()[0].columns,
        1..4
    );
    assert_eq!(second.poll().unwrap().damage, owner::Damage::Rows(vec![0]));
    let found = block_on(model.search("bcd".into(), search::Options::default())).unwrap();
    assert_eq!(found.matches.len(), 1);
    assert_eq!(first.poll().unwrap().damage, owner::Damage::None);
    block_on(model.release_anchor(&a)).unwrap();
    assert!(block_on(clone.set_selection(&a, &b, false)).is_err());
    assert_eq!(
        block_on(model.selection_text()).unwrap().as_deref(),
        Some("bcd")
    );
    block_on(model.release_anchor(&b)).unwrap();
    block_on(engine.close()).unwrap();
    let closed = first.poll().unwrap();
    assert!(matches!(closed.state, State::Closed));
    assert!(Arc::ptr_eq(
        closed.frame.as_ref().unwrap(),
        selected.frame.as_ref().unwrap()
    ));
    assert_eq!(source.selection_spans().len(), 0);
}

#[test]
fn native_failure_retains_shared_models_last_good_frame() {
    let engine = AsyncEngine::spawn(12, 3).unwrap();
    let model = engine.model();
    let mut reader = model.reader();
    call(&engine, Command::Capture);
    let good = reader.poll().unwrap().frame.unwrap();
    let storm = b"\x1b[6n".repeat(1000);
    assert!(block_on(engine.request(Command::Feed(storm)).unwrap().wait()).is_err());
    assert!(block_on(engine.close()).is_err());
    let failed = reader.poll().unwrap();
    assert!(matches!(failed.state, State::Failed(_)));
    assert!(Arc::ptr_eq(failed.frame.as_ref().unwrap(), &good));
}

#[test]
fn invalid_payloads_do_not_enter_the_queue_or_change_publication() {
    let engine = AsyncEngine::spawn(12, 3).unwrap();
    assert!(matches!(
        engine.request(Command::Feed(vec![0; owner::MAX_FEED_BYTES + 1])),
        Err(Error::Owner(_))
    ));
    assert!(matches!(
        engine.request(Command::Resize {
            cols: 0,
            rows: 3,
            cell_width_px: 0,
            cell_height_px: 0
        }),
        Err(Error::Owner(_))
    ));
    assert!(matches!(
        engine.request(Command::Search(
            "a".into(),
            search::Options {
                case_sensitive: true,
                whole_word: false
            }
        )),
        Err(Error::Owner(_))
    ));
    call(&engine, Command::Capture);
    block_on(engine.close()).unwrap();
}

#[test]
fn backend_drop_shuts_down_even_with_live_model_clones() {
    let engine = AsyncEngine::spawn(12, 3).unwrap();
    let model = engine.model();
    let completion = engine.closed.clone();
    drop(engine);
    assert!(completion.recv_blocking().is_err());
    assert!(matches!(
        model.reader().poll().unwrap().state,
        State::Closed
    ));
}

#[test]
fn projection_failure_retains_last_good_shared_frame_and_reports_failure() {
    let engine = AsyncEngine::spawn(12, 3).unwrap();
    let model = engine.model();
    let mut reader = model.reader();
    call(&engine, Command::Capture);
    let good = reader.poll().unwrap().frame.unwrap();
    assert!(matches!(
        block_on(engine.request(Command::MalformedCapture).unwrap().wait()),
        Err(Error::Projection(_))
    ));
    assert!(matches!(
        block_on(engine.close()),
        Err(Error::Projection(_))
    ));
    let failed = reader.poll().unwrap();
    assert!(matches!(failed.state, State::Failed(Error::Projection(_))));
    assert!(Arc::ptr_eq(failed.frame.as_ref().unwrap(), &good));
    assert_eq!(failed.damage, owner::Damage::None);
}

#[test]
fn dispatcher_panic_still_joins_the_native_worker_and_unblocks_close() {
    let engine = AsyncEngine::spawn(12, 3).unwrap();
    let model = engine.model();
    assert!(block_on(engine.request(Command::Panic).unwrap().wait()).is_err());
    assert!(matches!(block_on(engine.close()), Err(Error::Panicked)));
    assert!(matches!(
        model.reader().poll().unwrap().state,
        State::Failed(Error::Panicked)
    ));
}
