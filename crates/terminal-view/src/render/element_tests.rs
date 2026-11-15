//! End-to-end tests of `TerminalElement` on a headless window driven by
//! `FakeTerminalSession`, plus the module-boundary check for `render/`.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use gpui::{
    AppContext as _, Context, Entity, Font, FontFeatures, FontStyle, FontWeight, IntoElement,
    Render, TestAppContext, VisualTestContext, Window, px, size,
};
use oneterm_terminal::TerminalSession;
use oneterm_terminal::test_support::{FakeSessionProbe, FakeTerminalSession};

use super::{CellAnchor, TerminalElement, TerminalElementSpec};
use crate::render::diagnostics::FrameStats;
use crate::render::frame::GridSize;
use crate::render::state::{RenderInputs, RenderState};
use crate::theme::build_terminal_theme;

/// Run text `=>e\u{301}` over three cells: the `=>` ligature is one glyph for
/// bytes 0..2 (cells 0 and 1), `e` starts cell 2 at byte 2, the mark (byte 3)
/// shares its cell. GPUI's `force_width` pass put `e` at 8 px (glyph number 1)
/// instead of 16 px.
#[test]
fn glyphs_are_anchored_at_their_cell() {
    let cells = [0u32, 1, 2];
    let w = px(8.0);
    let mut anchor = CellAnchor::default();
    assert_eq!(anchor.x(&cells, w, 0, px(0.0)), px(0.0));
    assert_eq!(
        anchor.x(&cells, w, 2, px(8.0)),
        px(16.0),
        "`e` moves to cell 2"
    );
    assert_eq!(
        anchor.x(&cells, w, 3, px(10.0)),
        px(18.0),
        "the mark keeps its 2 px offset from `e`"
    );
    // No cell map (gutter labels): the shaped position is used as-is.
    assert_eq!(CellAnchor::default().x(&[], w, 5, px(40.0)), px(40.0));
}

/// Counts heap allocations on the current thread between `start` and `stop`.
/// The counters are `const`-initialised, `Drop`-free thread locals, so reading
/// them inside the allocator never allocates or registers a destructor.
mod alloc_counter {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static ENABLED: Cell<bool> = const { Cell::new(false) };
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    struct Counting;

    fn record() {
        if ENABLED.with(|e| e.get()) {
            COUNT.with(|c| c.set(c.get() + 1));
        }
    }

    // SAFETY: every method forwards to `System` with the same arguments, so
    // the allocator contract (layout/pointer pairing) is exactly `System`'s;
    // the counters are plain `Cell`s that cannot allocate or re-enter.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            record();
            unsafe { System.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            record();
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            record();
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }

    #[global_allocator]
    static GLOBAL: Counting = Counting;

    pub(super) fn start() {
        COUNT.with(|c| c.set(0));
        ENABLED.with(|e| e.set(true));
    }

    pub(super) fn stop() -> usize {
        ENABLED.with(|e| e.set(false));
        COUNT.with(|c| c.get())
    }
}

struct Host {
    session: Entity<Box<dyn TerminalSession>>,
    state: Rc<RefCell<RenderState>>,
}

#[cfg(feature = "ghostty-native")]
mod native {
    use super::*;
    use crate::render::frame::{CursorShape, Frame};
    use oneterm_terminal::ghostty_bridge::{AsyncEngine, Command, Reply};
    use std::future::Future;
    use std::sync::Arc;
    use std::task::{Context as TaskContext, Poll, Wake, Waker};

