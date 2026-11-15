# Work: Coordinated native SSH resize

ID: US-0226
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
- Spec Intake: IN-0057; depends on US-0224/0225.

## Outcome

NativeChannel coordinates Ghostty and SSH dimensions through its one owner,
submitting remote window-change before capturing the resized frame.

## Scope

- [x] In scope: validated resize admission in the existing 64-slot control/input
  FIFO, native resize, original transport pending-resize drain, capture ordering,
  failure/close handling and real channel proof.
- [x] Out of scope: production selection, GUI mapping, remote acknowledgment,
  lifecycle events, local owner integration, SFTP/tunnel/jump ownership.

## Acceptance

- [x] Real peer observes requested dimensions; a fresh frame has matching native
  rows/columns; multiple queued resize/input requests preserve owner FIFO order.
- [x] Zero/over-budget dimensions are rejected before admission without stopping
  the owner. Full/closed admission is typed and does not retry.
- [x] UI pressure defers both native and physical resize; release resumes them;
  close discards queued resizes without changing last-good or emitting resize.
- [x] Physical resize failure returns its cause and stops the owner without
  publishing a resized frame or replaying the operation.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`; agent style,
  structure, crate dependency rules, dependencies and error policy.
- IN-0057 HLD/LLD, US-0224/0225, DEC-0023; terminal async pump resize tests,
  SSH transport coalescing, owner driver and real window-change fixture.

### Documentation Action

Update required: native LLD, probe README and US-0208 handoff. Expose the existing
render size-budget validator for backend pre-admission checks. No persistence,
shipping session or legacy transport semantics change.

### Reconciliation

Updated native LLD, probe README and US-0208 handoff. Exposed the existing
viewport-budget validator; no new Rust dependency or native ABI. Legacy shipping
session and transport code remains unchanged. The original transport-only resize
error is removed because the owner now services pump-generated resize markers.

## Context

Resize shares the physical-input FIFO, not a new queue or transport-only API.
Owner dispatch occurs at batch boundaries. Pump resize mutates native state and
enqueues the original transport resize marker. The driver may service that
marker while awaiting pump work; an explicit final pending-resize drain ensures
window-change submission precedes capture even when the pump future wins select.
The final drain first services the bounded snapshot of already queued commands,
preserving earlier encoded input before taking any marker-suppressed pending
size. Transport markers can coalesce; endpoint control requests remain FIFO and bounded.
Raw writes may overtake queued controls, as in US-0225. Window-change submission
is not acknowledgment or a guarantee the remote application has resized. Failure
or cancellation closes the stream rather than rollback/replay. Last-good remains
until successful capture; previously published frames/events are not rolled back.

## Plan

- [x] Add pre-admission validation and coordinated owner resize.
- [x] Prove real sizes, FIFO, pressure/close and typed failure paths.
- [x] Run native suites, Clippy, full gate and documentation checks.

## Decisions

DEC-0023; no shadow engine or shipping selection.

## Verification Plan

Real authenticated loopback owner tests with deadlines; targeted writer-error
fixture; native SSH/terminal suites and all-target Clippy; ci-local.ps1 and
doc-path/English/dependency/diff checks. Preserve unrelated gate failures.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11:

- Native SSH: 129 tests pass, including 18 owner fixtures (six new).
- Native terminal: 313 tests pass.
- Native SSH/terminal all-target Clippy with -D warnings: pass.
- Doc-path, English contributor text (1230 files), dependency graph and
  `git diff --check`: pass.
- ci-local.ps1: fmt and both workspace Clippy variants pass. Workspace tests
  stop at core shell.rs:916, unchanged PowerShell Enter guard: 92 pass/1 fail,
  empty verdict vs INSTALLED:AcceptLine. Subsequent gate steps not reached.

Real loopback observes 40x6, Enter, 80x12 in that order and final frame 80x12.
Invalid zero/over-budget dimensions preserve live state and last-good. UI pressure
defers controls; raw probe still passes; release resumes remote resize/capture.
Full 64-slot resize queue closes without remote resize and retains last-good.
The disconnected-writer fixture prequeues resize before reading EOF, executes the
real run/teardown path, returns Error::Ssh, closes admission/native and preserves
the old frame. The barrier fixture covers both queued and suppressed resize
markers, prior bytes before size, zero byte budget and harmless stale markers.
It guards against a direct pending-size drain overtaking queued encoded input.

These are native endpoint/transport proofs, not GUI/session integration or remote
acknowledgment. Physical close/native destruction still have no deadline promise.
The dead-handle setup initially expected successful task completion, but russh
reports intentional disconnect as an error; the fixture now explicitly expects
that before testing the dead writer. Full acceptance remains gate-blocked.
Harness tool unavailable; Markdown record used.

## Handoff

Lifecycle and native session/local-loop integration remain before cutover.
