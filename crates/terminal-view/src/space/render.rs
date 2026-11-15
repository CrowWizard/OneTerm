//! Rendering of the Space tree: nested resizable groups, the bordered frame
//! around each Space, and the empty-Space placeholder (drop target + context menu).

use gpui::{
    AnyElement, App, Axis, Context, Div, ElementId, Entity, EntityId, Hsla,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Render, Role,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div, px,
};
use gpui::{Bounds, Pixels, Point, canvas, fill};
use gpui_component::{
    ActiveTheme as _, ElementExt as _, Icon, Sizable as _,
    dock::{ClosePanel, DragPanel},
    menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem},
    resizable::{h_resizable, resizable_panel, v_resizable},
    v_flex,
};
use oneterm_state::InputChannelRegistry;
use oneterm_theme::icon::AppIcon;
use std::cell::Cell;
use std::rc::Rc;

use super::tree::{SpaceContent, SpaceId, SpaceLeaf, SpaceNode, SpaceTree, SplitDir};
use crate::input::menu::split_items;
use crate::panel::TerminalPanel;
use crate::theme::{channel_chip, channel_color};

/// Layout depends on pane identity and rendering, not a particular engine.
pub(crate) trait PaneView: Clone {
    fn entity_id(&self) -> EntityId;
    fn element(&self) -> AnyElement;
}

impl<V: Render + 'static> PaneView for Entity<V> {
    fn entity_id(&self) -> EntityId {
        Entity::entity_id(self)
    }

    fn element(&self) -> AnyElement {
        self.clone().into_any_element()
    }
}

impl<T: PaneView> SpaceTree<T> {
    /// Render the whole tree for `panel`. `channels` is the panel's registry
    /// handle - passed in because the panel is being rendered and cannot be
    /// read back to reach its own dependencies.
    #[cfg_attr(
        feature = "hotpath-profiling",
        hotpath::measure(impl_type = "SpaceTree")
    )]
    pub(crate) fn render(
        &self,
        panel: WeakEntity<TerminalPanel>,
        preview: Rc<Cell<Option<(SpaceId, SplitDir)>>>,
        channels: Option<&Entity<InputChannelRegistry>>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        render_node(
            self.root(),
            (self.active(), self.is_single()),
            panel,
            preview,
            channels,
            window,
            cx,
        )
    }
}

/// Render `node`. `single` is true when this is the tree's sole leaf (the
/// plain-terminal fast path: no Space chrome).
fn render_node<T: PaneView>(
    node: &SpaceNode<T>,
    state: (SpaceId, bool),
    panel: WeakEntity<TerminalPanel>,
    preview: Rc<Cell<Option<(SpaceId, SplitDir)>>>,
    channels: Option<&Entity<InputChannelRegistry>>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let (active, single) = state;
    let split = match node {
        SpaceNode::Leaf(leaf) => {
            return render_leaf(leaf, (active, single), panel, preview, channels, window, cx);
        }
        SpaceNode::Split(split) => split,
    };

    // Stable element id derived from the split's first leaf id.
    let key = split
        .children
        .first()
        .map_or(0, |child| child.first_leaf_id().0) as usize;
    let mut group = if split.axis == Axis::Horizontal {
        h_resizable(("space-split-h", key))
    } else {
        v_resizable(("space-split-v", key))
    }
    .with_state(&split.state);

    for child in &split.children {
        let el = render_node(
            child,
            (active, false),
            panel.clone(),
            preview.clone(),
            channels,
            window,
            cx,
        );
        group = group.child(resizable_panel().child(el));
    }
    group.into_any_element()
}