    struct ThreadWake(std::thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    fn complete<F: Future>(future: F) -> F::Output {
        let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
        let mut cx = TaskContext::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::park(),
            }
        }
    }
    fn call(engine: &AsyncEngine, command: Command) -> Reply {
        complete(engine.request(command).unwrap().wait()).unwrap()
    }

    #[gpui::test]
    fn native_semantic_directory_colors_match_legacy(cx: &mut TestAppContext) {
        use oneterm_highlight::ShellProfile;
        let text = "d-r---    2026/8/27    21:45    17 .ash_history";
        let configured = || {
            let mut value = inputs_without_cursor();
            value.semantic.set_enabled(true);
            value.semantic.set_profile(ShellProfile::PowerShell);
            value
        };
        let painted = |h: &Harness<'_>| {
            let state = h.state.borrow();
            let plan = &state.plans.rows()[0];
            (0..text.len())
                .filter(|&col| text.as_bytes()[col].is_ascii_alphanumeric())
                .map(|col| {
                    let run = plan
                        .text
                        .iter()
                        .find(|run| {
                            usize::from(run.col) <= col && col < usize::from(run.col + run.cols)
                        })
                        .expect("text glyph run");
                    let offset = col - usize::from(run.col);
                    plan.colors_of(run)
                        .iter()
                        .find(|span| span.byte_end > offset as u32)
                        .unwrap()
                        .color
                })
                .collect::<Vec<_>>()
        };
        let expected = {
            let mut h = Harness::open(cx, 3, 80, text, configured());
            h.draw();
            painted(&h)
        };
        assert!(expected.windows(2).any(|pair| pair[0] != pair[1]));
        let engine = AsyncEngine::spawn(80, 3).unwrap();
        let palette = configured().theme.palette;
        let native_rgb = |color: oneterm_terminal::Rgb| oneterm_terminal::ghostty::render::Rgb {
            r: color.r,
            g: color.g,
            b: color.b,
        };
        call(
            &engine,
            Command::DefaultColors(Box::new(oneterm_terminal::ghostty::colors::DefaultColors {
                foreground: native_rgb(palette.foreground),
                background: native_rgb(palette.background),
                cursor: None,
                palette: std::array::from_fn(|index| {
                    native_rgb(oneterm_terminal::resolve_color(
                        &oneterm_terminal::Color::Palette(index as u8),
                        &palette,
                    ))
                }),
            })),
        );
        call(&engine, Command::Feed(text.as_bytes().to_vec()));
        call(&engine, Command::Capture);
        let (session, probe) = FakeTerminalSession::new(3, 80, "MUST NOT PAINT");
        let session = session.with_native_render_model(engine.model());
        let mut h = Harness::open_session(cx, Box::new(session), probe, configured());
        h.draw();
        assert_eq!(painted(&h), expected);
        h.state.borrow_mut().inputs.semantic.set_enabled(false);
        h.draw();
        assert_ne!(painted(&h), expected, "Off must invalidate semantic plans");
        h.state.borrow_mut().inputs.semantic.set_enabled(true);
        h.draw();
        assert_eq!(painted(&h), expected);
        for sgr in ["\x1b[31m", "\x1b[38;2;122;122;122m", "\x1b[38;5;16m"] {
            call(
                &engine,
                Command::Feed(format!("\r{sgr}{text}\x1b[0m").into_bytes()),
            );
            call(&engine, Command::Capture);
            h.draw();
            let explicit = painted(&h);
            h.state.borrow_mut().inputs.semantic.set_enabled(false);
            h.draw();
            assert_eq!(painted(&h), explicit, "program color preserved: {sgr:?}");
            h.state.borrow_mut().inputs.semantic.set_enabled(true);
        }
        assert_eq!(h.probe.snapshot_calls(), 0);
        complete(engine.close()).unwrap();
    }

    #[gpui::test]
    fn native_gpui_theme_colors_apply_shared_contrast_without_changing_exact_rgb(
        cx: &mut TestAppContext,
    ) {
        use crate::render::frame::Color;
        use oneterm_terminal::ghostty::{colors::DefaultColors, render::Rgb};
        let engine = AsyncEngine::spawn(16, 3).unwrap();
        let low = Rgb {
            r: 36,
            g: 37,
            b: 38,
        };
        let background = Rgb {
            r: 24,
            g: 25,
            b: 26,
        };
        let defaults = DefaultColors {
            foreground: low,
            background,
            cursor: None,
            palette: [low; 256],
        };
        call(&engine, Command::DefaultColors(Box::new(defaults.clone())));
        call(
            &engine,
            Command::Feed(b"D\x1b[31mA\x1b[38;2;36;37;38mT\x1b[38;5;16mE\x1b[0m".to_vec()),
        );
        call(&engine, Command::Capture);
        let (session, probe) = FakeTerminalSession::new(3, 16, "MUST NOT PAINT");
        let session = session.with_native_render_model(engine.model());
        let mut h = Harness::open_session(cx, Box::new(session), probe, inputs());
        h.draw();
        let check = |h: &Harness<'_>, fg: Color, bg: Color| {
            let state = h.state.borrow();
            let theme = &state.inputs.theme;
            let exact = theme.color(Color::Rgb(36, 37, 38));
            let adjusted = theme.ensure_contrast(theme.color(fg), theme.color(bg));
            let colors = &state.plans.rows()[0].colors;
            for (column, expected) in [adjusted, adjusted, exact, exact].into_iter().enumerate() {
                let painted = colors
                    .iter()
                    .find(|span| span.byte_end > column as u32)
                    .unwrap();
                assert_eq!(painted.color, expected, "painted column {column}");
            }
            assert_eq!(state.frame.background(), Some(bg));
        };
        check(&h, Color::Rgb(36, 37, 38), Color::Rgb(24, 25, 26));
        let mut light = defaults;
        light.background = Rgb {
            r: 230,
            g: 231,
            b: 232,
        };
        call(&engine, Command::DefaultColors(Box::new(light)));
        call(&engine, Command::Capture);
        h.draw();
        check(&h, Color::Rgb(36, 37, 38), Color::Rgb(230, 231, 232));
        call(
            &engine,
            Command::Feed(b"\x1b]10;#242526\x07\x1b]11;#18191a\x07\x1b]4;1;#242526\x07".to_vec()),
        );
        call(&engine, Command::Capture);
        h.draw();
        check(&h, Color::Rgb(36, 37, 38), Color::Rgb(24, 25, 26));
        call(
            &engine,
            Command::Feed(b"\x1b]110\x07\x1b]111\x07\x1b]104;1\x07".to_vec()),
        );
        call(&engine, Command::Capture);
        h.draw();
        check(&h, Color::Rgb(36, 37, 38), Color::Rgb(230, 231, 232));
        assert_eq!(h.probe.snapshot_calls(), 0);
        complete(engine.close()).unwrap();
    }

    #[gpui::test]
    fn native_gpui_prepaint_plans_cursor_selection_and_resize(cx: &mut TestAppContext) {
        use oneterm_terminal::SnapshotUpdate;
        use oneterm_terminal::ghostty::{
            anchors::{Coordinate, Point, Space},
            state::Scroll,
        };
        let engine = AsyncEngine::spawn(16, 3).unwrap();
        use oneterm_terminal::ghostty::{colors::DefaultColors, render::Rgb};
        call(
            &engine,
            Command::DefaultColors(Box::new(DefaultColors {
                foreground: Rgb {
                    r: 210,
                    g: 211,
                    b: 212,
                },
                background: Rgb {
                    r: 12,
                    g: 13,
                    b: 14,
                },
                cursor: Some(Rgb {
                    r: 90,
                    g: 91,
                    b: 92,
                }),
                palette: [Rgb {
                    r: 200,
                    g: 201,
                    b: 202,
                }; 256],
            })),
        );
        call(&engine, Command::Feed(b"abcdef\r\nghijkl\x1b[2 q".to_vec()));
        call(&engine, Command::Capture);
        let model = engine.model();
        let (session, probe) = FakeTerminalSession::new(3, 16, "LEGACY MUST NOT PAINT");
        let session = session.with_native_render_model(model.clone());
        let mut independent = Frame::new();
        independent.snapshot(&session);
        assert_eq!(independent.update(), SnapshotUpdate::Full);
        let mut h = Harness::open_session(cx, Box::new(session), probe, inputs());
        h.draw();
        {
            let state = h.state.borrow();
            assert_eq!(state.frame.size(), GridSize { rows: 3, cols: 16 });
            assert_eq!(state.frame.row(0).cell(0).ch, 'a');
            assert_eq!(state.plans.rows().len(), 3);
            assert_eq!(state.frame.cursor().shape, CursorShape::Block);
            assert!(!state.frame.cursor().blinking);
            let blink_off = crate::render::cursor::CursorConfig {
                shape: None,
                color: None,
                focused: true,
                blink_visible: false,
            };
            assert!(
                crate::render::cursor::resolve(&state.frame, &blink_off).is_some(),
                "a steady native cursor ignores the application blink-off phase"
            );
            assert_eq!((state.frame.cursor().row, state.frame.cursor().col), (1, 6));
            assert!(state.frame.modes().is_none());
            assert_eq!(state.frame.display_offset(), None);
            assert_eq!(
                state.frame.background(),
                Some(crate::render::frame::Color::Rgb(12, 13, 14))
            );
        }
        h.cx.update(|window, _| {
            let mut state = h.state.borrow_mut();
            let geometry = state.geometry.unwrap();
            let cursor = state.resolve_cursor(&geometry, window).unwrap();
            assert_eq!(
                cursor.color,
                state
                    .inputs
                    .theme
                    .color(crate::render::frame::Color::Rgb(90, 91, 92))
            );
        });
        assert_eq!(
            h.probe.snapshot_calls(),
            0,
            "native prepaint must not take a legacy snapshot"
        );
        let stats = h.draw();
        assert_eq!(stats.frames_unchanged, 1);
        assert_eq!(stats.rows_planned, 0);
        call(&engine, Command::Feed(b"\rZ".to_vec()));
        call(&engine, Command::Capture);
        let stats = h.draw();
        assert_eq!(stats.rows_planned, 1);
        assert_eq!(h.state.borrow().frame.row(1).cell(0).ch, 'Z');
        h.host.read_with(h.cx, |host, cx| {
            independent.snapshot(&**host.session.read(cx))
        });
        assert_eq!(
            independent.update(),
            SnapshotUpdate::Partial { scrolled: 0 }
        );
        assert_eq!(independent.row(1).cell(0).ch, 'Z');

        let point = |column| Point {
            space: Space::Active,
            coordinate: Coordinate { column, row: 0 },
        };
        let start = complete(model.create_anchor(point(1))).unwrap();
        let end = complete(model.create_anchor(point(3))).unwrap();
        complete(model.set_selection(&start, &end, false)).unwrap();
        call(&engine, Command::Capture);
        let stats = h.draw();
        assert_eq!(
            stats.rows_planned, 0,
            "selection does not rebuild glyph plans"
        );
        assert_eq!(
            h.state.borrow().overlays.selection,
            [crate::render::overlay::RowSpan {
                row: 0,
                col: 1,
                cols: 3
            }]
        );
        complete(model.clear_selection()).unwrap();
        call(&engine, Command::Capture);
        h.draw();
        assert!(h.state.borrow().overlays.selection.is_empty());

        call(
            &engine,
            Command::Resize {
                cols: 8,
                rows: 2,
                cell_width_px: 8,
                cell_height_px: 16,
            },
        );
        call(&engine, Command::Feed(b"\r\n1\r\n2\r\n3\r\n4".to_vec()));
        call(&engine, Command::Scroll(Scroll::Top));
        call(&engine, Command::Capture);
        h.draw();
        {
            let state = h.state.borrow();
            assert_eq!(state.frame.size(), GridSize { rows: 2, cols: 8 });
            assert_eq!(state.plans.rows().len(), 2);
            assert_eq!(state.frame.cursor().shape, CursorShape::Hidden);
        }
        complete(engine.close()).unwrap();
        let stats = h.draw();
        assert_eq!(stats.frames_unchanged, 1);
        assert_eq!(h.probe.snapshot_calls(), 0);
    }

    #[gpui::test]
    fn native_gpui_absent_frame_and_failed_owner_never_fall_back(cx: &mut TestAppContext) {
        let engine = AsyncEngine::spawn(8, 2).unwrap();
        let (session, probe) = FakeTerminalSession::new(2, 8, "WRONG");
        let session = session.with_native_render_model(engine.model());
        let mut h = Harness::open_session(cx, Box::new(session), probe, inputs());
        h.draw();
        assert_eq!(h.state.borrow().frame.size(), GridSize { rows: 0, cols: 0 });
        assert!(h.state.borrow().plans.rows().is_empty());
        assert_eq!(h.probe.snapshot_calls(), 0);
        call(&engine, Command::Feed(b"good\x1b[?5h".to_vec()));
        call(&engine, Command::Capture);
        let stats = h.draw();
        assert_eq!(stats.rows_planned, 2);
        assert!(h.state.borrow().frame.reverse_video());
        let key = h.state.borrow().frame.row_key(0);
        assert!(
            complete(
                engine
                    .request(Command::Feed(b"\x1b[6n".repeat(1000)))
                    .unwrap()
                    .wait()
            )
            .is_err()
        );
        assert!(complete(engine.close()).is_err());
        h.draw();
        assert_eq!(h.state.borrow().frame.row_key(0), key);
        assert_eq!(h.state.borrow().frame.row(0).cell(0).ch, 'g');
        assert_eq!(h.probe.snapshot_calls(), 0);
    }

    #[test]
    fn native_frame_source_switch_resets_reader_and_legacy_plan_damage() {
        use oneterm_terminal::SnapshotUpdate;
        let first = AsyncEngine::spawn(8, 2).unwrap();
        let second = AsyncEngine::spawn(8, 2).unwrap();
        call(&first, Command::Feed(b"first".to_vec()));
        call(&first, Command::Capture);
        call(&second, Command::Feed(b"second".to_vec()));
        call(&second, Command::Capture);
        let (legacy, probe) = FakeTerminalSession::new(2, 8, "legacy");
        let (a, _) = FakeTerminalSession::new(2, 8, "unused");
        let a = a.with_native_render_model(first.model());
        let (b, _) = FakeTerminalSession::new(2, 8, "unused");
        let b = b.with_native_render_model(second.model());
        let mut frame = Frame::new();
        frame.snapshot(&legacy);
        frame.snapshot(&legacy);
        assert_eq!(frame.update(), SnapshotUpdate::Unchanged);
        frame.snapshot(&a);
        assert_eq!(frame.update(), SnapshotUpdate::Full);
        assert_eq!(frame.row(0).cell(0).ch, 'f');
        frame.snapshot(&a);
        assert_eq!(frame.update(), SnapshotUpdate::Unchanged);
        frame.snapshot(&b);
        assert_eq!(frame.update(), SnapshotUpdate::Full);
        assert_eq!(frame.row(0).cell(0).ch, 's');
        frame.snapshot(&legacy);
        assert_eq!(
            frame.update(),
            SnapshotUpdate::Full,
            "native plans must not survive an unchanged legacy snapshot"
        );
        assert_eq!(frame.row(0).cell(0).ch, 'l');
        assert!(frame.modes().is_some());
        assert_eq!(probe.snapshot_calls(), 3);
        complete(first.close()).unwrap();
        complete(second.close()).unwrap();
    }
}

