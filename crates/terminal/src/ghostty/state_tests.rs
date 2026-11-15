use super::*;
use crate::ghostty::owner::{Damage, EngineOwner};
use crate::ghostty::render::RenderBuffer;

#[test]
fn owned_selection_updates_reach_the_native_owner() {
    use crate::ghostty::anchors::{Coordinate, Point, Space};
    use crate::ghostty::selection::Update;

    let mut owner = EngineOwner::new(12, 3).unwrap();
    owner.feed(b"owned").unwrap();
    let anchor = owner
        .create_anchor(Point {
            space: Space::Active,
            coordinate: Coordinate { column: 0, row: 0 },
        })
        .unwrap();
    for update in [
        Update::Set {
            start: anchor.clone(),
            end: anchor,
            rectangle: false,
        },
        Update::All,
        Update::Clear,
    ] {
        match update {
            Update::All => {
                owner.select_all().unwrap();
                assert_eq!(owner.selection_text().unwrap().as_deref(), Some("owned"));
            }
            Update::Set {
                start,
                end,
                rectangle,
            } => {
                owner.set_selection(&start, &end, rectangle).unwrap();
                assert_eq!(owner.selection_text().unwrap().as_deref(), Some("o"));
            }
            Update::Clear => {
                owner.clear_selection().unwrap();
                assert_eq!(owner.selection_text().unwrap(), None);
            }
            Update::Gesture(_) => panic!("gesture is covered by published-viewport tests"),
        }
    }
    owner.close().unwrap();
}

#[test]
fn configured_scrollback_limits_match_pinned_native_readback() {
    let mut terminal = Terminal::new(10, 3).unwrap();
    terminal.set_scrollback_limit(1234).unwrap();
    assert_eq!(terminal.read_state(35, 0usize).unwrap(), 1234);
    let mut bytes = 0usize;
    // SAFETY: pinned data key 34 writes size_t; unlimited returns NO_VALUE.
    assert_eq!(
        unsafe {
            ghostty_terminal_get(
                terminal.raw.as_ptr(),
                34,
                std::ptr::from_mut(&mut bytes).cast(),
            )
        },
        -4
    );
    terminal.set_scrollback_limit(0).unwrap();
    assert_eq!(terminal.read_state(34, usize::MAX).unwrap(), 0);
    assert_eq!(terminal.read_state(35, usize::MAX).unwrap(), 0);
    for _ in 0..10 {
        terminal.write(&[b'x', 13, 10]).unwrap();
    }
    assert_eq!(terminal.metadata().unwrap().scrollbar.total, 3);
}

#[test]
fn native_modes_and_rows_are_captured_together_and_owned() {
    let mut terminal = Terminal::new(10, 3).unwrap();
    terminal
        .write(b"\x1b[?1;66;2004;5;1004;1003;1006h\x1b[4h\x1b[?25lA")
        .unwrap();
    let frame = RenderBuffer::new().unwrap().capture(&mut terminal).unwrap();
    let modes = &frame.metadata.modes;
    assert!(modes.cursor_keys && modes.keypad_keys && modes.bracketed_paste);
    assert!(modes.reverse_colors && modes.insert && modes.focus_events);
    assert!(modes.mouse_any && modes.mouse_sgr && !modes.cursor_visible);
    assert!(!frame.cursor.visible);
    assert_eq!(frame.rows[0].cells[0].text, "A");
    terminal.write(b"\x1b[?1049hALT").unwrap();
    let alternate = RenderBuffer::new().unwrap().capture(&mut terminal).unwrap();
    assert!(alternate.metadata.alternate_screen);
    assert_eq!(alternate.metadata.scrollbar.total, 3);
    drop(terminal);
    assert!(!frame.metadata.alternate_screen);
    assert_eq!(frame.rows[0].cells[0].text, "A");
}