/// Render a single leaf Space.
fn render_leaf<T: PaneView>(
    leaf: &SpaceLeaf<T>,
    state: (SpaceId, bool),
    panel: WeakEntity<TerminalPanel>,
    preview: Rc<Cell<Option<(SpaceId, SplitDir)>>>,
    channels: Option<&Entity<InputChannelRegistry>>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let (active, single) = state;
    let id = leaf.id;
    let channel = match (&leaf.content, channels) {
        (SpaceContent::Terminal(view), Some(registry)) => {
            registry.read(cx).channel_of(view.entity_id())
        }
        _ => None,
    };
    // The badge names the channel; the Space border below repeats its colour.
    // Sits in the corner, 5 px from both edges (the owner's placement); it
    // overlaps the scrollbar track, which is empty at the top unless the view
    // is scrolled up.
    //
    // It is the only thing in that corner. `US-0117` put a `#N` chip beside it;
    // the owner removed the chip on 2026-09-17 because the 2px ring below
    // already answers "where does my typing go", and `#N` is a stable `SpaceId`
    // rather than a position, so it paid an opaque patch over the top row of a
    // running shell for a number with no meaning outside the code.
    let badge = channel.map(|channel| {
        channel_chip(channel, ("space-channel", id.0 as usize), cx)
            .absolute()
            .top(px(5.))
            .right(px(5.))
    });
    let content: AnyElement = match &leaf.content {
        SpaceContent::Terminal(view) => {
            let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
            let measured = bounds.clone();
            let drop_panel = panel.clone();
            let preview_panel = panel.clone();
            let shown = preview.clone();
            let moving = preview.clone();
            let color = cx.theme().tokens.drop_target;
            div()
                .id(("space-drop", id.0 as usize))
                .relative()
                .size_full()
                .debug_selector(move || format!("space-drop-{}", id.0))
                .on_prepaint(move |bounds, _, _| measured.set(bounds))
                .on_drag_move(move |event: &gpui::DragMoveEvent<DragPanel>, window, cx| {
                    let valid = preview_panel.upgrade().is_some_and(|target| {
                        TerminalPanel::dragged_by(event.drag(cx), cx).is_some_and(|source| {
                            source != target && source.read(cx).can_drag_terminal()
                        })
                    });
                    if valid && event.bounds.contains(&event.event.position) {
                        moving.set(Some((
                            id,
                            split_direction(event.bounds, event.event.position),
                        )));
                    } else if moving.get().is_some_and(|(space, _)| space == id) {
                        moving.set(None);
                    }
                    window.refresh();
                })
                .on_drop(move |drag: &DragPanel, window, cx| {
                    let direction = split_direction(bounds.get(), window.mouse_position());
                    if let Some(panel) = drop_panel.upgrade() {
                        panel.update(cx, |panel, cx| {
                            panel.handle_split_drop(id, direction, drag, window, cx)
                        });
                    }
                })
                .child(view.element())
                .child(
                    canvas(
                        move |bounds, window, cx| {
                            let (space, direction) = shown.get()?;
                            if !cx.has_active_drag()
                                || space != id
                                || !bounds.contains(&window.mouse_position())
                            {
                                return None;
                            }
                            Some(split_preview_bounds(bounds, direction))
                        },
                        move |_, preview, window, _| {
                            if let Some(bounds) = preview {
                                window.paint_quad(fill(bounds, color));
                            }
                        },
                    )
                    .absolute()
                    .inset_0(),
                )
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .debug_selector(|| "terminal-split-target".to_string()),
                )
                .into_any_element()
        }
        SpaceContent::Empty => render_placeholder(leaf, panel.clone(), window, cx),
    };

    // Fast path: the tab's only Space renders with no border / no activation
    // wrapper — visually identical to the pre-split single terminal. A lone
    // member still gets its badge, so it needs a positioned wrapper.
    if single {
        let Some(badge) = badge else {
            return content;
        };
        return div()
            .id(ElementId::from(("space", id.0 as usize)))
            .relative()
            .size_full()
            .child(content)
            .child(badge)
            .into_any_element();
    }

    let cue = space_border_color(
        id == active,
        channel.map(|ch| channel_color(ch, cx)),
        oneterm_theme::selection_accent(cx),
        cx.theme().border,
    );
    div()
        .id(ElementId::from(("space", id.0 as usize)))
        .relative()
        .size_full()
        // Keep a neutral outer border for separation and reserve a one-pixel
        // inner gutter for the channel colour or the selection. The resize
        // handle may paint over the outer shared edge, but cannot erase this
        // gutter; padding keeps it outside the terminal's content bounds
        // instead of overlaying terminal cells.
        .border_1()
        .border_color(cx.theme().border)
        .p(px(1.))
        .bg(cue)
        // Clicking anywhere in the Space makes it the active Space. Bubble phase:
        // the terminal view handles its own selection first, then this fires.
        .on_mouse_down(MouseButton::Left, activate_space(panel, id))
        .child(content)
        // The cue last, so it paints over the terminal's own edge pixels.
        .children((id == active).then(|| active_cue_ring(cue)))
        .children(badge)
        .into_any_element()
}