impl Render for Host {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        TerminalElement::new(TerminalElementSpec {
            id: "terminal".into(),
            session: super::ElementSession::Legacy(self.session.clone()),
            state: self.state.clone(),
            ime: None,
        })
    }
}

fn font() -> Font {
    Font {
        family: "Test Mono".into(),
        features: FontFeatures::default(),
        fallbacks: None,
        weight: FontWeight::NORMAL,
        style: FontStyle::Normal,
    }
}

fn inputs() -> RenderInputs {
    let theme = Rc::new(build_terminal_theme(&gpui_component::Theme::default()));
    RenderInputs::new(theme, font(), px(13.0))
}

/// A focused terminal in its blink-off phase paints no cursor, which keeps
/// the layer count at one for the grid assertions.
fn inputs_without_cursor() -> RenderInputs {
    let mut inputs = inputs();
    inputs.cursor.focused = true;
    inputs.cursor.blink_visible = false;
    inputs
}

struct Harness<'a> {
    host: Entity<Host>,
    probe: FakeSessionProbe,
    state: Rc<RefCell<RenderState>>,
    cx: &'a mut VisualTestContext,
}

impl<'a> Harness<'a> {
    fn open(
        cx: &'a mut TestAppContext,
        rows: usize,
        cols: usize,
        text: &str,
        inputs: RenderInputs,
    ) -> Harness<'a> {
        let (session, probe) = FakeTerminalSession::boxed(rows, cols, text);
        Self::open_session(cx, session, probe, inputs)
    }

    fn open_session(
        cx: &'a mut TestAppContext,
        session: Box<dyn TerminalSession>,
        probe: FakeSessionProbe,
        inputs: RenderInputs,
    ) -> Harness<'a> {
        let state = Rc::new(RefCell::new(RenderState::new(inputs)));
        let shared = state.clone();
        let (host, cx) = cx.add_window_view(move |_, cx| Host {
            session: cx.new(|_| session),
            state: shared,
        });
        Harness {
            host,
            probe,
            state,
            cx,
        }
    }

    /// Stats of the first frame: opening the window already draws once; a
    /// platform that does not is drawn explicitly.
    fn first_frame(&mut self) -> FrameStats {
        if self.probe.snapshot_calls() == 0 {
            return self.draw();
        }
        self.state.borrow().stats
    }

    fn draw(&mut self) -> FrameStats {
        self.cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        self.state.borrow().stats
    }

    /// Resize the window and return the stats of the frame that follows.
    fn resize(&mut self, width: f32, height: f32) -> FrameStats {
        let before = self.probe.snapshot_calls();
        self.cx.simulate_resize(size(px(width), px(height)));
        if self.probe.snapshot_calls() == before {
            return self.draw();
        }
        self.state.borrow().stats
    }

    fn session_state(&mut self) -> oneterm_terminal::TerminalQueryState {
        self.host
            .read_with(self.cx, |host, cx| host.session.read(cx).query_state())
    }

    /// The device-pixel cell the painter used this frame -- the one value the
    /// session is told about.
    fn cell_device(&self) -> crate::render::shapes::CellSizeDevicePx {
        self.state
            .borrow()
            .geometry
            .expect("a frame was drawn")
            .metrics
            .device
    }

    fn grid(&self) -> GridSize {
        self.state
            .borrow()
            .geometry
            .map(|g| g.size)
            .unwrap_or(GridSize { rows: 0, cols: 0 })
    }
}

