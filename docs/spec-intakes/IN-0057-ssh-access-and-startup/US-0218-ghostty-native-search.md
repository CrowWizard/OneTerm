# Work: Ghostty incremental native search

ID: US-0218
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

Drive native search incrementally on the same owner worker and return owned
screen-space results, never native references or fabricated RowIds.

## Scope

- [x] In scope: bounded needle/results, set/step/clear, exact native coordinates,
  screen ownership, output/reflow invalidation and pump request serialization.
- [x] Out of scope: production search UI/cutover, regex, claiming native search's
  soft-wrap/case behavior is equivalent to the legacy matcher.

## Acceptance

- [x] Native handle is created/freed on its terminal worker; refs never escape.
- [x] A step performs one feed and at most one tick; completion is explicit.
- [x] Query and result limits reject explicitly, without partial result lists or
  closing healthy sessions. Unsupported case/whole-word flags are rejected.
- [x] Owned result coordinates are scoped to one response/screen, not stable ids;
  refreshed results follow history, reflow, reset and screen switches.
- [x] Search does not mutate terminal selection or publish ahead of a batch.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`, agent style,
  structure, dependency and error policy; IN-0057 HLD/LLD, DEC-0023, US-0216/0217.
- `terminal/src/model.rs`, native owner/pump/projection and pinned `search.h`,
  selection/grid-ref headers; existing `vt/src/search/mod.rs` option semantics.

### Documentation Action

Update required: LLD, probe README, US-0208 handoff. Preserve production facts.
Native matching is ASCII-case-insensitive and may span soft wraps; do not silently
map unsupported legacy options or use screen coordinates as durable history ids.

### Reconciliation

Updated LLD, probe README and US-0208. Shipping search/model and PROJECT remain
unchanged because unsupported legacy options/soft-wrap behavior block cutover.

## Context

No second parser. Search owns native tracked state separately from caller anchors.
Bound query to 4096 bytes and collected matches to 4096; no native unbounded run.

## Plan

- [x] Bind/guard native search ABI and implement serialized incremental requests.
- [x] Exercise history, Unicode, bounds, lifecycle, modes and publication.
- [ ] Run focused proof/full gate and reconcile records.

## Decisions

DEC-0023. Unsupported option errors retain parity as a production blocker.

## Verification Plan

C ABI/standalone runner, native terminal/view tests and Clippy, complete ci-local,
doc paths/English/graph/whitespace. No actual GUI/SSH claim.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11: C ABI/runtime probe and 66 standalone tests pass. Three new
native search fixtures prove owned Unicode/ASCII-insensitive results, history/
reflow/screen/reset refresh and recoverable needle/result/option limits/clear.
One pump fixture proves ordering, no publication/writes and healthy recovery.
Whole native terminal suite: 295 pass; native terminal/view all-target Clippy and
9 renderer frame tests pass. Full ci-local passes fmt/both workspace Clippy then
fails unchanged core PowerShell Enter guard at shell.rs:916 (92 passed/1 failed,
empty default verdict instead of INSTALLED:AcceptLine); later steps not reached.
Not fully accepted.
Final documentation checks pass: doc paths, English contributor text, dependency
graph and git diff whitespace. The full gate remains blocked as recorded above.
Native internal result memory is not limited by the response
cap; no wall-clock native deadline, navigation/highlight or old option parity is
claimed. Harness unavailable; Markdown only, no database proof/status updates.

## Handoff

US-0219/0220 own asynchronous access and shared native model consumption.
