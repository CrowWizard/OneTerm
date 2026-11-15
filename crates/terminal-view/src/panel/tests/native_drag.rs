//! Real strip gestures must transfer native panes without ending their sessions.

use super::*;
use crate::native_surface::tests::native_duplicate_fixture;
use gpui::{Modifiers, MouseButton, point, px};
use gpui_component::dock::BasePanelView as _;
use std::sync::atomic::Ordering;

#[gpui::test]
fn native_strip_drag_moves_into_empty_split(cx: &mut TestAppContext) {
    exercise_drag(cx, false, false, false, None);
}

#[gpui::test]
fn native_strip_drag_moves_sole_terminal_with_empty_active_space(cx: &mut TestAppContext) {
    exercise_drag(cx, true, false, false, None);
}

#[gpui::test]
fn native_strip_drag_moves_own_terminal_into_new_split(cx: &mut TestAppContext) {
    exercise_drag(cx, false, true, false, None);
}

#[gpui::test]
fn native_strip_drag_reveals_hovered_tab_then_moves_into_its_split(cx: &mut TestAppContext) {
    exercise_drag(cx, false, false, true, None);
}

#[gpui::test]
fn native_strip_drag_onto_tab_creates_split_without_preparation(cx: &mut TestAppContext) {
    exercise_drag(cx, false, false, true, Some(SplitDir::Right));
}

#[gpui::test]
fn native_strip_drag_onto_tab_moves_sole_terminal_with_empty_active_space(cx: &mut TestAppContext) {
    exercise_drag(cx, true, false, true, Some(SplitDir::Right));
}

#[gpui::test]
fn native_strip_drag_previews_and_splits_left(cx: &mut TestAppContext) {
    exercise_drag(cx, false, false, true, Some(SplitDir::Left));
}

#[gpui::test]
fn native_strip_drag_previews_and_splits_up(cx: &mut TestAppContext) {
    exercise_drag(cx, false, false, true, Some(SplitDir::Up));
}

#[gpui::test]
fn native_strip_drag_previews_and_splits_down(cx: &mut TestAppContext) {
    exercise_drag(cx, false, false, true, Some(SplitDir::Down));
}

