# Work: Native SSH physical input admission

ID: US-0225
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
- Spec Intake: IN-0057; depends on US-0221 and US-0224.

## Outcome

NativeChannel admits bounded, validated physical key/mouse requests and its one
owner encodes them using the current Ghostty state into the original SSH FIFO.

## Scope

- [x] In scope: nonblocking typed admission, bounded input FIFO, native encoding
  at batch boundaries, failure/close cleanup, real channel tests.
- [x] Out of scope: GPUI physical key mapping, resize, paste, clipboard, lifecycle
  adaptation, local loop and production selector.

## Acceptance

- [x] Real peer observes application-cursor and SGR mouse bytes derived from
  remote modes, in admitted input order, without a legacy engine.
- [x] Invalid input is rejected before admission without terminating the owner;
  full/closed input admission returns a typed error and never retries.
- [x] UI pressure bounds pending input; close with a full input queue discards
  pending inputs without encoding/replaying them and retains last-good.
- [x] Existing autonomous owner tests and native terminal regressions pass.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md` and agent style,
  structure, crate dependency rules, dependencies and runtime error policy.
- IN-0057 HLD, `low-level-design/ghostty-engine.md`, US-0224 and DEC-0023.
- Native input validation, AsyncGhosttyPump key/mouse cancellation, SSH owner
  driver and real channel fixtures; no persistence or UI changes.

### Documentation Action

Update required: native LLD, probe README and US-0208 handoff. Expose existing
native validation methods for pre-admission checks rather than duplicate policy.

### Reconciliation

Updated native LLD, probe README and US-0208 handoff. Shipping APIs, persistence,
UI mapping and factory remain unchanged. Existing native validation is now public
so the backend checks policy before bounded admission, without duplicating it.

## Context

One input FIFO of 64 requests is separate from the original byte FIFO. Key text
is at most 4096 bytes, mouse requests are fixed-size. The owner only starts an
input at a completed batch boundary; it does not reenter the pump while delivery
is pending. Raw writes may overtake pending physical inputs; no total ordering
between these two admission surfaces is promised. Encoded bytes and replies use
the original FIFO and byte budget. Admission is not encoding or physical receipt.
Explicit close remains independent of both queues; teardown drops pending input.

## Plan

- [x] Add validated bounded endpoint admission and owner dispatch.
- [x] Prove mode-derived bytes, rejection, saturation and close on real channels.
- [x] Run focused suites, native Clippy and full gate; reconcile evidence.

## Decisions

DEC-0023; no shadow engine or shipping selection.

## Verification Plan

Native owner real loopback fixtures with deadlines; native SSH/terminal suites,
all-target Clippy, ci-local.ps1, documentation/English/dependency/diff checks.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11:

- Native SSH suite: 123 pass, including 12 autonomous owner fixtures (four new).
- Native terminal suite: 313 pass.
- Native SSH/terminal all-target Clippy with -D warnings: pass.
- Doc-path, English contributor-text (1229 files), dependency graph and
  `git diff --check`: pass.
- ci-local.ps1: fmt and both workspace Clippy variants pass. Workspace tests
  stop at unchanged core shell.rs:916 PowerShell Enter guard: 92 pass/1 fail,
  empty verdict vs INSTALLED:AcceptLine. Later gate steps not reached.

Real loopback bytes include application Up (ESC O A), SGR left click at cell
2,2, and Enter in physical-input FIFO order. Invalid oversized/control key text
and NaN mouse geometry are rejected while the owner remains usable. A full
64-slot input queue under UI pressure closes without emitting queued keys and
retains last-good; releasing UI instead resumes input after the batch. A raw
probe write overtakes queued input during pressure, proving the documented
separate ordering boundary. Test deadlines detect hangs, not a runtime deadline
promise. No GUI/platform mapping or production cutover proof. Full acceptance
remains blocked by the gate. Harness tool unavailable; Markdown record used.

## Handoff

Still requires real platform mapping, resize and native session integration.
