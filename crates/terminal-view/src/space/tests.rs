//! Unit tests for `SpaceTree` split/close/fill operations (ARCH-07).
//!
//! These tests verify tree consistency after each operation — leaf counts,
//! active id, find_leaf, and collapse behavior. Empty leaves and tagged pane
//! identities exercise ownership without constructing terminal sessions.

use gpui::{AppContext as _, FocusHandle};

use super::tree::{SpaceContent, SpaceId, SpaceLeaf, SpaceNode};
use super::{CloseOutcome, SplitDir};

type SpaceTree = super::SpaceTree;

/// Distinct pane identities exercise the shared tree without a legacy session.
#[derive(Clone, Debug, PartialEq, Eq)]
enum PaneIdentity {
    Legacy(u64),
    Native(u64),
}

#[gpui::test]
fn tab_drag_uses_active_or_unambiguous_sole_terminal(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = super::SpaceTree::new_terminal(PaneIdentity::Native(1), focus.clone());
        let original = tree.active();
        assert_eq!(tree.drag_terminal_space(), Some(original));
        let empty = tree.alloc_id();
        let state = cx.new(|_| gpui_component::resizable::ResizableState::default());
        tree.split(
            original,
            SplitDir::Right,
            SpaceLeaf {
                id: empty,
                content: SpaceContent::Empty,
                focus: focus.clone(),
            },
            state,
        );
        assert_eq!(tree.drag_terminal_space(), Some(original));
        tree.fill_empty(empty, PaneIdentity::Native(2)).unwrap();
        assert_eq!(tree.drag_terminal_space(), Some(empty));
        let other_empty = tree.alloc_id();
        let state = cx.new(|_| gpui_component::resizable::ResizableState::default());
        tree.split(
            empty,
            SplitDir::Down,
            SpaceLeaf {
                id: other_empty,
                content: SpaceContent::Empty,
                focus: focus.clone(),
            },
            state,
        );
        assert_eq!(
            tree.drag_terminal_space(),
            None,
            "never guess among inactive sessions"
        );
        tree.set_active(original);
        assert_eq!(tree.drag_terminal_space(), Some(original));
        tree.take_leaf_terminal(original);
        tree.take_leaf_terminal(empty);
        assert_eq!(tree.drag_terminal_space(), None);
    });
}

#[gpui::test]
fn tagged_panes_split_move_and_close_preserve_ownership(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = super::SpaceTree::new_terminal(PaneIdentity::Native(7), focus.clone());
        let original = tree.active();
        let sibling = tree.alloc_id();
        let state = cx.new(|_| gpui_component::resizable::ResizableState::default());
        tree.split(
            original,
            SplitDir::Right,
            SpaceLeaf {
                id: sibling,
                content: SpaceContent::Empty,
                focus: focus.clone(),
            },
            state,
        );
        let moved = tree.take_leaf_terminal(original).unwrap();
        assert_eq!(moved, PaneIdentity::Native(7));
        assert!(tree.fill_empty(sibling, moved).is_ok());
        assert!(tree.fill_empty(original, PaneIdentity::Legacy(9)).is_ok());
        assert_eq!(
            tree.terminal_views(),
            vec![PaneIdentity::Legacy(9), PaneIdentity::Native(7)]
        );
        let (outcome, removed) = tree.close(original);
        assert_eq!(outcome, CloseOutcome::Removed);
        assert_eq!(removed, Some(PaneIdentity::Legacy(9)));
        assert!(tree.is_single());
        assert_eq!(tree.active(), sibling);
        assert_eq!(tree.active_terminal(), Some(PaneIdentity::Native(7)));
        assert_eq!(tree.close(sibling), (CloseOutcome::LastSpaceClosed, None));
        assert_eq!(
            tree.take_leaf_terminal(sibling),
            Some(PaneIdentity::Native(7))
        );
        assert!(tree.has_no_terminals());
    });
}

