//! Which actions an elevated OneTerm window may run (`DEC-0019` M1, `IN-0043`).
//!
//! Confirmed administrators have ordinary functionality (US-0259). The action
//! classification below remains the fail-closed policy for Unknown tokens.
//! Removing the *rows* that
//! reach a denied operation is not the same as removing the
//! *actions*: a key binding dispatches straight to the handler, and
//! `new_ssh_session` ships bound to `ctrl-shift-n` as a global binding, so the
//! Quick Connect dialog opened in an elevated window with no row involved.
//!
//! So the rule lives here, once, as data — not as a guard repeated in a dozen
//! handlers where the thirteenth is the one that gets forgotten. The table is
//! **exhaustive**: every id in `BINDABLE_ACTIONS` must appear, and
//! `settings-ui`'s `every_bindable_action_is_classified_for_an_elevated_window`
//! fails when one does not. A new action cannot slip through silently; somebody
//! has to decide which side of the elevation policy it is on.
//!
//! This crate is the home because it owns the action vocabulary and sits below
//! every crate that dispatches one — `workspace`, `settings-ui`, `session-ui`
//! and `sftp-ui` are all on layer 3 and may not name each other (R2/R5).

/// Saved-session mutation actions denied for Unknown tokens only.
///
/// Keep sorted, and keep the reason obvious from the id.
const DENIED_WHEN_ELEVATED: &[&str] = &["delete_session", "session_property"];

/// Explicit network operations require a known token status, but not an ordinary token.
const NETWORK_ACTIONS: &[&str] = &[
    "new_ssh_session",
    "open_session",
    "sftp_delete",
    "sftp_download",
    "sftp_new_folder",
    "sftp_open",
    "sftp_properties",
    "sftp_refresh",
    "sftp_rename",
    "sftp_upload_files",
    "sftp_upload_folder",
    "toggle_right_dock",
];

/// Action ids an elevated window may run: the terminal, the window chrome and
/// the local-shell surfaces. Network actions are classified separately above.
///
/// Listed rather than defaulted to "allowed", so the exhaustiveness test has
/// something to check against and a new action fails until it is classified.
const ALLOWED_WHEN_ELEVATED: &[&str] = &[
    "about",
    "close_input_channel",
    "close_panel",
    "close_space",
    // Reopens the active live connection; does not modify saved sessions.
    "duplicate_session",
    "find",
    "join_input_channel_a",
    "join_input_channel_b",
    "join_input_channel_c",
    "join_input_channel_d",
    "join_input_channel_e",
    "leave_input_channel",
    // The idle lock applies to the elevated window too (intake Q5); it writes nothing.
    "lock_oneterm",
    "new_terminal_tab",
    "open_settings",
    "quit",
    "split_down",
    "split_left",
    "split_right",
    "split_up",
    "terminal_clear",
    "terminal_copy",
    "terminal_paste",
    "terminal_select_all",
    // A view overlay; in an elevated window it shows but is not saved (M4).
    "toggle_fps",
    "toggle_gutter",
];

/// Whether `action_id` may run in an elevated window.
///
/// `None` means the id is not in the table at all — a programming error the
/// exhaustiveness test catches. Callers treat it as **denied**: an action nobody
/// classified is not one to run under an administrator token.
pub fn action_classified_for_elevated_window(action_id: &str) -> Option<bool> {
    if DENIED_WHEN_ELEVATED.contains(&action_id) {
        return Some(false);
    }
    if ALLOWED_WHEN_ELEVATED.contains(&action_id) || NETWORK_ACTIONS.contains(&action_id) {
        return Some(true);
    }
    None
}

/// Whether `action_id` may run given this process's elevation.
///
/// Confirmed ordinary/elevated tokens: everything. Unknown: the restrictive
/// table above, with unclassified ids and network operations denied.
pub fn action_allowed_when_elevated(action_id: &str) -> bool {
    action_allowed_for_elevation(action_id, oneterm_core::elevation::elevation())
}

fn action_allowed_for_elevation(
    action_id: &str,
    elevation: oneterm_core::elevation::Elevation,
) -> bool {
    if !elevation.is_restricted() {
        return true;
    }
    if elevation.unknown_reason().is_some() && NETWORK_ACTIONS.contains(&action_id) {
        return false;
    }
    match action_classified_for_elevated_window(action_id) {
        Some(allowed) => allowed,
        None => {
            log::warn!(
                "action {action_id:?} is not classified for an unknown process token; refusing it"
            );
            false
        }
    }
}

