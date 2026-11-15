//! Actual pane handles used by the existing panel and Space tree.

use gpui::{AnyElement, App, Context, Entity, EntityId, FocusHandle, Focusable as _, IntoElement};
use oneterm_core::SessionDuplicateConfig;
use oneterm_terminal::{ProgramState, SessionKind, TerminalCapabilities};

use super::TerminalPanel;
use crate::space::{PaneView, SplitContext};
use crate::terminal_view::TerminalView;

#[derive(Clone, Debug)]
pub(super) enum TerminalPane {
    Legacy(Entity<TerminalView>),
    #[cfg(feature = "ghostty-native")]
    Native(Entity<crate::NativeTerminalPane>),
}

impl From<Entity<TerminalView>> for TerminalPane {
    fn from(view: Entity<TerminalView>) -> Self {
        Self::Legacy(view)
    }
}

impl PaneView for TerminalPane {
    fn entity_id(&self) -> EntityId {
        self.id()
    }
    fn element(&self) -> AnyElement {
        match self {
            Self::Legacy(view) => view.clone().into_any_element(),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => view.clone().into_any_element(),
        }
    }
}

impl TerminalPane {
    pub(super) fn join_channel(&self, channel: oneterm_core::InputChannel, cx: &mut App) {
        match self {
            Self::Legacy(view) => view.update(cx, |view, cx| view.join_channel(channel, cx)),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => view.update(cx, |view, cx| view.join_channel(channel, cx)),
        }
    }
    pub(super) fn leave_channel(&self, cx: &mut App) {
        match self {
            Self::Legacy(view) => view.update(cx, |view, cx| view.leave_channel(cx)),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => view.update(cx, |view, cx| view.leave_channel(cx)),
        }
    }
    pub(super) fn id(&self) -> EntityId {
        match self {
            Self::Legacy(view) => view.entity_id(),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => view.entity_id(),
        }
    }
    pub(super) fn session_alive(&self, cx: &App) -> bool {
        match self {
            Self::Legacy(view) => view.read(cx).session_alive(cx),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => view.read(cx).session_alive(),
        }
    }
    pub(super) fn legacy(&self) -> Option<Entity<TerminalView>> {
        match self {
            Self::Legacy(view) => Some(view.clone()),
            #[cfg(feature = "ghostty-native")]
            Self::Native(_) => None,
        }
    }
    pub(super) fn focus(&self, cx: &App) -> FocusHandle {
        match self {
            Self::Legacy(view) => view.read(cx).focus_handle(cx),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => view.read(cx).focus_handle(cx),
        }
    }
    pub(super) fn title(&self, cx: &App) -> Option<String> {
        match self {
            Self::Legacy(view) => view.read(cx).session.read(cx).title(),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => view.read(cx).title(),
        }
    }
    pub(super) fn cwd(&self, cx: &App) -> Option<std::path::PathBuf> {
        match self {
            Self::Legacy(view) => view.read(cx).session.read(cx).cwd(),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => view.read(cx).cwd(),
        }
    }
    pub(super) fn kind(&self, cx: &App) -> Option<SessionKind> {
        match self {
            Self::Legacy(view) => Some(view.read(cx).session.read(cx).kind()),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => view.read(cx).kind(),
        }
    }
    pub(super) fn status(&self, cx: &App) -> Option<ProgramState> {
        match self {
            Self::Legacy(view) => view.read(cx).program_status(),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => view.read(cx).program_status(),
        }
    }
    pub(super) fn duplicate_config(&self, cx: &App) -> Option<SessionDuplicateConfig> {
        match self {
            Self::Legacy(view) => view.read(cx).duplicate_config.clone(),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => view.read(cx).duplicate_config.clone(),
        }
    }
    pub(super) fn capabilities(&self, cx: &App) -> TerminalCapabilities {
        match self {
            Self::Legacy(view) => view.read(cx).session.read(cx).capabilities(),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => view.read(cx).capabilities(),
        }
    }
    pub(super) fn attach(&self, split: SplitContext, cx: &mut App) {
        match self {
            Self::Legacy(view) => view.update(cx, |view, _| view.split_ctx = Some(split)),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => view.update(cx, |view, _| view.split_ctx = Some(split)),
        }
    }
    pub(super) fn shutdown(&self, cx: &mut App) {
        match self {
            Self::Legacy(view) => view.update(cx, |view, cx| view.shutdown(cx)),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => view.update(cx, |view, cx| view.shutdown(cx)),
        }
    }
    pub(super) fn acknowledge(&self, cx: &mut App) {
        match self {
            Self::Legacy(view) => view.update(cx, |view, cx| view.acknowledge_program_status(cx)),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => view.update(cx, |view, cx| view.acknowledge_program_status(cx)),
        }
    }
    pub(super) fn subscribe(&self, cx: &mut Context<TerminalPanel>) -> gpui::Subscription {
        match self {
            Self::Legacy(view) => cx.subscribe(
                view,
                |_, _, _: &crate::terminal_view::TerminalViewEvent, cx| cx.notify(),
            ),
            #[cfg(feature = "ghostty-native")]
            Self::Native(view) => cx.subscribe(
                view,
                |_, _, _: &crate::terminal_view::TerminalViewEvent, cx| cx.notify(),
            ),
        }
    }
}
