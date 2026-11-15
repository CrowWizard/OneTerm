//! Duplicating a terminal session into a new tab, an empty Space, or a fresh
//! split.
//!
//! A local duplicate re-spawns the recorded shell config with the source's live
//! cwd; an SSH duplicate hands off to the workspace's duplicate-SSH dialog and
//! completes asynchronously. Every failure path releases the session it could
//! not install ([`close_unplaced`]) so a duplicate never leaks a live terminal.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{App, AppContext as _, Context, Window};
use gpui_component::WindowExt as _;
use gpui_component::notification::NotificationType;

use oneterm_core::{LocalShellConfig, SessionDuplicateConfig};
use oneterm_state::commands::SshDuplicateCompletion;
use oneterm_terminal::CreatedSession;
use oneterm_theme::notif_ext::notify;

use super::pane::TerminalPane;
use super::terminal_panel::DEFAULT_TAB_TITLE;
use super::{PanelSpec, TerminalPanel};
use crate::space::{SpaceId, SplitDir};

/// User-facing message when a duplicate's destination disappeared before the
/// session could be installed.
const DESTINATION_GONE: &str = "The duplicate destination is no longer available.";

/// Where a duplicated session should land.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DuplicateDestination {
    NewTab,
    ExistingSpace(SpaceId),
    Split(SplitDir),
}

/// A connected session waiting to be installed somewhere.
struct DuplicatedSession {
    session: CreatedSession,
    title: String,
    duplicate_config: SessionDuplicateConfig,
}

/// A terminal that could not be installed into its destination.
pub(super) enum Unplaced {
    /// A view whose target Space stopped being empty.
    View(TerminalPane),
}

/// Release an unplaced terminal (close the session / shut the view down) and
/// tell the user why it did not appear.
pub(super) fn close_unplaced(
    unplaced: Unplaced,
    kind: NotificationType,
    message: &'static str,
    window: &mut Window,
    cx: &mut App,
) {
    match unplaced {
        Unplaced::View(view) => view.shutdown(cx),
    }
    window.push_notification(notify(kind, message, cx), cx);
}

/// A duplicate starts in the source's live cwd, falling back to the shell's own
/// default when the source never reported one.
fn apply_duplicate_cwd(
    mut config: LocalShellConfig,
    live_cwd: Option<PathBuf>,
) -> LocalShellConfig {
    config.cwd = live_cwd;
    config
}

impl TerminalPanel {
    /// Completion shared by New SSH Here and asynchronous placement.
    pub(crate) fn ssh_here_completion(
        panel: gpui::WeakEntity<Self>,
        space_id: SpaceId,
    ) -> SshDuplicateCompletion {
        Rc::new(move |session, title, duplicate_config, window, cx| {
            let Some(panel) = panel.upgrade() else {
                crate::close_unplaced_session_in_workspace(session, cx);
                window
                    .push_notification(notify(NotificationType::Warning, DESTINATION_GONE, cx), cx);
                return;
            };
            panel.update(cx, |panel, cx| {
                panel.place_duplicate_session(
                    DuplicatedSession {
                        session,
                        title,
                        duplicate_config,
                    },
                    space_id,
                    DuplicateDestination::ExistingSpace(space_id),
                    window,
                    cx,
                );
            });
        })
    }

    /// Create a new tab from the session in `space_id` without changing the source.
    pub(crate) fn duplicate_session(
        &mut self,
        space_id: SpaceId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.duplicate_session_to(space_id, DuplicateDestination::NewTab, window, cx);
    }

    /// Duplicate the session in `space_id` into the requested destination.
    pub(crate) fn duplicate_session_to(
        &mut self,
        space_id: SpaceId,
        destination: DuplicateDestination,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.tree.leaf_terminal(space_id) else {
            return;
        };
        #[cfg(feature = "ghostty-native")]
        let native = matches!(view, TerminalPane::Native(_));
        let (duplicate_config, live_cwd) = { (view.duplicate_config(cx), view.cwd(cx)) };
        let Some(duplicate_config) = duplicate_config else {
            window.push_notification(
                notify(
                    NotificationType::Warning,
                    "This terminal does not provide duplication metadata.",
                    cx,
                ),
                cx,
            );
            return;
        };

        match duplicate_config {
            SessionDuplicateConfig::Local(config) => {
                let config = apply_duplicate_cwd(config, live_cwd);
                let mut create = || {
                    Self::spawn_local_session(&self.deps, config.clone(), cx)
                        .map(CreatedSession::Legacy)
                };
                #[cfg(feature = "ghostty-native")]
                let session = if native {
                    Self::spawn_native_local_session(&self.deps, config.clone(), cx)
                        .map(CreatedSession::Native)
                } else {
                    create()
                };
                #[cfg(not(feature = "ghostty-native"))]
                let session = create();
                let session = match session {
                    Ok(session) => session,
                    Err(error) => {
                        window.push_notification(
                            notify(
                                NotificationType::Error,
                                format!("Failed to duplicate local session: {error}"),
                                cx,
                            ),
                            cx,
                        );
                        return;
                    }
                };
                self.place_duplicate_session(
                    DuplicatedSession {
                        session,
                        title: DEFAULT_TAB_TITLE.to_string(),
                        duplicate_config: SessionDuplicateConfig::Local(config),
                    },
                    space_id,
                    destination,
                    window,
                    cx,
                );
            }
            SessionDuplicateConfig::Ssh(config) => {
                // The SSH dialog collects credentials and connects; the panel
                // may be gone by the time it completes.
                let panel = cx.entity().downgrade();
                let completion: SshDuplicateCompletion =
                    Rc::new(move |session, label, duplicate_config, window, cx| {
                        let Some(panel) = panel.upgrade() else {
                            crate::close_unplaced_session_in_workspace(session, cx);
                            window.push_notification(
                                notify(NotificationType::Warning, DESTINATION_GONE, cx),
                                cx,
                            );
                            return;
                        };
                        panel.update(cx, |panel, cx| {
                            panel.place_duplicate_session(
                                DuplicatedSession {
                                    session,
                                    title: label,
                                    duplicate_config,
                                },
                                space_id,
                                destination,
                                window,
                                cx,
                            );
                        });
                    });
                let commands = oneterm_state::commands::commands(cx);
                (commands.open_duplicate_ssh_dialog)(config, live_cwd, completion, window, cx);
            }
        }
    }