/// Frame time and the plan counters under sustained output, off a real GPUI
/// draw in a headless window.
///
/// `US-0085`'s stand-in for "frame time under `yes` and a 10 MB `type`" from the
/// running app's diagnostics log: the desktop this packet was implemented on was
/// disconnected, so no window could be presented or captured. Everything the
/// packet changed is in here — the frame is built from the snapshot state and the
/// plans are keyed on `(RowId, SeqNo)` — and the counters printed are the same
/// `FrameStats` the app's log line prints. Run it explicitly:
///
/// ```text
/// cargo test -p oneterm-terminal-view --profile fast-dev frame_time_under_output
///     -- --ignored --nocapture
/// ```
#[gpui::test]
#[ignore = "measurement, run explicitly"]
fn frame_time_under_output(cx: &mut TestAppContext) {
    // With `--features hotpath-profiling,hotpath/hotpath` the sites measure and the
    // report prints when the guard drops (`IN-0046`: the profiler's overhead is this
    // test's numbers with and without the features).
    #[cfg(feature = "hotpath-profiling")]
    let _hotpath = hotpath::HotpathGuardBuilder::new("frame_time_under_output").build();
    let mut h = Harness::open(cx, 30, 120, "", inputs());
    h.first_frame();

    let line =
        "the quick brown fox jumps over the lazy dog 0123456789 \u{1b}[33mcolour\u{1b}[0m tail\r\n";
    let mut frames = Vec::new();
    let mut planned = 0u64;
    let mut total = 0u64;
    let mut unchanged = 0u32;
    // A megabyte of scrolling output in 4 KiB chunks, one drawn frame per chunk.
    let mut payload = String::new();
    while payload.len() < 1024 * 1024 {
        payload.push_str(line);
    }
    let bytes = payload.into_bytes();
    for chunk in bytes.chunks(4096) {
        h.probe.feed(chunk);
        let stats = h.draw();
        frames.push(u64::from(stats.prepaint_us + stats.paint_us));
        planned += u64::from(stats.rows_planned);
        total += u64::from(stats.rows_total);
    }
    let flood: Vec<u64> = frames.clone();

    // Then sixty idle frames: nothing changed, so the tri-state should cost
    // nothing at all.
    frames.clear();
    for _ in 0..60 {
        let stats = h.draw();
        frames.push(u64::from(stats.prepaint_us + stats.paint_us));
        unchanged += stats.frames_unchanged;
        assert_eq!(stats.rows_planned, 0, "an idle frame plans nothing");
        assert_eq!(stats.rows_candidate, 0, "and considers nothing");
    }
    assert_eq!(unchanged, 60, "every idle frame was reported Unchanged");

    let report = |name: &str, mut samples: Vec<u64>| {
        samples.sort_unstable();
        let n = samples.len();
        let sum: u64 = samples.iter().sum();
        println!(
            "  {name}: n={n} total={} ms avg={} us p50={} us p95={} us max={} us",
            sum / 1000,
            sum / n as u64,
            samples[n / 2],
            samples[(n * 95) / 100],
            samples[n - 1],
        );
    };
    println!(
        "frame time, 1 MiB of scrolling output in 4 KiB chunks, debug_assertions={}",
        cfg!(debug_assertions)
    );
    report("flood (prepaint+paint)", flood.clone());
    report("idle  (prepaint+paint)", frames);
    println!(
        "  rows planned over {} flood frames: {planned} of {total} viewport rows",
        flood.len(),
    );
}

