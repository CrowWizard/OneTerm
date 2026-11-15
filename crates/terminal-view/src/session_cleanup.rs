//! Release connections that outlive their dialog or destination without a pane.

use gpui::BackgroundExecutor;
use oneterm_terminal::CreatedSession;

/// A vanished placement target still contributes to the workspace close proof.
pub fn close_unplaced_session_in_workspace(session: CreatedSession, cx: &mut gpui::App) {
    match session {
        #[cfg(feature = "ghostty-native")]
        CreatedSession::Native(session) => {
            let completion =
                crate::native_pane::close_unplaced(session, cx.background_executor().clone());
            crate::NativeWorkspaceSessions::register_unplaced(completion, cx);
        }
        session => close_unplaced_session(session, cx.background_executor().clone()),
    }
}

/// Observe teardown away from GPUI, including after a window has disappeared.
pub fn close_unplaced_session(session: CreatedSession, executor: BackgroundExecutor) {
    match session {
        CreatedSession::Legacy(session) => {
            if let Err(error) = session.close() {
                log::error!("Unplaced SSH session close failed: {error}");
            }
        }
        #[cfg(feature = "ghostty-native")]
        CreatedSession::Native(session) => {
            crate::native_pane::close_unplaced(session, executor);
        }
    }
    #[cfg(not(feature = "ghostty-native"))]
    drop(executor);
}
