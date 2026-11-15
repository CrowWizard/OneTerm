# Work: Autonomous native local PTY owner

ID: US-0228
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
- Spec Intake: IN-0057; depends on US-0221/0223/0227.

## Outcome

An explicit default-off local endpoint owns and autonomously drives the real PTY
and native pump, without a legacy parser or manual test I/O driver.

## Scope

- [x] In scope: raw input FIFO, automatic bounded read/feed/capture, partial
  writes during native/UI waits, child exit, independent close and teardown.
- [x] Out of scope: resize/physical input controls, production session/factory,
  local logging, GUI cutover and cross-platform runtime acceptance.

## Acceptance

- [x] Real ConPTY output and ACK reach native frames autonomously; normal exit
  publishes Exited then Closed after PTY/native teardown.
- [x] UI backpressure does not stop raw FIFO writes or independent close;
  no output backlog grows in Rust while awaiting native/UI.
- [x] Partial writes release only accepted bytes; close releases queued and
  in-flight reservations, rejects new input and preserves last-good frames.
- [x] Drop and cancellation of a polled wait request close without UI joins;
  wait completion observes teardown, not a thread-join or deadline guarantee.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md` and agent code
  style, structure, dependency rules, dependencies and error policy.
- IN-0057 HLD/native LLD, DEC-0023 and `docs/terminal-backend.md`.
- Local event loop/notifier/transport, passive PTY and Windows pipe contracts,
  manual ConPTY fixture, spawn guard, native pump and SSH owner lifecycle.

### Documentation Action

Update required: native LLD, probe README and US-0208 handoff. Shipping contracts
and external oneterm-vt API stay unchanged; no persistence changes.

### Reconciliation

Updated native LLD, probe README and US-0208 handoff. LocalSession and the
shipping event loop are not redirected. NativeLocal is an explicit hidden,
feature-gated export; external oneterm-vt API and dependencies stay unchanged.

## Plan

- [x] Implement an explicit native local endpoint and poll-driven owner thread.
- [x] Add focused partial-write/error proof and real autonomous ConPTY fixtures.
- [x] Run focused suites, native Clippy and full gate; reconcile doc checks below.

## Decisions

DEC-0023. Endpoint creation admits bounded input before child startup; startup
errors are returned by wait. No synchronous startup handshake or UI thread join.

## Verification Plan

Real ConPTY autonomous ACK/normal exit and backpressured close/Drop/cancel tests;
mock partial writes and errors; native local suite and all-target Clippy; complete
ci-local.ps1 plus doc-path, English, dependency graph and diff checks. Record
environment failures separately; forced-close timing is not orphan proof.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11:

- Six native local owner tests pass: autonomous READY/ACK/exit 7, UI-full FIFO
  writer acceptance/close, Drop and cancelled wait, partial writes/errors/exact
  budget reclamation, startup failure and post-teardown Closed backpressure with
  release or explicit cancellation. The original manual ConPTY fixture also passes.
- Full native local suite: 40 pass, 2 fail, 3 ignored. The same existing orphan
  tests fail at session_orphan_tests.rs:252, conhost exit 0x6d at immediate drop
  and first output. No orphan assertion was weakened or ignored.
- Native local all-target Clippy passes. Native SSH 136 and terminal 316 pass.
- Doc-path, English contributor text (1234 files), dependency graph and
  `git diff --check` pass.
- ci-local.ps1: fmt and both workspace Clippy variants pass, then unchanged core
  shell.rs:916 PowerShell Enter guard fails (92 pass/1 fail; empty verdict vs
  INSTALLED:AcceptLine). Later gate steps are not reached.

The initial fixture caught a Windows-specific empty-ring contract: PipeReader
returns zero while live. The owner now waits for the independent child watcher,
not a false EOF. The one-capacity fixture drains startup title events before
intentionally applying UI pressure. Mock writer accepts 3 then 4 bytes in partial
writes, leaves 4 reserved on BrokenPipe, then releases those and a queued 6-byte
payload exactly once. Real final-delivery proof observes native Closed/dead state,
zero input budget and rejected admission before allowing Closed to send; close
instead returns Cancelled without dropping the queued Exited event.

One 64-KiB output buffer is reused, with per-read feed/capture; no performance
claim or extra Rust output queue. Native/UI waits suspend read interest and keep
servicing partial FIFO writes. Writer acceptance is not physical child receipt;
ACK:ping separately proves physical delivery in the normal flow. Close completion
observes resource destruction but does not prove forced-drop orphan cleanup,
thread join, a deadline or Unix semantics. Child-exit draining covers currently
buffered output, not a guarantee of future pipe-thread tail data. Logging,
resize/physical input, production session/content/factory and GUI remain absent.
Full acceptance is gate-blocked. Harness tool unavailable; Markdown packet used.

## Handoff

Resize/physical input, session/content adaptation and remaining parity still
precede simultaneous local/SSH production cutover.