#[gpui::test]
fn failed_tagged_placement_returns_exact_handle(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let mut tree = super::SpaceTree::new_terminal(PaneIdentity::Native(1), cx.focus_handle());
        assert_eq!(
            tree.fill_empty(tree.active(), PaneIdentity::Native(2)),
            Err(PaneIdentity::Native(2))
        );
        assert_eq!(
            tree.fill_empty(SpaceId(999), PaneIdentity::Native(3)),
            Err(PaneIdentity::Native(3))
        );
        assert_eq!(tree.terminal_views(), vec![PaneIdentity::Native(1)]);
    });
}

#[gpui::test]
fn tagged_focus_resolver_runs_only_for_terminal_leaf(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = super::SpaceTree::new_terminal(PaneIdentity::Native(4), focus.clone());
        assert_eq!(
            tree.active_focus_handle(|pane| {
                assert_eq!(*pane, PaneIdentity::Native(4));
                focus.clone()
            }),
            Some(focus.clone())
        );
        tree.take_leaf_terminal(tree.active());
        assert_eq!(
            tree.active_focus_handle(|_| panic!("empty leaf must use placeholder focus")),
            Some(focus)
        );
    });
}

/// Helper: build a `SpaceLeaf` with `Empty` content and a dummy focus handle.
fn empty_leaf(id: u64, focus: FocusHandle) -> SpaceLeaf {
    SpaceLeaf {
        id: SpaceId(id),
        content: SpaceContent::Empty,
        focus,
    }
}

// ── Split tests ─────────────────────────────────────────────────────

#[gpui::test]
fn split_right_creates_two_leaves(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = SpaceTree::new_empty(focus.clone());
        let target = tree.active();
        let new_id = tree.alloc_id();
        let empty = empty_leaf(new_id.0, focus.clone());
        let state = cx.new(|_| gpui_component::resizable::ResizableState::default());

        tree.split(target, SplitDir::Right, empty, state);

        assert_eq!(tree.leaf_count(), 2);
        assert!(tree.has_leaf(target));
        assert!(tree.has_leaf(new_id));
        assert_eq!(tree.active(), new_id);

        // Root should be a split node.
        assert!(matches!(tree.root(), SpaceNode::Split(_)));
    });
}

#[gpui::test]
fn split_left_inserts_before(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = SpaceTree::new_empty(focus.clone());
        let target = tree.active();
        let new_id = tree.alloc_id();
        let empty = empty_leaf(new_id.0, focus.clone());
        let state = cx.new(|_| gpui_component::resizable::ResizableState::default());

        tree.split(target, SplitDir::Left, empty, state);

        // Root is a split with children [new, existing].
        let SpaceNode::Split(split) = tree.root() else {
            panic!("expected split root");
        };
        // Left = new leaf first.
        let first_id = match &split.children[0] {
            SpaceNode::Leaf(l) => l.id,
            _ => panic!("expected leaf"),
        };
        assert_eq!(first_id, new_id);
    });
}

#[gpui::test]
fn split_nonexistent_target_is_noop(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = SpaceTree::new_empty(focus.clone());
        let new_id = tree.alloc_id();
        let empty = empty_leaf(new_id.0, focus.clone());
        let state = cx.new(|_| gpui_component::resizable::ResizableState::default());

        // Try to split a leaf that doesn't exist.
        tree.split(SpaceId(999), SplitDir::Right, empty, state);

        // Tree should be unchanged.
        assert_eq!(tree.leaf_count(), 1);
        assert!(tree.is_single());
        // Active should not have changed to the non-existent new_id.
        assert_ne!(tree.active(), new_id);
    });
}

