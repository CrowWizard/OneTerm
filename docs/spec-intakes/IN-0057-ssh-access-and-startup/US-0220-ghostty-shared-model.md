# Work: Ghostty shared model publication

ID: US-0220
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

Provide a shared native model with asynchronous interactions and preprepared
immutable paint frames; UI reads never invoke native work or allocating projection.

## Scope

- [x] In scope: shared native model endpoint, nonblocking frame reads, independent
  consumer damage, native selection/search/scroll methods through the async bridge.
- [x] Out of scope: pretending legacy TerminalInfo/RowId/mode contracts are met,
  production TerminalSession or backend cutover, UI overlays/gestures.

## Acceptance

- [x] Capture/projection runs on bridge worker and publishes only whole prepared
  frames; failures retain last-good data and explicit lifecycle/error status.
- [x] Shared readers see exact metadata/selection/paint from one capture, with
  independent damage and no fabricated history ids or cached live query defaults.
- [x] Model operations asynchronously reach the same owner state as output;
  model exposes no feed/replies and does not publish on interaction alone.
- [x] Existing shipping model and real GPUI native-row reader regressions pass.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`, agent contracts,
  IN-0057 HLD/LLD, DEC-0023 and US-0214/0217/0218/0219.
- `terminal/src/model.rs`, `content.rs`, `handle.rs`, native owner/projection,
  session traits and terminal-view frame consumption boundary.

### Documentation Action

Update required: LLD, US-0208 and probe README. Keep shipping-engine and actual
local/SSH acceptance open; this model intentionally has no false legacy RowId API.

### Reconciliation

Updated LLD, README and US-0208. Existing model/content/session and PROJECT facts
remain valid: this is an explicit native staging model, not their production
replacement. Its interface contains no fake legacy RowId/TerminalInfo values.

## Context

Prepared frames are shared by Arc. Damage comparison remains staging work, not a
performance claim. Never allocate projection in render prepaint/Future poll.

## Plan

- [x] Publish prepared projection from async bridge and add shared model/reader.
- [x] Test atomic snapshot, independent damage, interaction/output ordering/close.
- [ ] Verify and reconcile.

## Decisions

DEC-0023; actual production contract replacement still requires full parity.

## Verification Plan

Native model integration fixtures, native terminal/view all-target Clippy,
renderer frame tests, complete ci-local and docs/English/graph/whitespace checks.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11: integration fixture feeds actual native output, receives DSR
before capture, checks exact prepared frame Arc, two independent readers, async
anchor/selection/copy/search operations, no publication on interaction alone and
last-good data on close. Separate native/projection-failure fixtures retain the
prior shared frame and expose failure. Existing shipping model fixtures and all
295 native terminal tests pass, as do native terminal/view all-target Clippy,
9 renderer frame tests, C ABI/runtime probe and 66 standalone tests. Readers
perform no native work/projection; their damage row list can allocate, as in the
existing staged reader. No performance or full TerminalSession parity claim.
Full gate passes fmt/both workspace Clippy then stops at unchanged core PowerShell
Enter guard (shell.rs:916, 92 passed/1 failed); later steps unreached. Not fully
accepted. Production pump/session, UI overlays/gestures and legacy search options
remain unimplemented integration, not passing evidence. Markdown records only.
Final doc-path, English, dependency-graph and git diff whitespace checks pass.

## Handoff

Transport reply-first async pump, legacy model contract replacement and complete
graphics/OSC/theme/input parity precede simultaneous production backend cutover.