fn exercise_drag(
    cx: &mut TestAppContext,
    empty_source: bool,
    same_tab: bool,
    reveal: bool,
    direction: Option<SplitDir>,
) {
    let direct = direction.is_some();
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    cx.update(crate::init);
    cx.update(oneterm_settings::TerminalSettings::init);
    cx.update(oneterm_state::AppState::init);
    cx.update(InputChannelRegistry::init);
    let (target_session, target_events, target_done) = native_duplicate_fixture(b"target");
    let (source_session, source_events, source_done) = native_duplicate_fixture(b"source");
    let handed = Rc::new(RefCell::new(None));
    let output = handed.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let (dock, _skin) =
            gpui_component::dock::DockSkin::dock_area("native-drag", None, window, cx);
        let target = TerminalPanel::open(
            PanelSpec::NativeSession {
                session: target_session,
                title: "Target".into(),
                duplicate_config: None,
            },
            window,
            cx,
        );
        let source = TerminalPanel::open(
            PanelSpec::NativeSession {
                session: source_session,
                title: "Source".into(),
                duplicate_config: None,
            },
            window,
            cx,
        );
        dock.update(cx, |dock, cx| {
            dock.set_center(
                DockLayout::tabs()
                    .panel_view(panel_handle(target.clone()), cx)
                    .panel_view(panel_handle(source.clone()), cx),
                window,
                cx,
            )
        });
        oneterm_state::AppState::global(cx)
            .update(cx, |state, _| state.dock_area = Some(dock.downgrade()));
        *output.borrow_mut() = Some((dock.clone(), target, source));
        Root::new(dock, window, cx)
    });
    let (dock, target, source) = handed.borrow().clone().unwrap();
    let dragged = if same_tab {
        target.clone()
    } else {
        source.clone()
    };
    let source_pane = dragged.read_with(cx, |panel, _| panel.tree.active_terminal().unwrap());
    let identity = source_pane.id();
    cx.update(|_, cx| source_pane.join_channel(InputChannel::C, cx));
    // Splitting activates the new empty Space. The tab still owns one live
    // terminal, so dragging that tab must not silently take nothing.
    if empty_source {
        source.update_in(cx, |panel, window, cx| {
            panel.split_active_at(panel.tree.active(), SplitDir::Down, window, cx);
            assert!(panel.tree.active_terminal().is_none());
        });
    }
    let (target_id, source_id) = cx.update(|_, cx| (target.panel_id(cx), source.panel_id(cx)));
    dock.update_in(cx, |dock, window, cx| {
        dock.select_panel(target_id, window, cx)
    });
    let empty = target.update_in(cx, |panel, window, cx| {
        if !direct {
            panel.split_active_at(panel.tree.active(), SplitDir::Right, window, cx);
        }
        panel.move_tab_into(crate::space::SpaceId(u64::MAX), dragged.clone(), window, cx);
        panel.tree.active()
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let area = dock.read_with(cx, |dock, _| dock.bounds());
    for fraction in [0.15, 0.85] {
        if !same_tab && fraction < 0.5 {
            continue;
        }
        if reveal {
            dock.update_in(cx, |dock, window, cx| {
                dock.select_panel(source_id, window, cx)
            });
            cx.run_until_parked();
            cx.update(|window, cx| window.draw(cx).clear(cx));
            let hover = cx.debug_bounds("dock-tab-0").unwrap().center();
            cx.simulate_mouse_move(hover, None, Modifiers::none());
            assert!(
                source.read_with(cx, |panel, _| panel.is_active),
                "ordinary hover must not select"
            );
        }
        let tab = cx
            .debug_bounds(if same_tab { "dock-tab-0" } else { "dock-tab-1" })
            .expect("source tab is visible");
        let from = tab.center();
        let mut to = {
            point(
                area.left() + area.size.width * fraction,
                area.top() + area.size.height * 0.5,
            )
        };
        cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(
            from + point(px(0.), px(6.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        if reveal {
            assert!(source.read_with(cx, |panel, _| panel.is_active));
            let hover = cx.debug_bounds("dock-tab-0").unwrap().center();
            cx.simulate_mouse_move(hover, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
            cx.update(|window, cx| window.draw(cx).clear(cx));
            assert!(
                cx.update(|_, cx| cx.has_active_drag()),
                "tab reveal must retain the original drag"
            );
            assert!(
                target.read_with(cx, |panel, _| panel.is_active),
                "drag-hover must reveal destination tab before the drop"
            );
            assert_eq!(source_done.load(Ordering::Relaxed), 0);
            if direct {
                assert_eq!(
                    target.read_with(cx, |panel, _| panel.tree.leaf_count()),
                    1,
                    "hover must not split"
                );
            }
        }
        if let Some(direction) = direction {
            let selector = target.read_with(cx, |panel, _| {
                format!("space-drop-{}", panel.tree.first_leaf_id().0)
            });
            let bounds = cx.debug_bounds(selector.leak()).unwrap();
            let (x, y) = match direction {
                SplitDir::Left => (0.1, 0.5),
                SplitDir::Right => (0.9, 0.5),
                SplitDir::Up => (0.5, 0.1),
                SplitDir::Down => (0.5, 0.9),
            };
            to = point(
                bounds.left() + bounds.size.width * x,
                bounds.top() + bounds.size.height * y,
            );
        }
        cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
        if direct {
            assert!(
                target.read_with(cx, |panel, _| panel.drop_preview.get().is_some()),
                "direction preview while dragging"
            );
            assert_eq!(target.read_with(cx, |panel, _| panel.tree.leaf_count()), 1);
            assert_eq!(
                target.read_with(cx, |panel, _| panel.drop_preview.get().map(|(_, dir)| dir)),
                direction
            );
            cx.update(|window, cx| window.draw(cx).clear(cx));
        }
        assert!(cx.update(|_, cx| cx.has_active_drag()));
        cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert!(!cx.update(|_, cx| cx.has_active_drag()));
        if fraction < 0.5 {
            assert!(dock.read_with(cx, |dock, _| dock.panel(source_id).is_some()));
            assert!(target.read_with(cx, |panel, _| panel.tree.leaf_terminal(empty).is_none()));
        }
    }
    assert_eq!(
        target.read_with(cx, |panel, _| {
            assert_eq!(panel.tree.leaf_count(), if same_tab { 1 } else { 2 });
            panel
                .tree
                .leaf_terminal(if direct { panel.tree.active() } else { empty })
                .unwrap()
                .id()
        }),
        identity
    );
    assert_eq!(
        dock.read_with(cx, |dock, _| dock.panel(source_id).is_none()),
        !same_tab
    );
    assert_eq!(
        source.read_with(cx, |panel, _| panel.has_no_terminals()),
        !same_tab
    );
    assert_eq!(
        source_done.load(Ordering::Relaxed),
        0,
        "move is not shutdown"
    );
    cx.update(|window, cx| {
        assert_eq!(
            InputChannelRegistry::global(cx)
                .read(cx)
                .channel_of(identity),
            Some(InputChannel::C)
        );
        assert!(source_pane.focus(cx).is_focused(window));
    });
    target.update_in(cx, |panel, window, cx| panel.shutdown(window, cx));
    source.update_in(cx, |panel, window, cx| panel.shutdown(window, cx));
    drop((source_events, target_events));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while source_done.load(Ordering::Relaxed) == 0 || target_done.load(Ordering::Relaxed) == 0 {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(20));
        cx.run_until_parked();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
