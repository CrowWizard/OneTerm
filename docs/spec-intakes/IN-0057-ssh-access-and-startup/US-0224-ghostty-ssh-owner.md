# Work: Native SSH channel owner loop

ID: US-0224
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
- Spec Intake: IN-0057; depends on US-0221/0222/0223.

## Outcome

An opt-in native SSH channel owner autonomously reads a real russh channel,
drives AsyncGhosttyPump and services its originating FIFO, including while
native work or reliable UI delivery is pending. Explicit close wakes the owner
independently of queue capacity and UI drain, closes the physical channel and
reclaims abandoned write budget without replay.

## Scope

- [x] In scope: opaque native channel endpoint with write/model/close/wait;
  owner task, bounded commands, cancellation and single teardown; typed result
  with observed exit status, real loopback proof under UI backpressure.
- [x] Out of scope: shipping TerminalSession/factory cutover, resize/input encoder
  command surface, jump hosts/SFTP/tunnels, reliable lifecycle SessionEvents,
  local production loop, native parity and GUI acceptance.

## Acceptance

- [x] Real remote data automatically reaches Ghostty; queued native replies
  reach the peer even when a reliable title event cannot yet be delivered.
- [x] Explicit close wakes under full UI/command queues, aborts active pump
  operation, closes the real channel and retains last-good publication.
- [x] Peer EOF/close and native/event delivery failure share one teardown;
  queued/in-flight writes release budget exactly once, never replay.
- [x] Endpoint Drop requests close; task retains the direct client handle until
  channel teardown; wait reports native destruction and does not block poll.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`; agent style,
  structure, crate rules, dependencies and runtime error policy.
- IN-0057 HLD/LLD, DEC-0023, US-0208/0221/0222/0223; terminal-backend design.
- SSH task/transport/russh channel split API and real-channel fixtures;
  AsyncGhosttyPump cancellation, bridge publication and shared model contracts.

### Documentation Action

Update required: LLD, probe README and US-0208 handoff. Shipping ssh_main_task
and TerminalSession remain unchanged; no production selector is added. The
native endpoint deliberately exposes only supported operations, not fake
TerminalInfo, ordered RowIds or silent resize defaults.

### Reconciliation

Updated LLD, probe README, US-0208 handoff and SSH crate overview. Shipping
session/factory/ssh_main_task remain unchanged. No new Rust dependency or native
ABI change. The native endpoint is an explicit staged direct-channel owner,
not a complete session or permission to cut over the application.

## Context

The channel reader and writer halves remain in one owner task. While a pump
future is pending, a select loop services the same bounded write FIFO, with
an independent CancellationToken for explicit endpoint close. No new writer
task or unbounded output buffer. Native publication means reply admission,
not peer receipt; real tests separately prove peer receipt under UI pressure.
Resize/gestures/lifecycle event adaptation must precede production selection.

## Plan

- [x] Add default-off native owner endpoint and loop.
- [x] Prove autonomous read/reply/close/error/Drop against real channels.
- [x] Run native regressions, all-target Clippy and full gate; reconcile gaps.

## Decisions

DEC-0023; no shadow engine or automatic application cutover.

## Verification Plan

Native SSH owner real loopback tests with deadlines and listener/task cleanup;
native SSH/terminal suites and Clippy; ci-local.ps1 and documentation checks.
Keep known gate failures visible and do not claim lifecycle/GUI parity.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11:

- Native SSH suite: 119 pass, including eight real owner fixtures.
- Native terminal suite: 313 pass.
- Native SSH/terminal all-target Clippy with -D warnings: pass.
- ci-local.ps1: fmt and both workspace Clippy variants pass; workspace tests
  stop at unchanged core shell.rs:916 PowerShell Enter guard, 92 pass/1 fail,
  empty verdict vs INSTALLED:AcceptLine. Later steps not reached.
- Initial owner proof found normal EOF teardown reporting Transport(Closed)
  because the receiver was closed before pump shutdown queued its wake marker.
  Shutdown now signals synchronously before receiver closure; regression passes.
- Doc-path, English contributor-text (1228 files), dependency-graph and
  `git diff --check`: pass.

Tests run on a single-thread tokio runtime with 20-second fixture deadlines.
Full queue close observes physical peer closure without draining UI, no writes
replayed, budget zero, retained last-good; idle close observes native Closed.
Native/event failure retains causal error and last-good. The unpolled write
fixture proves reservation cleanup even if a close-ready select never polls it.
SSH close/native destruction have no interrupt/deadline promise. Lifecycle UI
events, resize/physical input, hops/SFTP/tunnels and production integration are
not claimed. Peer EOF/close is read at normal chunk boundaries, not independently
while blocked by reliable UI delivery; only explicit endpoint close has the
out-of-band wakeup proof. Full acceptance remains blocked. Harness unavailable;
Markdown only.

## Handoff

After owner proof, extend real session contracts and both production loops;
preserve simultaneous cutover only after native semantic/UI parity is proven.