#[gpui::test]
fn dirty_frame_plans_rows_and_shapes(cx: &mut TestAppContext) {
    let mut h = Harness::open(
        cx,
        24,
        80,
        "hello world\n████████████\n────────────\nhttps://example.com/x",
        inputs_without_cursor(),
    );
    let first = h.first_frame();
    assert_eq!(first.snapshot_calls, 1, "{first:?}");
    assert!(first.rows_planned > 0, "{first:?}");
    assert!(first.rows_planned <= first.rows_candidate, "{first:?}");
    assert!(first.shape_calls > 0, "{first:?}");
    assert!(first.glyphs > 0, "{first:?}");
    assert!(first.quads > 0, "{first:?}");
    assert_eq!(first.layers, 1, "{first:?}");
    assert_eq!(first.url_scans, 1, "{first:?}");
    assert_eq!(first.glyph_errors, 0, "{first:?}");

    // Only the rows whose content changed are rebuilt.
    h.probe.set_text("hello there");
    let dirty = h.draw();
    assert_eq!(dirty.snapshot_calls, 1, "{dirty:?}");
    assert!(dirty.rows_planned >= 1, "{dirty:?}");
    assert!(dirty.rows_planned < dirty.rows_total, "{dirty:?}");
    assert!(dirty.rows_planned <= dirty.rows_candidate, "{dirty:?}");
    eprintln!("dirty_frame_plans_rows_and_shapes first={first:?} dirty={dirty:?}");
}

