# Work: Ghostty asynchronous reply-first pump

ID: US-0221
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

Drive the native async owner through a transport-owning staging pump, enforcing
reply-first batch publication and reliable-event backpressure without blocking
the executor. Cancellation of an active pump operation stops the session rather
than abandoning consumed output or publishing a partial batch.

## Scope

- [x] In scope: chunked feed, bounded pending events, async reliable delivery,
  prepared capture, resize/key/mouse/scroll, cancellation and native join.
- [x] Out of scope: production backend read loops/TerminalSession cutover,
  clipboard callback adaptation, missing OSC/graphics/search option parity.

## Acceptance

- [x] Replies enqueue on the originating transport before events/capture, with
  no retry of consumed output and no empty transport writes.
- [x] Reliable events retain policy/order and await queue capacity before frame
  publication; closed queues fail-stop instead of silently losing the batch.
- [x] Dropping a polled operation closes transport/admission immediately, prevents
  later publication, and retains last-good frames. Drop never blocks on join.
- [x] Explicit shutdown closes admission/transport before its future is polled,
  cancels unflushed events without publication, and awaits native destruction.
  Graceful close flushes pending reliable events without capture/repaint.
- [x] Bounds and invalid local input reject before mutation; transport/native or
  pending-event overflow errors stop the pump. Readers stay independent.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md` and agent style,
  structure, crate rules, dependency and error policies.
- IN-0057 HLD, `low-level-design/ghostty-engine.md`, DEC-0023, US-0219/0220;
  `docs/terminal-backend.md` remains the shipping-backend contract.
- Native blocking pump/tests, async bridge/model/tests, PtyTransport contract,
  OscRouter, SessionEventSink, production TerminalPump async flush.

### Documentation Action

Update required: LLD, probe README, US-0208 handoff. Shipping model/backend docs
remain valid: no production session or real transport loop selects this pump.

### Reconciliation

Updated LLD, probe README and US-0208 handoff. Shipping backend/PROJECT facts are
unchanged: only fake transports exercise this native staging pump, not a local
PTY or SSH session. No new dependency, native ABI, persisted schema or UI change.

## Context

PtyTransport methods are already nonblocking FIFO admission, not physical I/O.
The pump privately owns AsyncEngine; UI receives only SharedModel. An operation
guard closes on cancellation/error. Native publication and abort serialize on
the publication lock, so abort cannot be followed by a new successful frame.
Already queued reliable events/cache updates cannot be rolled back. Shutdown
is explicit fail-stop, distinct from backpressured graceful close.

## Plan

- [x] Add cancellation-safe pump and guarded bridge publication.
- [x] Prove ordering, backpressure, cancellation, failures and closure.
- [ ] Run focused and complete gates; reconcile evidence and remaining gaps.

## Decisions

DEC-0023; extends US-0210/0219 reply-first/fail-stop policy without new dependency.

## Verification Plan

Native Cargo pump/bridge/model tests with manual Future polls and bounded channel
barriers; full native terminal tests and terminal/view Clippy/frame tests; pinned
C probe/standalone tests; ci-local, doc paths/English/graph and whitespace checks.
No GUI/local/SSH acceptance claim.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11: initial native suite reached 312 tests; one new fixture failed
because its ignored receiver binding dropped the channel before setup. Retaining
that receiver fixed the fixture; all 312 tests then passed. Native terminal/view
all-target Clippy, nine renderer frame tests, pinned C ABI/runtime probe and 66
standalone tests also passed. Final verification after the close/join fixture:

- `cargo test -p oneterm-terminal --features ghostty-native --quiet`: 313 pass,
  including 17 async pump fixtures and 10 async bridge/model fixtures.
- `cargo clippy -p oneterm-terminal -p oneterm-terminal-view --all-targets
  --features oneterm-terminal-view/ghostty-native -- -D warnings`: pass.
- `cargo test -p oneterm-terminal-view --features ghostty-native render::frame
  --quiet`: nine pass.
- `scripts/ghostty-vt/probe.ps1` with pinned Zig/prefix: C ABI/runtime and 66
  standalone native tests pass (the Cargo-only async pump is not in that runner).
- Doc-path, English contributor-text (1220 files), dependency-graph and
  `git diff --check`: pass.

Full ci-local passed fmt and both workspace Clippy variants, then stopped at the
unchanged core PowerShell Enter guard (shell.rs:916, 92 passed/1 failed): empty
default verdict instead of INSTALLED:AcceptLine. Later script steps were not run.
The final post-change ci-local rerun reproduces the same blocker.
This work is not fully accepted. No production local/SSH, GUI, clipboard or paste
bridge proof is claimed. Native operations are not interruptible; graceful close
can wait on UI backpressure, while shutdown/drop never wait for event delivery.
Busy after a pump operation starts is fail-stop, not permission to replay input.
Harness tool is not exposed by catalog search; Markdown only, no database claim.

## Handoff

After staging proof, integrate real session contracts and transport read loops;
preserve simultaneous backend cutover only after remaining parity is proven.
