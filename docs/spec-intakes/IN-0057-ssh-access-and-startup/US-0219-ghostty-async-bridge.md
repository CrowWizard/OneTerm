# Work: Ghostty asynchronous command bridge

ID: US-0219
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

Await bounded native owner commands without blocking a GPUI/tokio calling thread;
close wakes admission out of band and joins off the caller's executor.

## Scope

- [x] In scope: Cargo-only bounded async-channel dispatcher, typed commands/replies,
  request cancellation semantics, native worker shutdown and last-good publication.
- [x] Out of scope: transport pump/read-loop migration, asynchronous clipboard
  callback reply, UI gestures and production local/SSH selection.

## Acceptance

- [x] Native handles never move threads; all blocking owner calls and joins run
  on dispatcher/owner workers, not in Future poll.
- [x] Bounded queue and payloads; nonblocking admission reports Busy explicitly.
- [x] Dropped response futures do not abandon/poison native worker state; accepted
  commands run at most once. Close rejects queued work and never replays feed.
- [x] Close/disconnect wakes an idle dispatcher without queue space; close is
  awaitable and completion means native destruction/join, not just enqueue.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`, agent style,
  structure/dependency/error rules, IN-0057 HLD/LLD and DEC-0023.
- `ghostty/owner.rs`, pump lifecycle, `backend/event_sink.rs`, existing
  async-channel dependency and frame/projection transactional contracts.

### Documentation Action

Update required: LLD, probe README, US-0208. This bridges owner commands only;
production transport/reliable-event cancellation remains a separate requirement.

### Reconciliation

Updated LLD, README and US-0208 with eight-slot admission, cancellation/delivery,
join and owner-only scope. Production async transport/clipboard contracts remain
unchanged and not yet integrated.

## Context

One blocking dispatcher owns the existing EngineOwner endpoint. Existing native
worker remains the only place native handles exist. No owner feed is accessible
through the shared UI model: raw replies must be consumed by a transport caller
before that caller requests capture. Bridge itself performs no transport I/O.

## Plan

- [x] Add bounded async command/reply dispatcher and out-of-band close.
- [x] Test admission, response cancellation, close and native errors.
- [ ] Verify and reconcile.

## Decisions

DEC-0023; no unsafe Send/Sync implementation or new dependency.

## Verification Plan

Native Cargo tests with manual Future polling/nonblocking admission and close
barriers; native Clippy; full gate/doc checks. Standalone FFI runner unchanged.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11: nine Cargo bridge/model fixtures pass, covering bounded queue
admission, manually polled Pending while worker is blocked, queued cancellation,
out-of-band close, backend Drop with model clones, cancelled interactions,
buffered undelivered DSR failure, invalid input, dispatcher panic, native failure,
projection failure and shared model reads. Nonempty replies/reliable events are
not ordinary cancellable values: abandoning them stops the endpoint. Unclaimed
new anchors are released or cleaned up by shutdown. The bridge is owner-only,
does no transport I/O, and cannot safely substitute for the production pump yet.
295 native terminal tests, native terminal/view all-target Clippy, C probe,
66 standalone tests and 9 renderer tests pass. Full gate passes fmt and both
workspace Clippy variants then stops at unchanged core PowerShell Enter guard
(shell.rs:916, 92 passed/1 failed); later checks unreached. Work remains in progress
and is not fully accepted. No database state/proof claim.
Final doc-path, English, dependency-graph and git diff whitespace checks pass.

## Handoff

US-0220 consumes this endpoint from a shared native model, not a production session.