#[gpui::test]
fn idle_frame_plans_nothing_but_paints(cx: &mut TestAppContext) {
    let mut h = Harness::open(
        cx,
        24,
        80,
        "idle ████ text\n──────",
        inputs_without_cursor(),
    );
    h.first_frame();
    let idle = h.draw();
    assert_eq!(idle.snapshot_calls, 1, "{idle:?}");
    assert_eq!(idle.rows_planned, 0, "{idle:?}");
    assert_eq!(idle.shape_calls, 0, "{idle:?}");
    assert_eq!(idle.url_scans, 0, "{idle:?}");
    assert!(idle.quads > 0, "{idle:?}");
    assert!(idle.glyphs > 0, "{idle:?}");
    assert_eq!(idle.layers, 1, "{idle:?}");
    eprintln!("idle_frame_plans_nothing_but_paints idle={idle:?}");
}

#[gpui::test]
fn idle_frame_allocates_nothing(cx: &mut TestAppContext) {
    let mut h = Harness::open(
        cx,
        24,
        80,
        "steady state\n████ ──── ╭──╮\nhttps://example.com/path",
        inputs(),
    );
    h.first_frame();
    h.draw();
    let session = h.host.read_with(h.cx, |host, _| host.session.clone());
    let state = h.state.clone();
    h.cx.update(|window, cx| {
        let state = &mut *state.borrow_mut();
        // The fake's `snapshot_into` allocates a fresh content; that is the
        // engine's side, so it stays outside the measured region.
        state.frame.snapshot(&**session.read(cx));
        let metrics = state.metrics(window, cx);
        let geometry = state.geometry.expect("drawn");
        state.glyphs.begin_frame();
        state.stats = FrameStats::default();
        // The counter must see a deliberate allocation before it is trusted
        // to report zero.
        alloc_counter::start();
        let probe = vec![0u8; 64];
        assert!(alloc_counter::stop() > 0, "allocation counter is inert");
        drop(probe);
        alloc_counter::start();
        state.update_plans(&metrics, window);
        state.compute_overlays();
        let cursor = state.resolve_cursor(&geometry, window);
        let allocations = alloc_counter::stop();
        assert!(
            cursor.is_some(),
            "unfocused terminals paint a hollow cursor"
        );
        assert_eq!(state.stats.rows_planned, 0, "{:?}", state.stats);
        assert_eq!(
            allocations, 0,
            "idle plan update must not allocate ({:?})",
            state.stats
        );
    });
}

