//! Terminal panel — each tab is one Terminal.

pub(crate) mod completion;
pub(crate) mod highlight;
pub(crate) mod input;
#[cfg(feature = "ghostty-native")]
mod native_pane;
#[cfg(feature = "ghostty-native")]
pub use native_pane::{NativePaneCompletion, NativeTerminalPane};
#[cfg(feature = "ghostty-native")]
mod native_surface;
#[cfg(feature = "ghostty-native")]
pub use native_surface::{NativeSurfaceError, NativeTerminalSurface};
pub(crate) mod panel;
pub(crate) mod render;
pub(crate) mod security;
mod session_cleanup;
pub use session_cleanup::{close_unplaced_session, close_unplaced_session_in_workspace};
pub(crate) mod space;
pub(crate) mod status;
pub(crate) mod terminal_view;
pub(crate) mod theme;
pub(crate) mod url;

pub use panel::{PanelSpec, TerminalPanel};
pub use security::terminal_security_policy;
pub use status::{find_in_active_terminal, new_terminal_with_shell_cmd, status_metrics};

use gpui::{App, KeyBinding, NoAction};
use gpui_component::dock::{panel_handle, register_panel};
use oneterm_state::panel_names;

/// Initialize the terminal feature.
///
/// Registers the terminal dock panel so saved layouts deserialize. Called by
/// the app aggregator, which also passes [`status_metrics`] to
/// `AppServices::install`.
pub fn init(cx: &mut App) {
    // Both native and legacy terminals own Tab; forms retain focus traversal.
    cx.bind_keys([
        KeyBinding::new("tab", NoAction {}, Some("Terminal")),
        KeyBinding::new("shift-tab", NoAction {}, Some("Terminal")),
    ]);
    panel::override_strip_menu(cx);
    register_panel(cx, panel_names::TERMINAL, |context, window, cx| {
        panel_handle(TerminalPanel::open(
            PanelSpec::DefaultShell {
                workspace: Some(context.dock_area().entity_id()),
            },
            window,
            cx,
        ))
    });
}

#[cfg(test)]
mod key_binding_tests;
#[cfg(feature = "ghostty-native")]
mod native_workspace_sessions;
#[cfg(feature = "ghostty-native")]
pub use native_workspace_sessions::{
    NativeConnectionGuard, NativeWorkspaceCompletion, NativeWorkspaceSessions,
};
