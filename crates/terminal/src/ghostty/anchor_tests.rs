use super::*;
use crate::ghostty::owner::{Damage, EngineOwner, Error as OwnerError, State};
use crate::ghostty::state::Scroll;

fn point(space: Space, column: u16, row: u32) -> Point {
    Point {
        space,
        coordinate: Coordinate { column, row },
    }
}

fn position(column: u16, row: u32, alternate_screen: bool) -> Option<Position> {
    Some(Position {
        coordinate: Coordinate { column, row },
        alternate_screen,
    })
}

#[test]
fn anchors_follow_cells_into_history_and_viewport_without_row_arithmetic() {
    let mut owner = EngineOwner::new(12, 3).unwrap();
    owner.feed(b"zero").unwrap();
    let id = owner.create_anchor(point(Space::Active, 2, 0)).unwrap();
    owner.feed(b"\r\none\r\ntwo\r\nthree\r\nfour").unwrap();
    let bottom = owner.create_anchor(point(Space::Active, 0, 2)).unwrap();
    assert_eq!(owner.resolve_anchor(&bottom, Space::History).unwrap(), None);
    assert!(matches!(
        owner.create_anchor(point(Space::History, 0, 2)),
        Err(OwnerError::Anchor(AnchorError::InvalidPoint))
    ));
    assert_eq!(
        owner.resolve_anchor(&id, Space::Screen).unwrap(),
        position(2, 0, false)
    );
    assert_eq!(
        owner.resolve_anchor(&id, Space::History).unwrap(),
        position(2, 0, false)
    );
    assert_eq!(owner.resolve_anchor(&id, Space::Active).unwrap(), None);
    assert_eq!(owner.resolve_anchor(&id, Space::Viewport).unwrap(), None);
    owner.scroll(Scroll::Top).unwrap();
    assert_eq!(
        owner.resolve_anchor(&bottom, Space::Viewport).unwrap(),
        None
    );
    assert!(matches!(
        owner.create_anchor(point(Space::Viewport, 0, 3)),
        Err(OwnerError::Anchor(AnchorError::InvalidPoint))
    ));
    assert_eq!(
        owner.resolve_anchor(&id, Space::Viewport).unwrap(),
        position(2, 0, false)
    );
    owner.scroll(Scroll::Bottom).unwrap();
    assert_eq!(owner.resolve_anchor(&id, Space::Viewport).unwrap(), None);
    owner.release_anchor(&id).unwrap();
    owner.close().unwrap();
}

#[test]
fn erased_history_anchor_stays_absent_instead_of_attaching_to_new_output() {
    let mut owner = EngineOwner::new(12, 3).unwrap();
    owner.feed(b"old").unwrap();
    let id = owner.create_anchor(point(Space::Active, 0, 0)).unwrap();
    owner.feed(b"\r\none\r\ntwo\r\nthree\r\nfour").unwrap();
    owner.feed(b"\x1b[3J").unwrap();
    assert_eq!(owner.resolve_anchor(&id, Space::Screen).unwrap(), None);
    owner
        .feed(b"\x1bcnew\r\noutput\r\ncontinues\r\nhere")
        .unwrap();
    assert_eq!(owner.resolve_anchor(&id, Space::Screen).unwrap(), None);
    owner.release_anchor(&id).unwrap();
    owner.close().unwrap();
}

#[test]
fn anchor_tracks_native_reflow_in_both_directions() {
    let mut owner = EngineOwner::new(12, 3).unwrap();
    owner.feed(b"abcdefghijk").unwrap();
    let id = owner.create_anchor(point(Space::Active, 8, 0)).unwrap();
    owner.resize(6, 3, 10, 20).unwrap();
    assert_eq!(
        owner.resolve_anchor(&id, Space::Screen).unwrap(),
        position(2, 1, false)
    );
    owner.resize(12, 3, 10, 20).unwrap();
    assert_eq!(
        owner.resolve_anchor(&id, Space::Screen).unwrap(),
        position(8, 0, false)
    );
    owner.close().unwrap();
}

#[test]
fn anchor_coordinates_keep_original_screen_provenance() {
    let mut owner = EngineOwner::new(12, 3).unwrap();
    owner.feed(b"primary").unwrap();
    let primary = owner.create_anchor(point(Space::Active, 2, 0)).unwrap();
    owner.feed(b"\x1b[?1049halt").unwrap();
    let alternate = owner.create_anchor(point(Space::Active, 1, 0)).unwrap();
    assert_ne!(primary, alternate);
    assert_eq!(
        owner.resolve_anchor(&primary, Space::Active).unwrap(),
        position(2, 0, false)
    );
    assert_eq!(
        owner.resolve_anchor(&alternate, Space::Active).unwrap(),
        position(1, 0, true)
    );
    owner.feed(b"\x1b[?1049l").unwrap();
    assert_eq!(
        owner.resolve_anchor(&primary, Space::Active).unwrap(),
        position(2, 0, false)
    );
    owner.release_anchor(&alternate).unwrap();
    owner.release_anchor(&primary).unwrap();
    owner.close().unwrap();
}

