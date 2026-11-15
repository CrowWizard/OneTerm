use super::*;
use crate::ghostty::anchors::{Coordinate, Point, Space};
use crate::ghostty::owner::{EngineOwner, Error as OwnerError, State};

#[test]
fn select_all_covers_history_and_only_the_active_screen() {
    let mut owner = EngineOwner::new(12, 3).unwrap();
    owner.feed(b"first\r\nsecond\r\nthird\r\nfourth").unwrap();
    owner.select_all().unwrap();
    assert_eq!(
        owner.selection_text().unwrap().as_deref(),
        Some("first\nsecond\nthird\nfourth")
    );
    owner.feed(b"\x1b[?1049h\x1b[Halt").unwrap();
    owner.select_all().unwrap();
    assert_eq!(owner.selection_text().unwrap().as_deref(), Some("alt"));
    owner.feed(b"\x1b[?1049l").unwrap();
    owner.select_all().unwrap();
    assert_eq!(
        owner.selection_text().unwrap().as_deref(),
        Some("first\nsecond\nthird\nfourth")
    );
    owner.clear_selection().unwrap();
    assert_eq!(owner.selection_text().unwrap(), None);
    owner.close().unwrap();
}

fn anchor(owner: &mut EngineOwner, column: u16, row: u32) -> AnchorId {
    owner
        .create_anchor(Point {
            space: Space::Active,
            coordinate: Coordinate { column, row },
        })
        .unwrap()
}

#[test]
fn inclusive_linear_reverse_rectangle_and_released_endpoints() {
    let mut owner = EngineOwner::new(12, 4).unwrap();
    assert_eq!(owner.selection_text().unwrap(), None);
    owner.feed(b"abcdef\r\nghijkl").unwrap();
    let a = anchor(&mut owner, 1, 0);
    let b = anchor(&mut owner, 3, 1);
    owner.set_selection(&a, &b, false).unwrap();
    assert_eq!(
        owner.selection_text().unwrap().as_deref(),
        Some("bcdef\nghij")
    );
    owner.set_selection(&b, &a, false).unwrap();
    assert_eq!(
        owner.selection_text().unwrap().as_deref(),
        Some("bcdef\nghij")
    );
    owner.set_selection(&a, &b, true).unwrap();
    assert_eq!(owner.selection_text().unwrap().as_deref(), Some("bcd\nhij"));
    owner.set_selection(&b, &a, true).unwrap();
    assert_eq!(owner.selection_text().unwrap().as_deref(), Some("bcd\nhij"));
    owner.release_anchor(&a).unwrap();
    owner.release_anchor(&b).unwrap();
    assert_eq!(owner.selection_text().unwrap().as_deref(), Some("bcd\nhij"));
    owner.clear_selection().unwrap();
    assert_eq!(owner.selection_text().unwrap(), None);
    let blank = anchor(&mut owner, 9, 2);
    owner.set_selection(&blank, &blank, false).unwrap();
    assert_eq!(owner.selection_text().unwrap().as_deref(), Some(""));
    owner.close().unwrap();
}

#[test]
fn unicode_soft_wrap_and_native_selection_reflow() {
    let mut owner = EngineOwner::new(6, 4).unwrap();
    owner.feed("aé界bcdef".as_bytes()).unwrap();
    let a = anchor(&mut owner, 0, 0);
    let b = anchor(&mut owner, 2, 1);
    owner.set_selection(&a, &b, false).unwrap();
    assert_eq!(
        owner.selection_text().unwrap().as_deref(),
        Some("aé界bcdef")
    );
    owner.resize(12, 4, 10, 20).unwrap();
    assert_eq!(
        owner.selection_text().unwrap().as_deref(),
        Some("aé界bcdef")
    );
    owner.resize(6, 4, 10, 20).unwrap();
    assert_eq!(
        owner.selection_text().unwrap().as_deref(),
        Some("aé界bcdef")
    );
    owner.close().unwrap();
}

