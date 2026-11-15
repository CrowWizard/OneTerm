use super::*;
use crate::ghostty::{Terminal, render::RenderBuffer};

fn frame() -> render::Frame {
    RenderBuffer::new()
        .unwrap()
        .capture(&mut Terminal::new(12, 3).unwrap())
        .unwrap()
}

#[test]
fn equal_effective_rgb_retains_distinct_foreground_intent_and_cache_keys() {
    let mut native = frame();
    let low = render::Rgb {
        r: 36,
        g: 37,
        b: 38,
    };
    native.foreground = low;
    native.palette.fill(low);
    for (column, color) in [
        render::Color::Default,
        render::Color::Palette(1),
        render::Color::Rgb(low),
        render::Color::Palette(16),
    ]
    .into_iter()
    .enumerate()
    {
        native.rows[0].cells[column].style.foreground = color;
    }
    let first = ProjectedFrame::new(Arc::new(native.clone())).unwrap();
    let row = first.row(0).unwrap();
    for (column, exact) in [false, false, true, true].into_iter().enumerate() {
        assert_eq!(row.native_foreground_is_exact(column), Some(exact));
        assert_eq!(row.native_foreground_is_default(column), Some(column == 0));
        assert_eq!(
            row.style_of(&row.cells()[column]).fg,
            Color::Rgb(Rgb {
                r: 36,
                g: 37,
                b: 38
            })
        );
    }
    let mut ansi = native.clone();
    ansi.rows[0].cells[0].style.foreground = render::Color::Palette(1);
    let ansi = ProjectedFrame::new(Arc::new(ansi)).unwrap();
    assert_ne!(
        row.key(),
        ansi.row(0).unwrap().key(),
        "equal-RGB default to ANSI changes semantic eligibility"
    );
    native.rows[0].cells[0].style.foreground = render::Color::Rgb(low);
    let changed = ProjectedFrame::new(Arc::new(native)).unwrap();
    assert_ne!(
        row.key(),
        changed.row(0).unwrap().key(),
        "intent changes invalidate equal-RGB row plans"
    );
}

#[test]
fn selection_masks_are_atomic_and_do_not_change_paint_keys() {
    use crate::ghostty::anchors::{Coordinate, Point, Space};
    use crate::ghostty::owner::EngineOwner;
    let mut owner = EngineOwner::new(12, 3).unwrap();
    owner.feed(b"abcdef\r\nghijkl").unwrap();
    let mut reader = ProjectedReader::new(owner.reader());
    let mut independent = ProjectedReader::new(owner.reader());
    owner.capture().unwrap();
    let before = reader.poll().unwrap().frame.unwrap();
    independent.poll().unwrap();
    let a = owner
        .create_anchor(Point {
            space: Space::Active,
            coordinate: Coordinate { column: 1, row: 0 },
        })
        .unwrap();
    let b = owner
        .create_anchor(Point {
            space: Space::Active,
            coordinate: Coordinate { column: 3, row: 1 },
        })
        .unwrap();
    owner.set_selection(&a, &b, true).unwrap();
    assert_eq!(reader.poll().unwrap().damage, Damage::None);
    let source = owner.capture().unwrap();
    let update = reader.poll().unwrap();
    assert_eq!(update.damage, Damage::Rows(vec![0, 1]));
    assert_eq!(independent.poll().unwrap().damage, Damage::Rows(vec![0, 1]));
    let selected = update.frame.unwrap();
    assert!(Arc::ptr_eq(selected.source(), &source));
    assert_eq!(
        selected.selection_spans(),
        &[
            SelectionSpan {
                row: 0,
                columns: 1..4
            },
            SelectionSpan {
                row: 1,
                columns: 1..4
            },
        ]
    );
    for y in 0..3 {
        assert_eq!(before.row(y).unwrap().key(), selected.row(y).unwrap().key());
    }
    owner.clear_selection().unwrap();
    owner.capture().unwrap();
    let cleared = reader.poll().unwrap();
    assert_eq!(cleared.damage, Damage::Rows(vec![0, 1]));
    assert!(cleared.frame.unwrap().selection_spans().is_empty());
    assert_eq!(selected.selection_spans().len(), 2);
    assert_eq!(independent.poll().unwrap().damage, Damage::Rows(vec![0, 1]));
    owner.close().unwrap();
}

#[test]
fn disjoint_selection_masks_preserve_exact_half_open_spans() {
    let mut native = frame();
    for x in [0, 1, 4, 11] {
        native.rows[0].cells[x].selected = true;
    }
    let projected = ProjectedFrame::new(Arc::new(native)).unwrap();
    assert_eq!(
        projected.selection_spans(),
        &[
            SelectionSpan {
                row: 0,
                columns: 0..2
            },
            SelectionSpan {
                row: 0,
                columns: 4..5
            },
            SelectionSpan {
                row: 0,
                columns: 11..12
            },
        ]
    );
}