#[gpui::test]
fn glyph_cache_hits_across_rows(cx: &mut TestAppContext) {
    let mut h = Harness::open(
        cx,
        24,
        80,
        "same text\nsame text\nsame text",
        inputs_without_cursor(),
    );
    let first = h.first_frame();
    assert_eq!(
        first.shape_calls, 2,
        "'same' and 'text' shape once: {first:?}"
    );
    assert!(first.glyph_hits >= 4, "{first:?}");
    let idle = h.draw();
    assert_eq!(idle.shape_calls, 0, "{idle:?}");
    assert!(h.state.borrow().glyphs.len() >= 2);
}

#[gpui::test]
fn resize_replans_all_and_resizes_session(cx: &mut TestAppContext) {
    let mut h = Harness::open(cx, 24, 80, "resize me", inputs_without_cursor());
    let first = h.first_frame();
    let grid = h.grid();
    assert_ne!(
        grid,
        GridSize { rows: 24, cols: 80 },
        "window differs from the fake's size"
    );
    let state = h.session_state();
    assert_eq!(
        (state.rows, state.cols),
        (usize::from(grid.rows), usize::from(grid.cols))
    );
    assert_eq!(first.rows_total, u32::from(grid.rows));

    let resized = h.resize(400.0, 300.0);
    let smaller = h.grid();
    assert!(
        smaller.rows < grid.rows && smaller.cols < grid.cols,
        "{smaller:?}"
    );
    assert_eq!(resized.rows_planned, u32::from(smaller.rows), "{resized:?}");
    let state = h.session_state();
    assert_eq!(
        (state.rows, state.cols),
        (usize::from(smaller.rows), usize::from(smaller.cols))
    );
}

/// `BUG-0061`: the engine starts with a zero cell size, so `CSI 14 t` told
/// every program the window had no pixels and the ones that gate image support
/// on it (OpenTUI, chafa) drew block mosaics instead of Sixels. The view is the
/// only place that knows the cell, and this is the path that hands it over.
#[gpui::test]
fn cell_pixels_reach_the_session_so_csi_14_t_answers(cx: &mut TestAppContext) {
    let mut h = Harness::open(cx, 24, 80, "", inputs_without_cursor());
    h.first_frame();

    let grid = h.grid();
    let cell = h.cell_device();
    assert!(cell.w > 0 && cell.h > 0, "{cell:?}");
    assert_eq!(
        h.probe.feed_replies(b"\x1b[14t"),
        format!(
            "\x1b[4;{};{}t",
            u32::from(grid.rows) * cell.h as u32,
            u32::from(grid.cols) * cell.w as u32
        ),
        "the pixel size is the view's own grid times its own device cell"
    );
    assert_eq!(
        h.probe.feed_replies(b"\x1b[18t"),
        format!("\x1b[8;{};{}t", grid.rows, grid.cols),
        "the neighbouring arm still answers in cells"
    );

    // A metrics change alone updates it. This is the DPI-scale path's
    // mechanism -- both go through the one `MetricsKey` cache -- driven here by
    // the font size, because the GPUI test window pins `scale_factor` at 2.0.
    h.state.borrow_mut().inputs.font_size = px(20.0);
    h.draw();
    let grown = h.cell_device();
    assert!(grown.h > cell.h, "{cell:?} -> {grown:?}");
    let grid = h.grid();
    assert_eq!(
        h.probe.feed_replies(b"\x1b[14t"),
        format!(
            "\x1b[4;{};{}t",
            u32::from(grid.rows) * grown.h as u32,
            u32::from(grid.cols) * grown.w as u32
        )
    );
}