#[test]
fn invalid_foreign_and_released_requests_leave_the_worker_and_frame_healthy() {
    let mut owner = EngineOwner::new(12, 3).unwrap();
    let mut other = EngineOwner::new(12, 3).unwrap();
    let mut reader = owner.reader();
    let frame = owner.capture().unwrap();
    reader.poll().unwrap();
    for point in [
        point(Space::Active, 12, 0),
        point(Space::Screen, 0, u32::MAX),
        point(Space::History, 0, 0),
    ] {
        let result = owner.create_anchor(point);
        assert!(
            matches!(result, Err(OwnerError::Anchor(AnchorError::InvalidPoint))),
            "{point:?}: {result:?}"
        );
    }
    let id = owner.create_anchor(point(Space::Viewport, 0, 0)).unwrap();
    let foreign = other.create_anchor(point(Space::Viewport, 0, 0)).unwrap();
    assert_ne!(id, foreign);
    assert!(matches!(
        owner.resolve_anchor(&foreign, Space::Screen),
        Err(OwnerError::Anchor(AnchorError::Foreign))
    ));
    assert!(matches!(
        owner.release_anchor(&foreign),
        Err(OwnerError::Anchor(AnchorError::Foreign))
    ));
    owner.release_anchor(&id).unwrap();
    assert!(matches!(
        owner.release_anchor(&id),
        Err(OwnerError::Anchor(AnchorError::Released))
    ));
    let new = owner.create_anchor(point(Space::Viewport, 0, 0)).unwrap();
    assert_ne!(id, new);
    assert!(matches!(
        owner.resolve_anchor(&id, Space::Screen),
        Err(OwnerError::Anchor(AnchorError::Released))
    ));
    assert_eq!(
        owner.resolve_anchor(&new, Space::Screen).unwrap(),
        position(0, 0, false)
    );
    assert_eq!(reader.poll().unwrap().damage, Damage::None);
    assert_eq!(reader.poll().unwrap().state, State::Running);
    assert_eq!(*frame, *owner.capture().unwrap());
    owner.close().unwrap();
    other.close().unwrap();
}

#[test]
fn registry_is_bounded_and_released_slots_do_not_revive_tokens() {
    let mut owner = EngineOwner::new(12, 3).unwrap();
    let mut ids = Vec::new();
    for _ in 0..MAX_ANCHORS {
        ids.push(owner.create_anchor(point(Space::Active, 0, 0)).unwrap());
    }
    assert!(matches!(
        owner.create_anchor(point(Space::Active, 0, 0)),
        Err(OwnerError::Anchor(AnchorError::Limit))
    ));
    let old = ids.pop().unwrap();
    owner.release_anchor(&old).unwrap();
    let new = owner.create_anchor(point(Space::Active, 1, 0)).unwrap();
    assert_ne!(old, new);
    assert!(matches!(
        owner.resolve_anchor(&old, Space::Screen),
        Err(OwnerError::Anchor(AnchorError::Released))
    ));
    assert_eq!(
        owner.resolve_anchor(&new, Space::Screen).unwrap(),
        position(1, 0, false)
    );
    owner.close().unwrap();
}

#[test]
fn tracked_handle_can_be_resolved_and_freed_after_terminal_destruction() {
    let mut terminal = Terminal::new(12, 3).unwrap();
    let mut anchors = Anchors::new();
    let reply = anchors
        .execute(&mut terminal, Command::Create(point(Space::Active, 0, 0)))
        .unwrap()
        .unwrap();
    assert!(matches!(reply, Reply::Created(_)));
    drop(terminal);
    let mut coordinate = Coordinate::default();
    // SAFETY: upstream explicitly permits tracked-ref calls and free after its
    // terminal is gone. No untracked snapshot or page pointer is ever retained.
    let status = unsafe {
        ghostty_tracked_grid_ref_point(
            anchors.entries[0].tracked.raw.as_ptr(),
            Space::Screen.tag(),
            &mut coordinate,
        )
    };
    assert_eq!(status, -4);
    drop(anchors);
}

#[test]
fn point_layout_and_owned_tokens_match_the_boundary_contract() {
    assert_eq!(size_of::<Coordinate>(), 8);
    assert_eq!(align_of::<Coordinate>(), 4);
    assert_eq!(std::mem::offset_of!(Coordinate, column), 0);
    assert_eq!(std::mem::offset_of!(Coordinate, row), 4);
    assert_eq!(size_of::<PointValue>(), 16);
    assert_eq!(align_of::<PointValue>(), 8);
    assert_eq!(size_of::<NativePoint>(), 24);
    assert_eq!(align_of::<NativePoint>(), 8);
    assert_eq!(std::mem::offset_of!(NativePoint, value), 8);
    fn send_sync<T: Send + Sync>() {}
    send_sync::<AnchorId>();
    send_sync::<Position>();
}
