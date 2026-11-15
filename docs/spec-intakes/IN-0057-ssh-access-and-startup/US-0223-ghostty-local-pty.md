# Work: Ghostty real local PTY roundtrip

ID: US-0223
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
- Spec Intake: IN-0057; depends on US-0221/0222.

## Outcome

Exercise a real Windows PseudoConsole child through LocalTransport/ShellNotifier
and AsyncGhosttyPump, without constructing a legacy terminal for its output.

## Scope

- [x] In scope: default-off native feature forwarding, owner-thread real PTY
  fixture, input FIFO and byte accounting, native paint, resize and child exit.
- [x] Out of scope: shipping ShellEventLoop or session cutover, automatic async
  shutdown while blocked by UI, Unix native build, full console-host VT parity.

## Acceptance

- [x] A real child prints readiness; native frames contain it and its input ACK.
- [x] Original LocalTransport queue delivers input once, releasing byte budget;
  resize coalesces and reaches both PTY child and native frame dimensions.
- [x] Child exits successfully; explicit native shutdown closes notifier
  admission, and registration is removed before owner-thread PTY destruction.
- [x] Bounded poll waits, cleanup guards and shared spawn serialization keep
  failure paths from corrupting the existing process-orphan fixtures.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`, agent style,
  structure, crate rules, dependencies and runtime error policy.
- IN-0057 HLD/LLD, DEC-0023, US-0208/0221/0222 and terminal-backend design.
- local event_loop, notifier/control, transport, session and orphan fixtures;
  vt passive PTY ownership contract and native pumps/frame contracts.

### Documentation Action

Update required: LLD, probe README and US-0208 handoff with actual observations.
Shipping contracts stay unchanged: explicit fixture I/O is not ShellEventLoop.

### Reconciliation

Updated LLD, probe README and US-0208 with actual ConPTY proof and remaining
failure boundaries. No shipping session/backend behavior change. Shared spawn
lock moved from legacy session fixtures into a test-only coordination module;
native and orphan fixtures now share it without widening production APIs.

## Context

ConPTY is a console host, not a transparent Unix byte pipe; native DSR replies
are not guaranteed to originate from a child's query. This slice proves real
child input/output/size/exit, not raw DSR or complete console-host VT parity.
Use the existing spawn lock for native and legacy child fixtures, without
constructing legacy state or widening production APIs for test access.

## Plan

- [x] Add shared test-only spawn coordination and native PTY fixture.
- [x] Run focused native proof and regression/Clippy, then complete gate.
- [x] Reconcile evidence and production integration gaps.

## Decisions

DEC-0023; native handles remain on their dedicated owner worker.

## Verification Plan

Native local fixture, native local all-target Clippy and native terminal tests;
default local regressions; ci-local.ps1; doc-path, English, graph and whitespace.
Report actual platform/fixture failures rather than weakening assertions.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11:

- `cargo test -p oneterm-local-shell --features ghostty-native native_local_pty
  --quiet`: pass, including the final two-input FIFO/closed-native assertions.
- `cargo test -p oneterm-terminal --features ghostty-native --quiet`: 313 pass.
- `cargo clippy -p oneterm-local-shell -p oneterm-terminal --all-targets
  --features oneterm-local-shell/ghostty-native -- -D warnings`: pass.
- Full native local suite: 34 pass/2 fail/3 ignored. Default local suite:
  33 pass/2 fail/3 ignored. Both reproduce the existing orphan failures at
  session_orphan_tests.rs:252, conhost 0x6d for drop before startup/after output.
  No orphan assertion was relaxed or newly ignored.
- `pwsh -NoProfile -File scripts/ci-local.ps1`: fmt and both workspace Clippy
  variants pass; stops at unchanged core shell.rs:916 PowerShell Enter guard,
  92 pass/1 fail, empty verdict vs INSTALLED:AcceptLine. Later steps not reached.
- Doc-path, English contributor-text (1225 files), dependency-graph and
  `git diff --check`: pass.

The successful child roundtrip/normal exit does not prove forced-drop cleanup,
shipping ShellEventLoop, native session/UI parity, openpty or a production
shutdown wakeup while the event sink is backpressured. Native operations can
still wait indefinitely; only fixture I/O waits have a 20-second bound. Work
remains partial and not fully accepted. Harness tool unavailable; Markdown only.

## Handoff

Production native session contracts, both real read loops, async close wakeup,
remaining native semantic parity and actual GUI acceptance remain open.