#[test]
fn owned_publication_reuses_capture_and_preserves_atomic_metadata() {
    use crate::ghostty::owner::EngineOwner;
    let mut owner = EngineOwner::new(12, 3).unwrap();
    let mut reader = ProjectedReader::new(owner.reader());
    assert!(reader.poll().unwrap().frame.is_none());
    owner.feed(b"\x1b[?2004hA").unwrap();
    let source = owner.capture().unwrap();
    let update = reader.poll().unwrap();
    assert_eq!(update.damage, Damage::Full);
    let first = update.frame.unwrap();
    assert!(Arc::ptr_eq(first.source(), &source));
    assert!(first.source().metadata.modes.bracketed_paste);
    assert_eq!(
        first.row(0).unwrap().cells()[0].content,
        SnapshotContent::Scalar('A')
    );
    let repeated = reader
        .poll_with(|_| panic!("unchanged capture must not project"))
        .unwrap();
    assert_eq!(repeated.damage, Damage::None);
    assert!(Arc::ptr_eq(&first, &repeated.frame.unwrap()));
    owner.close().unwrap();
    let closed = reader.poll().unwrap();
    assert_eq!(closed.state, State::Closed);
    assert!(Arc::ptr_eq(&first, &closed.frame.unwrap()));
}

#[test]
fn rejected_projection_preserves_last_good_frame_and_unacknowledged_damage() {
    use crate::ghostty::owner::EngineOwner;
    let mut owner = EngineOwner::new(12, 3).unwrap();
    let mut first = ProjectedReader::new(owner.reader());
    let mut other = ProjectedReader::new(owner.reader());
    owner.capture().unwrap();
    let original = first.poll().unwrap().frame.unwrap();
    other.poll().unwrap();
    owner.feed(b"A").unwrap();
    owner.capture().unwrap();
    let rejected = first.poll_with(|source| {
        let mut malformed = (*source).clone();
        malformed.rows[0].cells[0].style.flags = 0x8000;
        ProjectedFrame::new(Arc::new(malformed))
    });
    assert!(matches!(rejected, Err(Error::InvalidSnapshot(_))));
    assert!(Arc::ptr_eq(first.current().unwrap(), &original));
    assert_eq!(other.poll().unwrap().damage, Damage::Rows(vec![0]));
    // Skip another capture after failure: both changes must still be reported.
    owner.feed(b"\x1b[3;1HB").unwrap();
    let latest = owner.capture().unwrap();
    let recovered = first.poll().unwrap();
    assert_eq!(recovered.damage, Damage::Rows(vec![0, 2]));
    assert!(Arc::ptr_eq(recovered.frame.unwrap().source(), &latest));
    assert_eq!(other.poll().unwrap().damage, Damage::Rows(vec![0, 2]));
    owner.close().unwrap();
}

#[test]
fn native_failure_is_observable_without_discarding_the_paint_publication() {
    use crate::ghostty::owner::{EngineOwner, Failure};
    let mut owner = EngineOwner::new(12, 3).unwrap();
    let mut reader = ProjectedReader::new(owner.reader());
    owner.capture().unwrap();
    let previous = reader.poll().unwrap().frame.unwrap();
    assert!(owner.feed(&b"\x1b[6n".repeat(4096)).is_err());
    let update = reader.poll().unwrap();
    assert_eq!(
        update.state,
        State::Failed(Failure::Native(Error::ReplyOverflow))
    );
    assert_eq!(update.damage, Damage::None);
    assert!(Arc::ptr_eq(&previous, &update.frame.unwrap()));
    assert!(owner.close().is_err());
}

#[test]
fn projection_preserves_clusters_width_styles_and_wrap_without_anchors() {
    let mut terminal = Terminal::new(12, 3).unwrap();
    terminal
        .write("\x1b[?2027h\x1b[1;3;4:3;38;2;11;22;33m日e\u{301}👩‍💻123456789012345".as_bytes())
        .unwrap();
    let native = RenderBuffer::new().unwrap().capture(&mut terminal).unwrap();
    let projected = ProjectedFrame::new(Arc::new(native)).unwrap();
    drop(terminal);
    let row = projected.row(0).unwrap();
    assert_eq!(row.logical_id(), None);
    assert_eq!(row.cells()[0].width, CellWidth::Wide);
    assert_eq!(row.cells()[1].width, CellWidth::WideSpacer);
    let SnapshotContent::Cluster { start, len } = row.cells()[2].content else {
        panic!("expected combining cluster")
    };
    assert_eq!(row.cluster(start, len), &['e', '\u{301}']);
    let SnapshotContent::Cluster { start, len } = row.cells()[3].content else {
        panic!("expected ZWJ cluster")
    };
    assert_eq!(row.cluster(start, len), &['👩', '\u{200d}', '💻']);
    let style = row.style_of(&row.cells()[0]);
    assert!(
        style
            .attrs
            .contains(Attrs::BOLD | Attrs::ITALIC | Attrs::UNDERCURL)
    );
    assert_eq!(
        style.fg,
        Color::Rgb(Rgb {
            r: 11,
            g: 22,
            b: 33
        })
    );
    assert!(row.wrapped());
    assert!(row.cells().iter().all(|cell| cell.hyperlink.is_none()
        && cell.graphic.is_none()
        && cell.semantic == Semantic::None));
}