#[gpui::test]
fn split_nested_creates_three_leaves(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = SpaceTree::new_empty(focus.clone());
        let leaf0 = tree.active();

        // First split: leaf0 → [leaf0, leaf1]
        let id1 = tree.alloc_id();
        let empty1 = empty_leaf(id1.0, focus.clone());
        let state1 = cx.new(|_| gpui_component::resizable::ResizableState::default());
        tree.split(leaf0, SplitDir::Right, empty1, state1);
        assert_eq!(tree.leaf_count(), 2);

        // Second split: split leaf1 → [leaf1, leaf2]
        let id2 = tree.alloc_id();
        let empty2 = empty_leaf(id2.0, focus.clone());
        let state2 = cx.new(|_| gpui_component::resizable::ResizableState::default());
        tree.split(id1, SplitDir::Down, empty2, state2);
        assert_eq!(tree.leaf_count(), 3);

        assert!(tree.has_leaf(leaf0));
        assert!(tree.has_leaf(id1));
        assert!(tree.has_leaf(id2));
        assert_eq!(tree.active(), id2);
    });
}

// ── Close tests ──────────────────────────────────────────────────────

#[gpui::test]
fn empty_spaces_keep_stable_numbers_when_an_earlier_space_is_removed(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = SpaceTree::new_empty(focus.clone());
        let first = tree.active();

        let second = tree.alloc_id();
        let state = cx.new(|_| gpui_component::resizable::ResizableState::default());
        tree.split(
            first,
            SplitDir::Right,
            empty_leaf(second.0, focus.clone()),
            state,
        );
        let third = tree.alloc_id();
        let state = cx.new(|_| gpui_component::resizable::ResizableState::default());
        tree.split(first, SplitDir::Down, empty_leaf(third.0, focus), state);

        assert_eq!(tree.empty_space_destinations(), vec![first, third, second]);

        tree.close(first);
        assert_eq!(tree.empty_space_destinations(), vec![third, second]);

        let fourth = tree.alloc_id();
        assert_eq!(fourth.display_number(), 4);
        let state = cx.new(|_| gpui_component::resizable::ResizableState::default());
        tree.split(
            second,
            SplitDir::Right,
            empty_leaf(fourth.0, cx.focus_handle()),
            state,
        );
        assert_eq!(tree.empty_space_destinations(), vec![third, second, fourth]);
    });
}

#[gpui::test]
fn close_last_leaf_returns_last_space_closed(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = SpaceTree::new_empty(focus);
        let leaf = tree.active();

        let (outcome, view) = tree.close(leaf);
        assert_eq!(outcome, CloseOutcome::LastSpaceClosed);
        assert!(view.is_none());
    });
}

#[gpui::test]
fn close_one_of_two_collapses_to_single(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = SpaceTree::new_empty(focus.clone());
        let leaf0 = tree.active();
        let id1 = tree.alloc_id();
        let empty = empty_leaf(id1.0, focus.clone());
        let state = cx.new(|_| gpui_component::resizable::ResizableState::default());
        tree.split(leaf0, SplitDir::Right, empty, state);
        assert_eq!(tree.leaf_count(), 2);

        // Close leaf0 — should collapse to just leaf1.
        let (outcome, _view) = tree.close(leaf0);
        assert_eq!(outcome, CloseOutcome::Removed);
        assert_eq!(tree.leaf_count(), 1);
        assert!(tree.is_single());
        assert!(!tree.has_leaf(leaf0));
        assert!(tree.has_leaf(id1));
    });
}

#[gpui::test]
fn close_updates_active_when_active_removed(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = SpaceTree::new_empty(focus.clone());
        let leaf0 = tree.active();
        let id1 = tree.alloc_id();
        let empty = empty_leaf(id1.0, focus.clone());
        let state = cx.new(|_| gpui_component::resizable::ResizableState::default());
        tree.split(leaf0, SplitDir::Right, empty, state);

        // Active is id1 (the new leaf). Close it — active should fall back.
        assert_eq!(tree.active(), id1);
        let (outcome, _) = tree.close(id1);
        assert_eq!(outcome, CloseOutcome::Removed);
        assert_eq!(tree.leaf_count(), 1);
        assert_eq!(tree.active(), leaf0);
    });
}