    /// Install a connected duplicate into `destination`, releasing it with a
    /// notification when the destination is gone or no longer empty.
    fn place_duplicate_session(
        &mut self,
        duplicate: DuplicatedSession,
        source_space: SpaceId,
        destination: DuplicateDestination,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let DuplicatedSession {
            session,
            title,
            duplicate_config,
        } = duplicate;

        let target = match destination {
            DuplicateDestination::NewTab => {
                if self
                    .tab_panel
                    .as_ref()
                    .and_then(|tabs| tabs.upgrade())
                    .is_none()
                {
                    self.close_duplicate_session(
                        session,
                        NotificationType::Error,
                        "The terminal tab container is unavailable.",
                        window,
                        cx,
                    );
                    return;
                }
                let spec = match session {
                    CreatedSession::Legacy(session) => PanelSpec::Session {
                        session,
                        title,
                        duplicate_config: Some(duplicate_config),
                    },
                    #[cfg(feature = "ghostty-native")]
                    CreatedSession::Native(session) => PanelSpec::NativeSession {
                        session,
                        title,
                        duplicate_config: Some(duplicate_config),
                    },
                };
                let panel = TerminalPanel::open(spec, window, cx);
                Self::add_tab_to_dock(panel, window, cx);
                return;
            }
            DuplicateDestination::ExistingSpace(target) => target,
            DuplicateDestination::Split(dir) => {
                if self.tree.leaf_terminal(source_space).is_none() {
                    self.close_duplicate_session(
                        session,
                        NotificationType::Warning,
                        DESTINATION_GONE,
                        window,
                        cx,
                    );
                    return;
                }
                self.split_tree(source_space, dir, cx)
            }
        };

        let view = match session {
            CreatedSession::Legacy(session) => {
                let session = cx.new(|_| session);
                TerminalPane::Legacy(Self::new_view(
                    &self.deps,
                    session,
                    Some(duplicate_config),
                    window,
                    cx,
                ))
            }
            #[cfg(feature = "ghostty-native")]
            CreatedSession::Native(session) => {
                let Some(view) =
                    Self::new_native_view(&self.deps, session, Some(duplicate_config), window, cx)
                else {
                    window.push_notification(
                        notify(
                            NotificationType::Error,
                            "Native duplicate event receiver is unavailable.",
                            cx,
                        ),
                        cx,
                    );
                    return;
                };
                view
            }
        };
        if let Err(view) = self.place_view(target, view, window, cx) {
            close_unplaced(
                Unplaced::View(view),
                NotificationType::Warning,
                DESTINATION_GONE,
                window,
                cx,
            );
        }
    }

    fn close_duplicate_session(
        &self,
        session: CreatedSession,
        kind: NotificationType,
        message: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        crate::close_unplaced_session_in_workspace(session, cx);
        window.push_notification(notify(kind, message, cx), cx);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use oneterm_core::{LocalShellConfig, ShellKind};

    use super::apply_duplicate_cwd;

    #[test]
    fn missing_live_cwd_uses_shell_default() {
        let mut config = LocalShellConfig::default();
        config.cwd = Some(PathBuf::from("configured"));
        let duplicate = apply_duplicate_cwd(config, None);
        assert_eq!(duplicate.cwd, None);
    }

    #[test]
    fn local_duplicate_preserves_shell_config_and_replaces_cwd() {
        let mut config = LocalShellConfig::default();
        config.kind = ShellKind::Custom;
        config.program = Some(PathBuf::from("custom-shell"));
        config.args = vec!["--login".to_string()];
        let duplicate = apply_duplicate_cwd(config, Some(PathBuf::from("live")));

        assert_eq!(duplicate.kind, ShellKind::Custom);
        assert_eq!(duplicate.program, Some(PathBuf::from("custom-shell")));
        assert_eq!(duplicate.args, vec!["--login"]);
        assert_eq!(duplicate.cwd, Some(PathBuf::from("live")));
    }
}
