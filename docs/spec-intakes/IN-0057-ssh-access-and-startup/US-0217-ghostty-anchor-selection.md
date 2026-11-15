# Work: Ghostty anchor selection and copy

ID: US-0217
Intake: IN-0057
Created: 2026-11-11

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
- Risk lane: high-risk
- Spec Intake: IN-0057

## Outcome

Set native linear/rectangular selection from real tracked anchors, copy bounded
owned UTF-8 from the active selection, and prepare immutable viewport selection
spans from the same native capture as paint rows. No legacy RowId conversion.

## Scope

- [x] In scope: worker/pump set/clear/copy, fresh worker-confined snapshots,
  native terminal-owned selection tracking, bounded plain formatter output,
  transactional paint projection of selected cells and damage regressions.
- [x] Out of scope: search, word/line gesture interpretation, GPUI mouse routing,
  system clipboard writes/OSC grants, async bridge and production engine switch.

## Acceptance

- [x] Inclusive forward/reverse linear and rectangular selections copy native
  text with soft-wrap joining, trimming, Unicode and reflow behavior verified.
- [x] Native selections outlive released caller anchors and remain owned by the
  correct screen; invalid/foreign/released/inactive-screen endpoints are rejected
  without changing the last valid selection or closing a healthy pump.
- [x] Every untracked grid reference and formatter stays inside a serialized
  worker operation; no native pointer crosses the response/publication boundary.
- [x] Copy is at most 1 MiB UTF-8; overflow returns an explicit recoverable error
  with no partial text. Missing selection is None, selected blank text is Some("").
- [x] Native selected-cell masks prepare exact viewport spans, not invented
  history endpoints, and do not change paint row keys solely for highlighting.
- [x] Selection mutations publish only at explicit batch capture; changed and
  cleared highlights damage the right rows independently for every reader.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`, agent code style,
  structure/dependency/error rules, IN-0057 HLD/LLD, DEC-0023 and US-0208/0216.
- `terminal/src/model.rs`, `ghostty/anchors.rs`, `owner.rs`, `project.rs`,
  `project_tests.rs`, `backend/ghostty_pump.rs`, `terminal-view/src/render/frame.rs`
  and render overlay/state call sites.
- Pinned `selection.h`, `formatter.h`, `terminal.h`, `grid_ref_tracked.h`:
  terminal selection copies and tracks endpoints; formatter selections and grid
  snapshots expire on terminal mutation and must never be retained by callers.

### Documentation Action

Update required: Ghostty LLD, probe README and US-0208 handoff. Current production
engine, gestures and clipboard contracts remain unchanged; retain their no-cutover
facts. Document new copy cap as staging contract, not existing behavior parity.

### Reconciliation

Updated the Ghostty LLD with selection lifetime, bounds and native screen-switch
semantics; updated the probe README and US-0208 handoff with evidence and limits.
PROJECT and production backend/model docs remain accurate: default-off staging
has not replaced either production engine or enabled clipboard I/O.

## Context

Reject inactive/mixed-screen endpoints before snapshotting; a tracked anchor may
belong to an inactive screen even when its coordinate resembles the active one.
After set, native terminal state owns selection lifetime independently of the
caller's anchor tokens. Copy reads only the active screen selection and does not
perform clipboard I/O. Explicit clear affects the active screen only. Formatter
uses plain output, unwrap and trim enabled, freshly configured on every copy.

## Plan

- [x] Bind/guard native selection and formatter ABI.
- [x] Add worker/pump anchor selection, clear and bounded copy operations.
- [x] Prepare exact native selection spans separately from paint row content.
- [x] Prove text, screen/lifetime/security rejection, bounds, damage/publication.
- [ ] Run probes, native tests/Clippy, renderer regressions, full gate/doc checks.

## Decisions

DEC-0023; no second parser/state engine or new clipboard permission.

## Verification Plan

Pinned C ABI value/layout guards; standalone native selection tests; native
terminal suite and terminal/view all-target Clippy; renderer frame regressions;
ci-local.ps1; doc-path, English, dependency graph and whitespace checks.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows focused proof (2026-11-11):

- C ABI/runtime probe and standalone Rust runner: 63 passed, warnings denied.
- `cargo test -p oneterm-terminal --features ghostty-native`: 282 passed.
- Native terminal/view all-target Clippy with warnings denied: passed, including
  the view's own ghostty-native feature.
- `cargo test -p oneterm-terminal-view --features ghostty-native render::frame`:
  9 passed, including real native rows through the existing cell/text reader.
- Seven selection fixtures cover inclusive/reverse/rectangle copy, blank versus
  missing selection, Unicode/soft wrap/reflow, caller release, foreign/released/
  expired/inactive-screen rejection, history erasure, ABI layout and real copy
  output over 1 MiB. Bounded formatter test checks exact-limit success too.
- Two projection fixtures cover exact/disjoint spans, source Arc identity,
  unchanged paint keys, selection/clear damage and independent readers. Pump
  fixture proves recoverable rejection, no transport writes/close, and no early
  publication. Transactional rejection fixtures now use reserved style flags.
- Full `ci-local.ps1`: fmt and both workspace Clippy variants pass; workspace
  tests stop at unchanged core PowerShell Enter guard (92 passed, 1 failed,
  shell.rs:916, powershell.exe default empty verdict versus INSTALLED:AcceptLine).
  Later script steps were not reached. This task is not fully accepted/complete.
- Final reverse-rectangle assertion rerun: all seven native selection tests pass;
  fmt, doc-path, English, dependency-graph and diff-whitespace checks pass.

Initial alternate-screen fixture assumed a homed cursor, then assumed selection
restoration on return. Corrected after pinned Terminal.zig switchScreen inspection:
switch clears the destination selection; fixture explicitly homes alternate text
and verifies native clearing rather than introducing restoration behavior.
Harness tool unavailable; Markdown records only. No database proof/status claim.

## Handoff

US-0208 retains complete gestures/search/async/model integration and cutover.
US-0217 remains in progress because the full gate is blocked; focused behavior is
implemented and verified on the pinned Windows artifact. Production selection
overlays and physical mouse routing are not wired to these spans yet.