/// The active Space's cue: a 2px ring in the cue colour, painted over the
/// Space's own 1px gutter and the outermost pixel of the terminal's content
/// area (`US-0117`, `P20`).
///
/// An absolutely-positioned child is laid out against the **padding box**, so
/// `inset_0` starts *inside* the 1px outer border: the border is never
/// overpainted and stays the neutral separator decision 8 fixed, while the
/// 2px ring covers the 1px gutter plus one pixel of terminal edge.
///
/// An overlay rather than a wider border or a wider padding, because
/// `terminal-split.md` decision 8 fixes the Space frame at 1px outer border +
/// 1px inner gutter and this strengthens the *cue*, not the frame — and because
/// geometry that changed with focus would move the terminal's content box by a
/// pixel on every focus switch, which can cost the grid a column. The ring has
/// no id and no mouse handler, so it creates no hitbox and the terminal below
/// still receives every click.
fn active_cue_ring(color: Hsla) -> Div {
    div().absolute().inset_0().border_2().border_color(color)
}

/// Choose the nearest edge in normalized coordinates, independent of aspect ratio.
fn split_direction(bounds: Bounds<Pixels>, pointer: Point<Pixels>) -> SplitDir {
    let x = (pointer.x - bounds.left()) / bounds.size.width.max(px(1.));
    let y = (pointer.y - bounds.top()) / bounds.size.height.max(px(1.));
    [
        (x, SplitDir::Left),
        (1. - x, SplitDir::Right),
        (y, SplitDir::Up),
        (1. - y, SplitDir::Down),
    ]
    .into_iter()
    .min_by(|a, b| a.0.total_cmp(&b.0))
    .unwrap()
    .1
}

fn split_preview_bounds(mut bounds: Bounds<Pixels>, direction: SplitDir) -> Bounds<Pixels> {
    match direction {
        SplitDir::Left | SplitDir::Right => {
            bounds.size.width *= 0.5;
            if direction == SplitDir::Right {
                bounds.origin.x += bounds.size.width;
            }
        }
        SplitDir::Up | SplitDir::Down => {
            bounds.size.height *= 0.5;
            if direction == SplitDir::Down {
                bounds.origin.y += bounds.size.height;
            }
        }
    }
    bounds
}

/// The 1px frame color of a Space. Only the active Space is highlighted: in
/// its `channel` colour when it is a member, in the theme's active colour
/// otherwise. Every inactive Space keeps the plain border; the badge alone
/// says which channel it is in.
fn space_border_color(
    is_active: bool,
    channel: Option<Hsla>,
    active: Hsla,
    inactive: Hsla,
) -> Hsla {
    if is_active {
        channel.unwrap_or(active)
    } else {
        inactive
    }
}

/// A mouse-down handler that makes Space `id` of `panel` the active Space.
fn activate_space(
    panel: WeakEntity<TerminalPanel>,
    id: SpaceId,
) -> impl Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static {
    move |_, window, cx| {
        let _ = panel.update(cx, |p, cx| p.set_active_space(id, window, cx));
    }
}

/// Render the empty-Space placeholder: centered hint text, a drop target for the dock's
/// `DragPanel` of a terminal tab (a strip tab, a rail row or a collapsed cell), and its own
/// context menu.
fn render_placeholder<T>(
    leaf: &SpaceLeaf<T>,
    panel: WeakEntity<TerminalPanel>,
    _window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let id = leaf.id;
    let number = id.display_number();

    v_flex()
        .id(ElementId::from(("space-empty", id.0 as usize)))
        .role(Role::Pane)
        .aria_label(format!("Empty terminal space {number}"))
        .track_focus(&leaf.focus)
        .size_full()
        .items_center()
        .justify_center()
        .gap_2()
        .bg(cx.theme().background)
        .text_color(cx.theme().muted_foreground)
        .child(
            Icon::new(AppIcon::Terminal)
                .large()
                .text_color(oneterm_theme::brand_accent()),
        )
        .child(format!("Space #{number}"))
        // The likeliest action first, worded exactly as the context-menu row it
        // points at; the two rarer ones keep their mention (`US-0115`, `F25`).
        // "New Terminal Here" spawns the default shell, with no picker: the
        // empty Space is a placement action, and a user who wants a specific
        // shell opens it from the "+" menu and drags the tab in.
        .child("Right-click \u{2192} New Terminal Here")
        .child("or split, or drag a terminal tab here")
        // Clicking the placeholder activates this Space.
        .on_mouse_down(MouseButton::Left, activate_space(panel.clone(), id))
        // Visual affordance while a terminal tab's drag hovers this empty Space, and the drop
        // that moves its terminal in. The dock's own drag is the only one (a strip tab, a rail
        // row and a collapsed cell all start it); another kind of panel gets neither the fill
        // nor a move.
        .drag_over::<DragPanel>(|this, drag, _, cx| {
            if TerminalPanel::dragged_by(drag, cx).is_some() {
                this.bg(cx.theme().tokens.drop_target)
            } else {
                this
            }
        })
        .on_drop({
            let panel = panel.clone();
            move |drag: &DragPanel, window, cx| {
                let _ = panel.update(cx, |p, cx| p.handle_panel_drop(id, drag, window, cx));
            }
        })
        .context_menu(move |menu, window, cx| placeholder_menu(menu, panel.clone(), id, window, cx))
        .into_any_element()
}