#[test]
fn exact_keys_track_cells_styles_palette_and_do_not_depend_on_capture_lifetime() {
    let mut terminal = Terminal::new(12, 3).unwrap();
    terminal.write(b"hello").unwrap();
    let mut capture = RenderBuffer::new().unwrap();
    let first = ProjectedFrame::new(Arc::new(capture.capture(&mut terminal).unwrap())).unwrap();
    let key = first.row(0).unwrap().key();
    let repeat = ProjectedFrame::new(Arc::new(capture.capture(&mut terminal).unwrap())).unwrap();
    assert_eq!(key, repeat.row(0).unwrap().key());
    terminal.write(b"\r\x1b[31mH").unwrap();
    let changed = ProjectedFrame::new(Arc::new(capture.capture(&mut terminal).unwrap())).unwrap();
    assert_ne!(key, changed.row(0).unwrap().key());
    assert_eq!(first.row(1).unwrap().key(), changed.row(1).unwrap().key());
    let mut native = capture.capture(&mut terminal).unwrap();
    native.background.r ^= 0xff;
    let palette = ProjectedFrame::new(Arc::new(native)).unwrap();
    assert_ne!(changed.row(1).unwrap().key(), palette.row(1).unwrap().key());
    drop(first);
    drop(terminal);
    assert_eq!(key, repeat.row(0).unwrap().key());
}

#[test]
fn projection_rejects_malformed_or_unsupported_values_instead_of_partial_rows() {
    let mut native = frame();
    native.rows[0].cells.pop();
    assert!(matches!(
        ProjectedFrame::new(Arc::new(native)),
        Err(Error::InvalidSnapshot(_))
    ));
    let mut native = frame();
    native.rows[0].cells[0].style.flags = 0x8000;
    assert!(matches!(
        ProjectedFrame::new(Arc::new(native)),
        Err(Error::InvalidSnapshot(_))
    ));
    let mut native = frame();
    native.rows[0].cells[0].text = "x".repeat(render::MAX_TEXT_BYTES + 1);
    assert!(matches!(
        ProjectedFrame::new(Arc::new(native)),
        Err(Error::SnapshotLimit)
    ));
    let mut native = frame();
    native.rows[0].cells[0].style.flags = 6 << 8;
    assert!(matches!(
        ProjectedFrame::new(Arc::new(native)),
        Err(Error::InvalidSnapshot(_))
    ));
}

#[test]
fn every_native_attribute_maps_explicitly_and_runs_merge_only_equal_styles() {
    let mut native = frame();
    native.rows[0].cells[0].style.flags = 0xff | (5 << 8);
    let row = ProjectedFrame::new(Arc::new(native.clone())).unwrap();
    let row = row.row(0).unwrap();
    let attrs = row.style_of(&row.cells()[0]).attrs;
    assert_eq!(
        attrs,
        Attrs::BOLD
            | Attrs::ITALIC
            | Attrs::DIM
            | Attrs::BLINK_SLOW
            | Attrs::INVERSE
            | Attrs::HIDDEN
            | Attrs::STRIKEOUT
            | Attrs::OVERLINE
            | Attrs::DASHED_UNDERLINE
    );
    assert_eq!(row.runs().len(), 2);
    assert_eq!(row.runs()[0].cols, 0..1);
    assert_eq!(row.runs()[1].cols, 1..12);
    for (value, expected) in [
        (0, Attrs::empty()),
        (1, Attrs::UNDERLINE),
        (2, Attrs::DOUBLE_UNDERLINE),
        (3, Attrs::UNDERCURL),
        (4, Attrs::DOTTED_UNDERLINE),
        (5, Attrs::DASHED_UNDERLINE),
    ] {
        native.rows[0].cells[0].style.flags = value << 8;
        let projected = ProjectedFrame::new(Arc::new(native.clone())).unwrap();
        let row = projected.row(0).unwrap();
        assert_eq!(row.style_of(&row.cells()[0]).attrs, expected);
    }
}
