# Terminal New SSH context menu

Owner amendment 2026-11-15: empty splits offer New SSH Here. Quick Connect and
saved-session authentication carry a weak panel and stable empty Space through
the existing CreatedSession completion seam. Never use the later active Space,
overwrite content or fall back to a new tab. Missing/occupied targets warn and
use existing observed unplaced cleanup. Occupied-terminal New SSH stays new-tab.

## Ownership

A terminal-view shared builder adds New SSH after New Terminal, or New SSH
Here after New Terminal Here for an empty Space. Native pane, legacy terminal
and empty-Space menus call it.
Read saved_ssh_sessions from the installed WorkspaceCommands each time the
context menu opens; do not depend on session-ui or a backend. Quick Connect...
calls open_quick_connect_dialog in a live terminal and open_quick_connect_to
in an empty Space. Saved rows capture a stable id and call
open_saved_ssh_session or open_saved_ssh_session_to, respectively; both
revalidate against the live store. Here callbacks also carry the captured
weak panel and stable SpaceId through existing authenticated completion.

## Behavior and security

Quick Connect... is always the first submenu entry when network actions are
allowed. Group labels and a No saved sessions empty row are inert. Long lists
scroll. Existing connect dialogs retain modal focus; do not refocus the terminal
after opening them. Live-terminal New SSH opens a new terminal tab; New SSH
Here fills only the clicked empty Space, even if focus changes during
authentication. A closed or occupied destination warns and cleans up the
unplaced connection, never replacing content or opening a fallback tab. Preserve unknown-token
denial and administrator explicit-connect policy via existing action checks and
public dialog guards. Menu construction does not connect, persist or read secrets.
No SSH factory, credential, trust, storage or teardown contract changes.

## Verification

Test actual submenu navigation/clicks for Quick Connect and saved stable ids;
empty/grouped/refreshed lists and long-list reachability. Verify native and
legacy/empty menu integration with regression tests, strict native Clippy and
full bundled ci-local. Never dial a saved or external host during tests.