/// BUG-0061 verification (independent, not the implementer's test): ONE frame.
/// The very first prepaint must already have pushed the cell size, or a program
/// that queries right after spawn is still told the window has no pixels.
#[gpui::test]
fn verify_first_prepaint_alone_makes_csi_14_t_non_zero(cx: &mut TestAppContext) {
    let mut h = Harness::open(cx, 24, 80, "", inputs_without_cursor());
    if h.probe.snapshot_calls() == 0 {
        h.draw();
    }
    assert_eq!(h.probe.snapshot_calls(), 1, "exactly one frame was drawn");

    let reply = h.probe.feed_replies(b"\x1b[14t");
    let nums: Vec<u32> = reply
        .trim_start_matches('\u{1b}')
        .trim_start_matches("[4;")
        .trim_end_matches('t')
        .split(';')
        .map(|n| n.parse().expect("numeric reply"))
        .collect();
    assert_eq!(nums.len(), 2, "{reply:?}");
    assert!(nums[0] > 0 && nums[1] > 0, "{reply:?}");

    let grid = h.grid();
    let cell = h.cell_device();
    assert_eq!(nums[0], u32::from(grid.rows) * cell.h as u32);
    assert_eq!(nums[1], u32::from(grid.cols) * cell.w as u32);
}

#[gpui::test]
fn cursor_layer_and_gutter_paint(cx: &mut TestAppContext) {
    let mut inputs = inputs();
    inputs.cursor.focused = true;
    inputs.cursor.blink_visible = true;
    inputs.show_gutter = true;
    inputs.gutter.absolute_line_count = 3;
    inputs.gutter.times = Rc::new([36_000u32, 36_001, 36_002].into_iter().collect());
    let mut h = Harness::open(cx, 24, 80, "x", inputs);
    h.probe.set_cursor(0, 0);
    h.first_frame();
    let s = h.draw();
    assert_eq!(s.layers, 2, "grid + cursor: {s:?}");
    let state = h.state.borrow();
    let geometry = state.geometry.expect("drawn");
    assert!(geometry.gutter_width > px(0.0));
    assert_eq!(state.gutter.labels.len(), usize::from(geometry.size.rows));
    assert!(geometry.origin.x > geometry.bounds.origin.x + geometry.gutter_width - px(1.0));
}

/// `frame.rs` is the only file under `render/` allowed to name the engine's own
/// vocabulary; everything above it speaks the view's `Cell` / `Color` /
/// `CellFlags`. The needle changed at `US-0085` with the crate the view reads.
#[test]
fn render_has_single_engine_file() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/render");
    // The engine's *cell* vocabulary. `SnapshotUpdate` is deliberately not in the
    // list: the tri-state is the contract the plan cache is written against.
    // Spelled in halves so this file does not match its own list.
    let needles = [
        concat!("Render", "Row"),
        concat!("Render", "Cell"),
        concat!("Render", "Content"),
        concat!("Named", "Color"),
        concat!("Cell", "Width"),
    ];
    let mut offenders = Vec::new();
    let entries = std::fs::read_dir(&dir).expect("render dir");
    for entry in entries {
        let path = entry.expect("dir entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if !name.ends_with(".rs") || name == "frame.rs" {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("read source");
        if needles.iter().any(|needle| source.contains(needle)) {
            offenders.push(name.to_string());
        }
    }
    assert!(
        offenders.is_empty(),
        "only frame.rs may name the engine's cell vocabulary: {offenders:?}"
    );
}

/// A real Sixel image: uploaded once, painted once per frame from the first
/// cell that still references it, gone when the screen is cleared.
///
/// `US-0085` fed it as a real DCS sequence instead of fabricating a
/// `GraphicCell` per covered cell: the engine owns the decode, the placement and
/// the release signal, and the painter derives the per-cell offset from the
/// placement (R-21).
#[gpui::test]
fn sixel_image_paints_once_per_frame(cx: &mut TestAppContext) {
    let mut h = Harness::open(cx, 6, 12, "", inputs_without_cursor());
    let _ = h.first_frame();
    // Two bands of two columns: 2 x 12 pixels. The element pushes the real
    // device cell in prepaint before this is fed, so the placement is sized by
    // that cell, not by the VT340 one.
    h.probe.push_sixel(None);
    let stats = h.draw();
    assert_eq!((stats.images_uploaded, stats.images), (1, 1), "{stats:?}");
    let stats = h.draw();
    assert_eq!(
        (stats.images_uploaded, stats.images),
        (0, 1),
        "known id is not re-uploaded: {stats:?}"
    );
    // `cls`: the cells lose their reference, the engine releases the image.
    h.probe.clear_graphics();
    let stats = h.draw();
    assert_eq!(stats.images, 0, "{stats:?}");
}