#[test]
fn invalid_endpoints_preserve_selection_and_worker_health() {
    let mut owner = EngineOwner::new(12, 3).unwrap();
    let mut foreign = EngineOwner::new(12, 3).unwrap();
    owner.feed(b"primary").unwrap();
    let a = anchor(&mut owner, 0, 0);
    let b = anchor(&mut owner, 6, 0);
    owner.set_selection(&a, &b, false).unwrap();
    let other = anchor(&mut foreign, 0, 0);
    assert!(matches!(
        owner.set_selection(&a, &other, false),
        Err(OwnerError::Selection(SelectionError::Anchor(
            AnchorError::Foreign
        )))
    ));
    owner.release_anchor(&b).unwrap();
    assert!(matches!(
        owner.set_selection(&a, &b, false),
        Err(OwnerError::Selection(SelectionError::Anchor(
            AnchorError::Released
        )))
    ));
    assert_eq!(owner.selection_text().unwrap().as_deref(), Some("primary"));
    owner.feed(b"\x1b[?1049h\x1b[Halt").unwrap();
    let alt = anchor(&mut owner, 0, 0);
    assert!(matches!(
        owner.set_selection(&a, &alt, false),
        Err(OwnerError::Selection(SelectionError::InactiveScreen))
    ));
    assert_eq!(owner.selection_text().unwrap(), None);
    owner.set_selection(&alt, &alt, false).unwrap();
    assert_eq!(owner.selection_text().unwrap().as_deref(), Some("a"));
    owner.clear_selection().unwrap();
    owner.feed(b"\x1b[?1049l").unwrap();
    // Native switchScreen clears the destination screen's selection.
    assert_eq!(owner.selection_text().unwrap(), None);
    owner.feed(b"\x1bc").unwrap();
    assert!(matches!(
        owner.set_selection(&a, &a, false),
        Err(OwnerError::Selection(SelectionError::Expired))
    ));
    assert_eq!(owner.reader().poll().unwrap().state, State::Running);
    owner.close().unwrap();
    foreign.close().unwrap();
}

#[test]
fn formatter_bound_rejects_without_partial_text_or_mutation() {
    let mut terminal = Terminal::new(12, 3).unwrap();
    terminal.write(b"abcdef").unwrap();
    let mut anchors = Anchors::new();
    let mut make = |column| match anchors
        .execute(
            &mut terminal,
            super::super::anchors::Command::Create(Point {
                space: Space::Active,
                coordinate: Coordinate { column, row: 0 },
            }),
        )
        .unwrap()
        .unwrap()
    {
        super::super::anchors::Reply::Created(id) => id,
        _ => panic!("expected anchor"),
    };
    let a = make(0);
    let b = make(5);
    anchors
        .set_selection(&mut terminal, &a, &b, false)
        .unwrap()
        .unwrap();
    assert_eq!(
        copy(&mut terminal, 5).unwrap(),
        Err(SelectionError::TextLimit)
    );
    assert_eq!(
        copy(&mut terminal, 6).unwrap().unwrap().as_deref(),
        Some("abcdef")
    );
}

#[test]
fn public_copy_cap_rejects_utf8_overflow_and_keeps_worker_usable() {
    let mut owner = EngineOwner::new(512, 1024).unwrap();
    let line = "é".repeat(512);
    for row in 0..1024 {
        owner.feed(line.as_bytes()).unwrap();
        if row != 1023 {
            owner.feed(b"\r\n").unwrap();
        }
    }
    let a = anchor(&mut owner, 0, 0);
    let b = anchor(&mut owner, 511, 1023);
    owner.set_selection(&a, &b, false).unwrap();
    assert!(matches!(
        owner.selection_text(),
        Err(OwnerError::Selection(SelectionError::TextLimit))
    ));
    owner.set_selection(&a, &a, false).unwrap();
    assert_eq!(owner.selection_text().unwrap().as_deref(), Some("é"));
    owner.clear_selection().unwrap();
    owner.close().unwrap();
}

#[test]
fn selection_follows_history_and_copy_does_not_depend_on_viewport() {
    let mut owner = EngineOwner::new(12, 3).unwrap();
    owner.feed(b"first").unwrap();
    let a = anchor(&mut owner, 0, 0);
    let b = anchor(&mut owner, 4, 0);
    owner.set_selection(&a, &b, false).unwrap();
    owner.feed(b"\r\nsecond\r\nthird\r\nfourth").unwrap();
    assert_eq!(owner.selection_text().unwrap().as_deref(), Some("first"));
    owner.scroll(crate::ghostty::state::Scroll::Top).unwrap();
    assert_eq!(owner.selection_text().unwrap().as_deref(), Some("first"));
    owner.feed(b"\x1b[3J").unwrap();
    assert!(matches!(
        owner.set_selection(&a, &b, false),
        Err(OwnerError::Selection(SelectionError::Expired))
    ));
    owner.close().unwrap();
}

#[test]
fn selection_layout_matches_c_guards() {
    use std::mem::{align_of, offset_of, size_of};
    assert_eq!((size_of::<GridRef>(), align_of::<GridRef>()), (16, 8));
    assert_eq!(offset_of!(GridRef, x), 8);
    assert_eq!(offset_of!(GridRef, y), 10);
    assert_eq!(
        (size_of::<NativeSelection>(), align_of::<NativeSelection>()),
        (40, 8)
    );
    assert_eq!(offset_of!(NativeSelection, end), 16);
    assert_eq!(offset_of!(NativeSelection, rectangle), 32);
}