/// The empty-Space context menu: New Terminal Here, New SSH Here, the four Split items,
/// Close Terminal Tab, and Close Space (only with a sibling Space).
fn placeholder_menu(
    menu: PopupMenu,
    panel: WeakEntity<TerminalPanel>,
    space_id: SpaceId,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let can_close_space = panel
        .upgrade()
        .map(|p| p.read(cx).leaf_count() > 1)
        .unwrap_or(false);

    let menu = menu.item(PopupMenuItem::new("New Terminal Here").on_click({
        let panel = panel.clone();
        move |_, window, cx| {
            let _ = panel.update(cx, |p, cx| p.new_terminal_here(space_id, window, cx));
        }
    }));
    let menu = crate::input::menu::ssh_here_submenu(menu, panel.clone(), space_id, window, cx);
    let mut menu = split_items(menu, panel.clone(), space_id, None, false)
        .separator()
        // Close Terminal Tab — closes the whole tab (all Spaces).
        .item(
            PopupMenuItem::new("Close Terminal Tab").on_click(move |_, window, cx| {
                window.dispatch_action(Box::new(ClosePanel), cx);
            }),
        );

    if can_close_space {
        menu = menu.item(PopupMenuItem::new("Close Space").on_click({
            move |_, window, cx| {
                let _ = panel.update(cx, |p, cx| p.close_space(space_id, window, cx));
            }
        }));
    }
    menu
}

#[cfg(test)]
mod tests {
    use gpui::hsla;

    use super::space_border_color;

    #[test]
    fn preview_geometry_matches_each_direction() {
        use super::{SplitDir, split_direction, split_preview_bounds};
        use gpui::{Bounds, point, px, size};
        let bounds = Bounds::new(point(px(10.), px(20.)), size(px(800.), px(400.)));
        for (x, y, dir) in [
            (0.1, 0.5, SplitDir::Left),
            (0.9, 0.5, SplitDir::Right),
            (0.5, 0.1, SplitDir::Up),
            (0.5, 0.9, SplitDir::Down),
        ] {
            let pointer = point(
                bounds.left() + bounds.size.width * x,
                bounds.top() + bounds.size.height * y,
            );
            assert_eq!(split_direction(bounds, pointer), dir);
            let preview = split_preview_bounds(bounds, dir);
            assert!(preview.contains(&pointer));
            assert_eq!(
                (preview.size.width / bounds.size.width)
                    * (preview.size.height / bounds.size.height),
                0.5
            );
        }
    }

    #[cfg(feature = "ghostty-native")]
    #[test]
    fn shared_renderer_accepts_native_surface_entity_without_legacy_view() {
        fn accepts_pane<T: super::PaneView>() {}
        accepts_pane::<gpui::Entity<crate::NativeTerminalSurface>>();
    }

    #[test]
    fn selected_space_uses_active_gutter_color() {
        let active = hsla(0.1, 0.8, 0.5, 1.0);
        let inactive = hsla(0.0, 0.0, 0.2, 1.0);

        assert_eq!(space_border_color(true, None, active, inactive), active);
        assert_eq!(space_border_color(false, None, active, inactive), inactive);
    }

    #[test]
    fn member_space_uses_its_channel_color_only_while_selected() {
        let active = hsla(0.1, 0.8, 0.5, 1.0);
        let inactive = hsla(0.0, 0.0, 0.2, 1.0);
        let channel = hsla(0.6, 0.7, 0.6, 1.0);

        assert_eq!(
            space_border_color(true, Some(channel), active, inactive),
            channel
        );
        assert_eq!(
            space_border_color(false, Some(channel), active, inactive),
            inactive
        );
    }
}