#[test]
fn viewport_interactions_round_trip_real_native_history_without_row_ids() {
    let mut owner = EngineOwner::new(10, 3).unwrap();
    owner.feed(b"zero\r\none\r\ntwo\r\nthree\r\nfour").unwrap();
    let bottom = owner.capture().unwrap();
    assert!(bottom.metadata.scrollbar.total > 3);
    owner.scroll(Scroll::Top).unwrap();
    let top = owner.capture().unwrap();
    assert_eq!(top.metadata.scrollbar.offset, 0);
    assert_eq!(top.rows[0].cells[0].text, "z");
    owner.scroll(Scroll::Delta(1)).unwrap();
    let next = owner.capture().unwrap();
    assert_eq!(next.metadata.scrollbar.offset, 1);
    owner.scroll(Scroll::Row(0)).unwrap();
    assert_eq!(
        owner.capture().unwrap().metadata.scrollbar,
        top.metadata.scrollbar
    );
    owner.scroll(Scroll::Bottom).unwrap();
    assert_eq!(*owner.capture().unwrap(), *bottom);
    owner.close().unwrap();
}

#[test]
fn metadata_only_changes_and_skipped_publications_damage_each_reader() {
    let mut owner = EngineOwner::new(10, 3).unwrap();
    let mut first = owner.reader();
    let mut second = owner.reader();
    owner.capture().unwrap();
    assert_eq!(first.poll().unwrap().damage, Damage::Full);
    assert_eq!(second.poll().unwrap().damage, Damage::Full);
    owner.feed(b"\x1b[?2004h").unwrap();
    owner.capture().unwrap();
    assert_eq!(first.poll().unwrap().damage, Damage::Full);
    owner.feed(b"\x1b[?1h").unwrap();
    owner.capture().unwrap();
    let update = second.poll().unwrap();
    assert_eq!(update.damage, Damage::Full);
    assert!(update.frame.unwrap().metadata.modes.bracketed_paste);
    assert_eq!(second.poll().unwrap().damage, Damage::None);
    owner.close().unwrap();
}

#[test]
fn native_metadata_and_scroll_layouts_match_c_guards() {
    assert_eq!(size_of::<ModeConfig>(), 4);
    assert_eq!(align_of::<ModeConfig>(), 2);
    assert_eq!(std::mem::offset_of!(ModeConfig, value), 2);
    assert_eq!(size_of::<Scrollbar>(), 24);
    assert_eq!(align_of::<Scrollbar>(), 8);
    assert_eq!(size_of::<NativeScroll>(), 24);
    assert_eq!(align_of::<NativeScroll>(), 8);
    assert_eq!(std::mem::offset_of!(NativeScroll, value), 8);
}

#[test]
fn screen_round_trip_and_reset_do_not_leave_cached_input_modes() {
    let mut terminal = Terminal::new(10, 3).unwrap();
    terminal
        .write(b"\x1b[?1;2004;1002;1006h\x1b[>3uprimary")
        .unwrap();
    let original = terminal.metadata().unwrap();
    assert_eq!(original.kitty_keyboard_flags, 3);
    assert!(original.modes.mouse_button && original.modes.mouse_sgr);
    terminal.write(b"\x1b[?1049h\x1b[>7u").unwrap();
    assert!(terminal.metadata().unwrap().alternate_screen);
    assert_eq!(terminal.metadata().unwrap().kitty_keyboard_flags, 7);
    terminal.write(b"\x1b[?1049l").unwrap();
    assert!(!terminal.metadata().unwrap().alternate_screen);
    assert_eq!(terminal.metadata().unwrap().kitty_keyboard_flags, 3);
    terminal.write(b"\x1bc").unwrap();
    let reset = terminal.metadata().unwrap();
    assert!(!reset.modes.cursor_keys && !reset.modes.bracketed_paste);
    assert!(!reset.modes.mouse_button && !reset.modes.mouse_sgr);
    assert!(reset.modes.cursor_visible);
    assert_eq!(reset.kitty_keyboard_flags, 0);
}
