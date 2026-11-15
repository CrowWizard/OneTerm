# Work: Native SSH lifecycle delivery

ID: US-0227
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
- Spec Intake: IN-0057; depends on US-0224/0225/0226.

## Outcome

The native SSH owner sends ordered reliable Exited and Closed events, caches
observed exit status, and explicitly reports lifecycle cancellation when endpoint
close interrupts backpressure rather than hanging or claiming delivery.

## Scope

- [x] In scope: native pump exit publication, native owner lifecycle ordering,
  final Closed after teardown, typed delivery outcome, cancellation/error tests.
- [x] Out of scope: shipping selector/session integration, peer-close detection
  during blocked batches, local owner loop, jump/SFTP/tunnel ownership and GUI.

## Acceptance

- [x] A real peer exit updates cached exit status/alive and sends Exited before
  Closed, preserving already received output and allowing data after exit status.
- [x] Normal Closed delivery waits for bounded UI capacity after physical/native
  teardown; state is dead and writes/controls are rejected while delivery waits.
- [x] Explicit close/Drop/cancelled wait wakes blocked exit/final delivery;
  Report distinguishes Complete from Cancelled without replay or hidden buffers.
- [x] Event loss fails with a typed error; preexisting native/SSH errors retain
  precedence over any final lifecycle delivery error.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`; agent code style,
  structure, dependency rules, dependencies and runtime error policy.
- IN-0057 HLD/LLD, US-0224/0225/0226, DEC-0023 and `docs/terminal-backend.md`.
- Shipping SSH task and pump lifecycle order; SharedSessionState record_exit;
  SessionEventSink bounded/checked policy; native operation cancellation and tests.

### Documentation Action

Update required: native LLD, probe README and US-0208 handoff. No persistence,
shipping session selector or legacy lifecycle behavior changes.

### Reconciliation

Updated native LLD, probe README and US-0208 handoff. The sink's checked send is
public under ghostty-native for backend lifecycle adaptation; legacy builds and
shipping lifecycle remain unchanged. Endpoint close also signals its token when
transport admission has already closed, allowing final-delivery cancellation.

## Context

Native pump publish_exit flushes prior reliable events, records the observed
status/dead cache and waits to send Exited without capture. The SSH driver still
services writes/explicit close while publication waits, then continues reading
late data until EOF/close. Cancellation of a polled exit operation fails native
admission closed and retains last-good, as with existing batch cancellation.
Final Closed is sent after command/control admission closure, budget drain,
physical channel close and native destruction, including error paths. It is
backpressured but cancellable by the existing endpoint token. If queue space is
immediately available, delivery wins over cancellation; otherwise cancellation
returns an explicit Cancelled report outcome. Earlier cancelled exit delivery
also keeps that report outcome even if Closed subsequently fits. No retry or
unbounded pending-event task. The direct connection handle remains owned until
the task result, including final UI wait; explicit close wakes that wait. Original
operation errors take precedence over final delivery errors. No remote EOF
monitor is added while a native batch awaits UI capacity.

## Plan

- [x] Add native exit publication and final reliable lifecycle adaptation.
- [x] Prove ordered exit/late output, final delivery backpressure, explicit close,
  cancellation and error precedence against real SSH channels.
- [x] Run native suites, Clippy, full gate and documentation checks.

## Decisions

DEC-0023; explicit close may abort pending delivery, never silently claim success.

## Verification Plan

Native pump lifecycle tests; authenticated loopback owner fixtures with deadlines;
native SSH/terminal suites and all-target Clippy; ci-local.ps1 and doc-path,
English, dependency and diff checks. Record all unrun/blocked platform proof.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11:

- Native SSH suite: 136 pass, including 25 owner fixtures (seven new).
- Native terminal suite: 316 pass, including 20 async pump fixtures (three new).
- Native SSH/terminal all-target Clippy with -D warnings: pass.
- Doc-path, English contributor text (1231 files), dependency graph and
  `git diff --check`: pass.
- ci-local.ps1: fmt and both workspace Clippy variants pass. Workspace tests
  stop at unchanged core shell.rs:916 PowerShell Enter guard: 92 pass/1 fail,
  empty verdict vs INSTALLED:AcceptLine. Subsequent gate steps not reached.

Real loopback proves Exited(9), late title/output and Closed ordering; status 7
then signal clears cache and sends Exited(None). Pump proof flushes prior title/
bell before exit without capture; late output still paints. Under final UI
pressure, peer channel closure/native Closed are observed before releasing the
queue; admission rejects writes/resize and wait remains pending. Release sends
Closed exactly once. Explicit close returns Cancelled with no replay; idle close
with available capacity returns Complete and sends Closed. Drop and cancelling
a polled wait both finish the actual owner task under final pressure (observed
without abort). Blocked exit close retains cache/last-good and marks lifecycle
Cancelled. Lost final event is typed; native error and physical resize error
fixtures preserve original causes even when final Closed also fails.

No native handle/channel deadline, independent peer-close detection during UI
pressure, production session/local-loop integration or GUI proof is claimed.
Direct connection handle ownership lasts until the task result; native/channel
teardown precedes final UI wait. Full acceptance remains gate-blocked. Harness
tool unavailable; Markdown record used.

## Handoff

Native session/content/local-loop and remaining parity still precede cutover.
