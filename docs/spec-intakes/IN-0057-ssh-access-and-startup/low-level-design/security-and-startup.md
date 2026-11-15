# Detail Design: security and startup

## Administrator access

Superseded for confirmed administrators by US-0259 / administrator-functional-parity.md
(2026-11-15). The old feature restrictions below apply to Unknown token only.
Retain domain checks, truthful identity and isolated entry startup suppression.

Amend DEC-0019 M1 only: permit explicit Quick Connect, opening saved SSH sessions,
and SFTP operations. Keep delete_session and session_property denied. US-0258
permits new Session creation through the separate locked append transaction in
administrator-session-create.md. Saved duplication, existing group mutations and
startup selection remain disabled; retain the independent snapshot write guard.
Do not change M2-M7, local-shell command resolution, logging policy, vault writes,
or the startup dock-document read policy. Unknown token status keeps the old
network denial, even though a confirmed administrator can use network surfaces.
Refuse SFTP external-editor launches before downloading an edit copy, and enforce
the same restriction in core's editor launcher for custom commands and OS file
associations. Explicit transfers remain allowed. Quick Connect cannot save a new
session, and saved-session connection forms cannot save usernames or passwords.

## Startup persistence

Add optional top-level startup_session_id to the session document (schema v2).
Absent means disabled. Non-u64 values are ignored with a warning, never quarantine
the session list. A dangling id is retained but reported at use, never retargeted.
Removing its session clears the preference in the same snapshot. Save through
the existing single-flight sealed-document writer; no writes on the UI thread.
Identity repairs or legacy-schema migration disable automatic connection rather
than allowing a reassigned id to retarget it.

## Connection lifetime

App defers startup work until the main Root's notification/dialog host exists.
Only a confirmed ordinary process with no pending crash reports needing the
recovery dialog may auto-connect. Clone the selected session
by id and use the existing form's connect closure for one automatic attempt.
Auto-attempt with a saved username and agent/private-key credentials, or when all
password forms use an unlocked saved password. Missing credentials show the form.
After explicit vault unlock, auto-attempt only if every password form is ready.
Cancellation never triggers a delayed connection; no timer or background retry.
A manual connection attempt consumes any pending automatic attempt as well.
Keep existing host-key confirmation, typed failure reporting, connection
cancellation, and successful-password sealing behavior.

## Engine risk

The owner replaced the initial librio-vt target with libghostty-vt. The previous
PTY-coupling finding no longer blocks the chosen engine. The new engine provides
a transport-independent terminal, persistent writes and explicit reply callbacks.
The migration must still prove OSC policy, rendering and transport parity before
cutover; see `ghostty-engine.md`. No security policy moves into native callbacks.
