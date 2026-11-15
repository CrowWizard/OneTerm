# Work: Native local session lifecycle adapter

ID: US-0230
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
- Spec Intake: IN-0057; depends on US-0228/0229.

## Outcome

An explicit native local session uses the same guarded local shell resolution
and implements the existing TerminalLifecycle contract, owns the single event
receiver and exposes native frames/controls without constructing a legacy engine.

## Scope

- [x] In scope: shared shell option construction, explicit security policy,
  single event ownership, identity/liveness/title/cwd, raw/control forwarding,
  repaint-hint release and async teardown observation.
- [x] Out of scope: pretending full TerminalSession support, production factory
  selection, legacy TerminalContent conversion, IME/paste/selection/search API,
  scrollback configuration, logging, platform key mapping and GUI acceptance.

## Acceptance

- [x] Ordinary and native constructors use one guarded resolve_shell/options
  implementation, preserving quoting, resolved env/cwd and elevation checks.
- [x] A real configured native session produces native frames, owns exactly one
  take_events receiver and forwards input/resize/lifecycle without legacy state.
- [x] Caller security policy reaches the native router; no default substitution.
- [x] Session close/Drop/wait retain the independent teardown semantics; no UI
  join, fake capabilities or incomplete TerminalSession implementation.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`; agent code style,
  structure, dependencies, dependency rules and error policy.
- IN-0057 HLD/native LLD, DEC-0023, US-0228/0229 and `docs/terminal-backend.md`.
- LocalSession spawn/options/quoting/elevation tests; TerminalLifecycle,
  TerminalSession/render/input contracts; native local/pump and OscRouter policy.

### Documentation Action

Update required: native LLD, probe README, backend migration note and US-0208
handoff. No persistence, external VT API or production selector changes.

### Reconciliation

Updated native LLD, probe README, backend migration note and US-0208 handoff.
LocalSession now calls the shared resolved_options helper without changing its
engine, guarded fields, quoting, logging identity or factory return type.

## Plan

- [x] Share guarded shell options and pass explicit policy to the native pump.
- [x] Add native lifecycle session adapter and real configuration/control proof.
- [x] Run focused and existing regressions and full gate; doc checks below.

## Decisions

DEC-0023. Do not implement legacy render/input/IME traits with silent unsupported
defaults. This adapter implements TerminalLifecycle only, with explicit typed
native operations until the full native session/content contract is adapted.

## Verification Plan

Real configured native session ACK/resize/events/close plus existing shell spawn
and quoting tests; security propagation test; native local/terminal/SSH suites
and Clippy, ci-local.ps1, doc-path/English/dependency/diff checks. Record known
orphan/PowerShell failures, unsupported full-session behavior and GUI gaps.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11:

- Three native session tests pass: real configured env/cwd and sanitized title/
  OSC7 cwd, single take_events through dyn TerminalLifecycle, native 100x30 resize
  and key ACK, raw write, ordered Exited(7)/Closed; configuration rejection before
  spawning; close plus Drop eventually delivering Closed. Send+Sync is asserted.
- Guarded Options extraction is shared by native and shipping constructors; no
  original config fields are reread after resolve_shell. Existing local quoting,
  shell integration and ordinary session tests pass apart from the two known
  conhost 0x6d orphan failures (default: 33 pass/2 fail/3 ignored).
- Explicit policy forwarding is verified by source/data-flow review: constructor
  calls spawn_with_security which passes its argument to OscRouter::with_security.
  Real title sanitation is exercised. Configurable remote clipboard gates are
  not behaviorally proven by this local adapter; native clipboard parity remains
  missing and no claim of complete OSC/security equivalence is made.
- Native full local: 47 pass/2 same orphan failures/3 ignored; native owner ten
  fixtures and manual ConPTY fixture pass. Native local Clippy, native SSH136 and
  terminal316 pass. ci-local.ps1 passes fmt and both workspace Clippy variants,
  stops at unchanged PowerShell Enter guard shell.rs:916 (92 pass/1 fail; empty
  verdict vs INSTALLED:AcceptLine). Later gate commands are not reached.
- Final native local filter: 14 pass. Doc-path, English text (1238 files),
  dependency graph and diff checks pass. Full gate output retained locally at
  `.tmp/us0230-ci-local.log`; the same PowerShell failure reproduces.

No LocalSession/factory selector switch, full TerminalSession implementation,
production GUI, native configurable scrollback/logging/IME/paste/selection/search
or Unix runtime proof. Event receivers must be drained by their single consumer;
wait deliberately retains reliable backpressure. PTY startup errors remain async
wait errors, while config/native bridge initialization failures are synchronous.
Full acceptance remains gate-blocked. Harness tool unavailable; Markdown used.

## Handoff

Full native render/input/IME and factory integration remain prerequisites for
simultaneous production local/SSH cutover, not implied by lifecycle adaptation.