/// The ids denied in an elevated window, for the exhaustiveness test.
pub fn denied_when_elevated() -> &'static [&'static str] {
    DENIED_WHEN_ELEVATED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_id_is_classified_exactly_once() {
        for id in DENIED_WHEN_ELEVATED {
            assert!(
                !ALLOWED_WHEN_ELEVATED.contains(id) && !NETWORK_ACTIONS.contains(id),
                "{id} is in both tables"
            );
        }
        for id in NETWORK_ACTIONS {
            assert!(!ALLOWED_WHEN_ELEVATED.contains(id));
        }
        for table in [DENIED_WHEN_ELEVATED, ALLOWED_WHEN_ELEVATED, NETWORK_ACTIONS] {
            let mut sorted = table.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), table.len(), "duplicate id in {table:?}");
        }
    }

    #[test]
    fn an_unclassified_action_is_not_allowed_in_an_elevated_window() {
        assert_eq!(
            action_classified_for_elevated_window("brand_new_action"),
            None
        );
        // Saved-session mutations remain denied.
        for id in DENIED_WHEN_ELEVATED {
            assert_eq!(action_classified_for_elevated_window(id), Some(false));
        }
    }

    #[test]
    fn explicit_network_access_requires_a_known_token_not_an_ordinary_one() {
        use oneterm_core::elevation::Elevation;

        for id in NETWORK_ACTIONS {
            assert_eq!(action_classified_for_elevated_window(id), Some(true));
            assert!(action_allowed_for_elevation(id, Elevation::Elevated));
            assert!(!action_allowed_for_elevation(
                id,
                Elevation::Unknown("test")
            ));
        }
        for id in DENIED_WHEN_ELEVATED {
            assert!(action_allowed_for_elevation(id, Elevation::Elevated));
            assert!(!action_allowed_for_elevation(
                id,
                Elevation::Unknown("test")
            ));
        }
    }

    /// The **other** direction, and the one the packet's own risk 4 names: "a
    /// guard whose condition is wrong in the other direction".
    ///
    /// A typo in the denied table would silently unbind `Ctrl+T`, copy, paste or
    /// the splits in an elevated window — `apply_key_bindings` re-adds only the
    /// allowed ids, so a misclassified one ends with **no binding at all**. An
    /// elevated window that cannot open a tab is a broken window, and nothing
    /// else here would notice.
    #[test]
    fn the_local_terminal_stays_usable_in_an_elevated_window() {
        for id in [
            // What the window is for.
            "new_terminal_tab",
            "split_left",
            "split_right",
            "split_up",
            "split_down",
            "close_space",
            "close_panel",
            // ...and the things a terminal is useless without.
            "find",
            "terminal_copy",
            "terminal_paste",
            "terminal_select_all",
            "terminal_clear",
            // ...and the window chrome, which M1 never touched.
            "open_settings",
            "about",
            "quit",
        ] {
            assert_eq!(
                action_classified_for_elevated_window(id),
                Some(true),
                "{id} must keep working in an elevated window"
            );
        }
    }

    #[test]
    fn an_unelevated_process_may_run_everything() {
        // The process global is `NotElevated` here, so the table is not consulted.
        assert!(action_allowed_when_elevated("new_ssh_session"));
        assert!(action_allowed_when_elevated("brand_new_action"));
    }

    #[test]
    fn confirmed_administrators_have_ordinary_action_permissions() {
        use oneterm_core::elevation::Elevation;
        for id in DENIED_WHEN_ELEVATED
            .iter()
            .chain(NETWORK_ACTIONS)
            .chain(ALLOWED_WHEN_ELEVATED)
            .copied()
            .chain(["unclassified_future_action"])
        {
            assert_eq!(
                action_allowed_for_elevation(id, Elevation::Elevated),
                action_allowed_for_elevation(id, Elevation::NotElevated),
                "{id}"
            );
            assert!(action_allowed_for_elevation(id, Elevation::Elevated));
        }
        assert!(!action_allowed_for_elevation(
            "unclassified_future_action",
            Elevation::Unknown("test")
        ));
    }
}
