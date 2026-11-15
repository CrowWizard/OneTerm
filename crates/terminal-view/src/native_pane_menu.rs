//! Native menus reuse Space mechanics, never a legacy session or edit adapter.

use gpui::{AnyElement, App, Context, IntoElement, WeakEntity};
use gpui_component::menu::{ContextMenuExt as _, PopupMenuItem};

use super::NativeTerminalPane;
use crate::panel::DuplicateDestination;
use crate::space::SplitDir;

impl NativeTerminalPane {
    pub(super) fn with_context_menu(&self, body: gpui::Div, cx: &Context<Self>) -> AnyElement {
        if !self
            .settings
            .as_ref()
            .is_some_and(|settings| settings.read(cx).show_context_menu)
        {
            return body.into_any_element();
        }
        let pane = cx.entity().downgrade();
        let split = self.split_ctx.clone();
        body.context_menu(move |mut menu, window, cx| {
            let alive = pane
                .upgrade()
                .is_some_and(|pane| pane.read(cx).session_alive());
            menu = menu.item(
                PopupMenuItem::new("New Terminal").on_click(|_, window, cx| {
                    window.dispatch_action(Box::new(oneterm_actions::AddPanel), cx);
                }),
            );
            menu = crate::input::menu::new_ssh_submenu(menu, window, cx);
            if let Some(split) = &split {
                let source = split.clone();
                if alive {
                    let empty = source
                        .panel
                        .upgrade()
                        .map(|panel| panel.read(cx).empty_space_destinations())
                        .unwrap_or_default();
                    menu = menu.submenu("Duplicate Session", window, cx, move |mut menu, _, _| {
                        let destinations = std::iter::once(DuplicateDestination::NewTab)
                            .chain(
                                empty
                                    .iter()
                                    .copied()
                                    .map(DuplicateDestination::ExistingSpace),
                            )
                            .chain([
                                DuplicateDestination::Split(SplitDir::Right),
                                DuplicateDestination::Split(SplitDir::Down),
                            ]);
                        for destination in destinations {
                            let source = source.clone();
                            menu = menu.item(
                                PopupMenuItem::new(crate::input::menu::duplicate_label(
                                    destination,
                                ))
                                .on_click(move |_, window, cx| {
                                    if let Some(panel) = source.panel.upgrade() {
                                        panel.update(cx, |panel, cx| {
                                            panel.duplicate_session_to(
                                                source.space_id,
                                                destination,
                                                window,
                                                cx,
                                            );
                                        });
                                    }
                                }),
                            );
                        }
                        menu
                    });
                } else {
                    menu = menu.item(crate::input::menu::dead_duplicate_item());
                }
                menu = crate::input::menu::split_items(
                    menu.separator(),
                    split.panel.clone(),
                    split.space_id,
                    None,
                    !alive,
                );
            }
            menu = menu.separator();
            for (label, edit) in [
                ("Copy", copy as fn(&NativeTerminalPane, &mut App)),
                ("Paste", paste),
                ("Select All", select_all),
                ("Clear", clear),
            ] {
                let pane = pane.clone();
                menu = menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
                    edit_pane(&pane, edit, window, cx);
                }));
            }
            if let Some(logging) = pane
                .upgrade()
                .and_then(|pane| pane.read(cx).capabilities().logging)
            {
                let running = matches!(
                    logging.state(),
                    oneterm_terminal::TerminalLogState::Running { .. }
                );
                menu = menu
                    .separator()
                    .submenu("Log", window, cx, move |menu, _, _| {
                        let start = logging.clone();
                        let stop = logging.clone();
                        menu.item(
                            PopupMenuItem::new("Start")
                                .disabled(running || !alive)
                                .on_click(move |_, window, cx| {
                                    crate::input::menu::start_logging(start.clone(), window, cx)
                                }),
                        )
                        .item(
                            PopupMenuItem::new("Stop").disabled(!running).on_click(
                                move |_, window, cx| {
                                    crate::input::menu::stop_logging(stop.clone(), window, cx)
                                },
                            ),
                        )
                    });
            }
            menu = menu
                .separator()
                .item(
                    PopupMenuItem::new("Close Terminal Tab").on_click(|_, window, cx| {
                        window.dispatch_action(Box::new(gpui_component::dock::ClosePanel), cx);
                    }),
                );
            if let Some(split) = &split
                && split.panel.upgrade().is_some_and(|panel| {
                    crate::input::menu::can_close_space(panel.read(cx).leaf_count())
                })
            {
                let source = split.clone();
                menu = menu.item(PopupMenuItem::new("Close Space").on_click(
                    move |_, window, cx| {
                        if let Some(panel) = source.panel.upgrade() {
                            panel.update(cx, |panel, cx| {
                                panel.close_space(source.space_id, window, cx)
                            });
                        }
                    },
                ));
            }
            menu
        })
        .into_any_element()
    }
}

fn edit_pane(
    pane: &WeakEntity<NativeTerminalPane>,
    edit: fn(&NativeTerminalPane, &mut App),
    window: &mut gpui::Window,
    cx: &mut App,
) {
    if let Some(pane) = pane.upgrade() {
        pane.update(cx, |pane, cx| {
            edit(pane, cx);
            window.focus(&pane.focus, cx);
        });
    }
}

fn copy(pane: &NativeTerminalPane, cx: &mut App) {
    pane.edit(false, cx);
}
fn paste(pane: &NativeTerminalPane, cx: &mut App) {
    pane.edit(true, cx);
}
fn select_all(pane: &NativeTerminalPane, cx: &mut App) {
    pane.select_all(cx);
}
fn clear(pane: &NativeTerminalPane, cx: &mut App) {
    pane.clear_command(cx);
}
