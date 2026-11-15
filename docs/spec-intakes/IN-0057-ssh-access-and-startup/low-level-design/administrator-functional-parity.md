# Detail Design: administrator functional parity

Owner approved full parity on 2026-11-15 after warning about persistence, credentials,
configured programs, logging, automatic connection and updates under administrator.
Supersedes US-0258 create-only permission. No OS bypass or domain-check removal.

Keep actual Elevation::{NotElevated, Elevated, Unknown} and truthful title rendering.
is_restricted() becomes the permission predicate for Unknown only. All existing
shared write, network, vault, logging, editor and Shell guards keep consulting it.
Confirmed Elevated and NotElevated use ordinary Session CRUD/password/group,
settings/dock persistence, configured Shell/editor, production startup/update and
crash UI paths. Keep unreadable-file protection, schema validation, encrypted
secrets, atomic lock/backups, vault unlock/idle lock and host/clipboard trust.
Unknown retains the restrictive action allowlist and write/execution refusals.

Identity-sensitive callers must inspect the actual token: separate elevated crash
storage, debug-console detachment and explicit runas startup resolution remain.
The UAC launch menu stays a closed Windows Shell set; ordinary administrator tabs
can run configured/custom programs. Never spoof global elevation.

Retire the special administrator append-only Session branch and use the ordinary
coalesced store writer. Existing snapshot semantics are unchanged; no cross-window
merge or Windows ownership/ACL guarantee is added. Isolated native acceptance
still suppresses production auto-connect/update startup effects. Windows continues
to block Explorer drops across integrity levels. No updater install is run as proof.