#[gpui::test]
fn close_nested_collapses_correctly(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = SpaceTree::new_empty(focus.clone());
        let leaf0 = tree.active();

        // Split into 3: [leaf0, split([leaf1, leaf2])]
        let id1 = tree.alloc_id();
        let empty1 = empty_leaf(id1.0, focus.clone());
        let state1 = cx.new(|_| gpui_component::resizable::ResizableState::default());
        tree.split(leaf0, SplitDir::Right, empty1, state1);

        let id2 = tree.alloc_id();
        let empty2 = empty_leaf(id2.0, focus.clone());
        let state2 = cx.new(|_| gpui_component::resizable::ResizableState::default());
        tree.split(id1, SplitDir::Down, empty2, state2);
        assert_eq!(tree.leaf_count(), 3);

        // Close leaf1 — the inner split should collapse, leaving [leaf0, leaf2].
        let (outcome, _) = tree.close(id1);
        assert_eq!(outcome, CloseOutcome::Removed);
        assert_eq!(tree.leaf_count(), 2);
        assert!(tree.has_leaf(leaf0));
        assert!(tree.has_leaf(id2));
        assert!(!tree.has_leaf(id1));
    });
}

// ── Fill / take tests ────────────────────────────────────────────────

#[gpui::test]
fn fill_empty_replaces_content(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = super::SpaceTree::<PaneIdentity>::new_empty(focus);
        let leaf_id = tree.active();

        assert!(tree.fill_empty(leaf_id, PaneIdentity::Native(5)).is_ok());
        assert_eq!(tree.active_terminal(), Some(PaneIdentity::Native(5)));
        assert!(tree.empty_space_destinations().is_empty());
    });
}

#[gpui::test]
fn take_leaf_terminal_returns_none_for_empty(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = SpaceTree::new_empty(focus);
        let leaf_id = tree.active();

        let result = tree.take_leaf_terminal(leaf_id);
        assert!(result.is_none());
    });
}

#[gpui::test]
fn take_leaf_terminal_returns_none_for_nonexistent(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = SpaceTree::new_empty(focus);

        let result = tree.take_leaf_terminal(SpaceId(999));
        assert!(result.is_none());
    });
}

// ── Consistency tests ────────────────────────────────────────────────

#[gpui::test]
fn set_active_only_for_existing_leaves(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = SpaceTree::new_empty(focus.clone());
        let leaf0 = tree.active();

        // Setting to a nonexistent leaf is a no-op.
        tree.set_active(SpaceId(999));
        assert_eq!(tree.active(), leaf0);

        // Split and set active to the new leaf.
        let id1 = tree.alloc_id();
        let empty = empty_leaf(id1.0, focus.clone());
        let state = cx.new(|_| gpui_component::resizable::ResizableState::default());
        tree.split(leaf0, SplitDir::Right, empty, state);

        tree.set_active(id1);
        assert_eq!(tree.active(), id1);

        // Setting back to old leaf.
        tree.set_active(leaf0);
        assert_eq!(tree.active(), leaf0);
    });
}

#[gpui::test]
fn tree_starts_with_single_empty_leaf(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let tree = SpaceTree::new_empty(focus);

        assert!(tree.is_single());
        assert_eq!(tree.leaf_count(), 1);
        assert!(tree.has_no_terminals());
        assert_eq!(tree.terminal_views().len(), 0);
        assert_eq!(tree.empty_space_destinations(), vec![SpaceId(1)]);
    });
}

#[gpui::test]
fn alloc_id_is_monotonic(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let focus = cx.focus_handle();
        let mut tree = SpaceTree::new_empty(focus);

        let id1 = tree.alloc_id();
        let id2 = tree.alloc_id();
        let id3 = tree.alloc_id();

        assert_eq!(id1.0, 2);
        assert_eq!(id2.0, 3);
        assert_eq!(id3.0, 4);
    });
}
