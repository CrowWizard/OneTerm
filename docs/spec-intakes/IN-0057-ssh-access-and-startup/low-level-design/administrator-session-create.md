# Detail Design: administrator Session creation

Superseded by US-0259 administrator-functional-parity.md on 2026-11-15. Retire
the create-only runtime path in favor of ordinary Session management. The rest
of this document records the earlier narrow approval.

Owner approved on 2026-11-15. Amend DEC-0019 M1/M4 only for explicit creation
of a new saved Session under a confirmed Elevated token. Unknown remains denied.
Edit, delete, saved duplication, existing group mutation, startup preference,
vault and all other configuration restrictions remain unchanged. New Sessions
may select a group. No password saving permission is added.

Header plus, New Session action and blank-area menu open the existing form.
The dialog checks edit/create policy and displays refusals rather than returning
silently. Empty-state text reflects the create permission.
The global NewSession key binding outside the Session panel intentionally remains
Quick Connect (US-0114); the terminal plus menu's restricted rows are unchanged.
This work changes the Session panel's New Session action and header, not that
pre-existing global shortcut meaning.

Administrator Save uses an independent background append, never add()/save()
or may_persist(). Recheck the actual token and startup persist_blocked flag.
Inside update_secret_json_file's inter-process lock, validate current schema and
identities without repair/quarantine. Missing files initialize schema v2; invalid,
unsupported or migration-needing documents refuse the append. Allocate a checked
nonzero id from the current disk counter and append one row. Preserve existing
JSON fields, opaque sealed values and startup id. Reject exhausted ids and invalid
jump routes. Secret transaction backup semantics keep the new document in .bak,
owner-only on Unix. No path selection or elevation override is introduced.

Only after durable success merge the committed row into the administrator read
model and show success. Failure shows an error without a phantom row. Window
closure does not cancel an accepted filesystem transaction. Concurrent appends
serialize and allocate ids from disk. Ordinary snapshot semantics are unchanged:
this is not a general multi-window merge redesign and does not prevent a later
stale ordinary snapshot overwrite.
