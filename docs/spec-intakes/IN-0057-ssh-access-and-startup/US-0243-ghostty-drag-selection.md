# Work: Native tracked drag selection

ID: US-0243
Intake: IN-0057
Created: 2026-11-12

## Status

<!-- HARNESS:STATUS:BEGIN -->
- [x] Planned
- [x] In progress
- [ ] Implemented
- [ ] Changed
- [ ] Reopened (acceptance rework)
- [ ] Retired
<!-- HARNESS:STATUS:END -->

## Classification

- Change type: existing-contract change
- Risk lane: high-risk (native interaction contract)
- Spec Intake: IN-0057; accepted HLD, native LLD, DEC-0023.

## Outcome

Left drag in the independent native surface creates a native selection, painted
and copyable through the existing pipeline, without stale frame coordinates or
unbounded GPUI tasks/anchors. No production engine cutover.

## Scope

- In: linear left drag, Shift override of mouse reporting, tracked start anchor,
  last-painted viewport provenance, bounded motion coalescing, release/blur cleanup,
  typed errors and real local/SSH session seam.
- Out: word/line/rectangle selection, select-all, off-grid autoscroll, search UI,
  menu/broadcast/settings integration and production panel selection.

## Acceptance

- [x] The session's existing selection FIFO operation accepts gesture updates;
  reliable events precede selection mutation/capture/repaint.
- [ ] A viewport sample refers to the last painted publication, rejected if a
  different source/grid revision or any intervening native grid mutation exists.
  Selection-only captures preserve revision for continuous queued motion. The
  tracked start survives output/reflow; stale new samples never remap silently.
- [x] Left drag selects locally when mouse reporting is off or Shift overrides;
  admitted native mouse presses still receive their once-only release.
- [x] One gesture task and at most one pending motion sample per surface; finish
  cannot be overwritten by motion. Owner holds at most one gesture start anchor,
  temporary end anchors release on success/error, and end releases the start.
- [x] Copy reads the resulting native selection. Release outside/blur retains the
  last valid selection; no invented/clamped padding coordinates or implicit retry.
- [ ] Focused tests, native Clippy, full gate and doc checks reconciled; desktop
  UAT, pre-existing failures and remaining search scope explicit.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`, required agent
  guides, native HLD/LLD, DEC-0023 and US-0208/0234/0240/0242.
- `docs/terminal-backend.md`: selection publication and reporting boundaries.
- Native selection/anchors/bridge/pump, local/SSH selection FIFO, native surface
  pointer/render state and existing native tests.

### Documentation Action

Update native LLD before implementation, then backend notes, probe README and
US-0208 handoff. No schema, dependency or external oneterm-vt API change.

### Reconciliation

Native LLD records revision identity, coalescing and teardown ownership. Backend
notes, probe README and US-0208 handoff now describe this independent surface,
without claiming production cutover. No schema/dependency/public VT API change.

## Plan

- [x] Add owner gesture and provenance guard using existing selection queue.
- [x] Wire bounded native surface drag task and painted sample hit-testing.
- [ ] Prove selection/copy/cleanup/stale samples and reconcile checks.

## Decisions

Native LLD and DEC-0023; selection coordinates are never durable anchors.

## Verification Plan

Native owner/pump tests for start/move/end, stale unpublished output/publication,
tracked start across scroll/reflow, cleanup and bounded anchor reuse. GPUI tests
for local drag/copy, Shift override, reporting/release preservation and pending
motion coalescing/blur. Real local/SSH trait selection operations, native full
terminal/SSH/focused local/surface regressions, all-target native Clippy and
ci-local, doc paths/English/dependency/diff checks.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

2026-11-12 focused native runs passed:

- `cargo test -p oneterm-terminal-view --features ghostty-native native_surface --quiet`:
  9 passed. Includes 1000 motions coalesced to Begin/latest Move/Finish, real native
  selection formatting and clipboard copy, Shift override, outside-up and blur.
  Idle teardown proof drops Drag state inside a live entity, not the entity itself;
  detached Finish completes and all 256 anchor slots can then be allocated.
- `cargo test -p oneterm-ssh -p oneterm-terminal --features oneterm-ssh/ghostty-native --quiet`:
  final run SSH153 and terminal346 passed. Both real endpoint selection tests now
  exercise Begin/Move/Finish through a SurfaceSession trait object and native copy.
  The first SSH run failed at `task::task_tests::the_task_yields_the_engine_to_a_waiting_frame`
  (`crates/ssh/src/task_tests.rs:166`, close rejected as Closed); no related code was
  changed, and the full rerun passed. The final log replaced that first-run log;
  this record preserves the failure rather than claiming an always-green run.
- `cargo test -p oneterm-local-shell --features ghostty-native native_local_ --quiet`:
  25 passed; this is not the full local-shell suite or orphan-test acceptance.
- Native all-target Clippy for terminal-view/local-shell/SSH passed with warnings
  denied. Logs: `.tmp/us0243-{surface,backends,local,native-clippy}.log`.
- Final documentation-path check passed (220 paths), English contributor-text
  check passed (1263 files), dependency graph passed (20 packages), and
  `git diff --check` passed. These do not replace the blocked full gate.
- Pump regression proves foreign-source and unpublished-output rejection,
  selection-only revision stability, tracked start entering history, alternate
  screen rejection, invalid-coordinate recovery, and 300 gestures followed by
  allocation of every anchor slot. Log: `.tmp/us0243-selection.log`.

`pwsh scripts/ci-local.ps1` passed fmt and both workspace Clippy variants, then
stopped at the existing core PowerShell Enter-guard failure: stdout empty instead
of `INSTALLED:AcceptLine`, `crates/core/src/config/shell.rs:916` (92 passed, 1 failed).
Later gate steps did not execute. Log: `.tmp/us0243-ci-local.log`. Harness discovery
previously returned no tool; this packet remains in progress.

Remaining proof gaps: real entity destruction (rather than Drag-state teardown),
explicit drag across resize/reflow, and gesture-specific reliable-event backpressure.
Existing owner anchor/reflow and selection FIFO tests are supporting evidence,
not substitutes for those exact scenarios. Desktop drag/focus/clipboard UAT is
unperformed. Existing DirectWrite glyph and ConPTY orphan failures are not resolved.
Production local/SSH factories remain unchanged; full migration is not accepted.

## Handoff

Next: bounded native search UI/navigation. Full panel/security/parity and desktop
acceptance still precede simultaneous local/SSH production cutover.
