# Work: Native terminal copy-on-select and right-click cancellation

ID: BUG-0092
Intake: IN-0057-ssh-access-and-startup
Created: 2026-11-16

## Status

<!-- HARNESS:STATUS:BEGIN -->
- [x] Planned
- [ ] In progress
- [x] Implemented
- [x] Changed
- [ ] Reopened (acceptance rework)
- [ ] Retired
<!-- HARNESS:STATUS:END -->

## Classification

- Change type: bug
- Risk lane: normal
- Spec Intake, when required: IN-0057

## Outcome

Native terminal selection copies to the system clipboard when `copy_on_select`
is enabled, and cancelling a terminal right-click context menu does not paste
or send any right-click input to the shell.

## Scope

- [x] In scope: native selection completion, native clipboard copy, and right-click event propagation.
- [ ] Out of scope: clipboard policy, paste validation, terminal mouse reporting when the menu is disabled, and legacy terminal behavior.

## Acceptance

- [ ] Completing a native drag selection invokes the existing asynchronous copy path when enabled.
- [ ] A right-click with the context menu enabled is consumed by the menu path on release.
- [ ] With the menu disabled, native right-click reporting remains unchanged.
- [ ] Existing legacy copy-on-select and context-menu tests remain valid.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md` — terminal input and clipboard sanitization boundary.
- `docs/spec-intakes/IN-0057-ssh-access-and-startup/low-level-design/native-workspace.md` — native surface right-click ownership and edit entry points.
- `crates/terminal-view/src/input/mouse.rs` — legacy mouse contract and copy-on-select behavior.
- `crates/terminal-view/src/native_surface_pointer.rs` — native pointer and selection ownership.

### Documentation Action

No contract change: the reviewed design already requires native right-click
ownership and defines copy-on-select; this packet records the missing wiring.

## Plan

- [x] Wire native copy-on-select through the existing clipboard edit operation.
- [x] Stop propagation for menu-owned native right-click release.
- [x] Add focused regression coverage and run static checks only.
- [ ] Commit and push to `main`; validate through GitHub Actions.

## Verification Plan

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof — focused native drag-selection test updated but not run locally.
- [ ] Integration proof — deferred to GitHub Actions.
- [ ] E2E proof
- [ ] Platform proof
- [x] Verify command passed — `cargo fmt --all -- --check` and `git diff --check`.
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Local compilation and tests are intentionally skipped per user instruction.
Static formatting, diff, English-text, documentation-path and dependency checks
will be run before upload. Cloud Actions remains the platform verification.
