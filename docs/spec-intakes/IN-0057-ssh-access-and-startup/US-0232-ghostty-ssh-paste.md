# Work: Acknowledged native SSH paste

ID: US-0232
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
- Spec Intake: IN-0057; native LLD; depends on US-0224/0225/0226 and US-0231.

## Outcome

The explicit native direct SSH endpoint supports acknowledged paste using live
native mode and the shared paste policy, bounded control admission and independent
close on post-admission cancellation, without a production selector change.

## Scope

- [x] In scope: async NativeChannel paste, one existing 64-slot control FIFO,
  separate 4-MiB pending/active text budget, typed acknowledgment and RAII cleanup.
- [x] Out of scope: authentication/trust policy, jump carriers, SFTP/tunnel
  ownership, full TerminalSession, IME, clipboard OSC, production factory and GUI.

## Acceptance

- [x] Paste uses live native mode/shared 1-MiB policy and one original transport
  FIFO payload, not a side writer or a stale frame; no capture is required.
- [x] Future construction does not enqueue; policy, queue and text-budget
  rejections before admission are recoverable and retain no reservation.
- [x] Pending/active paste text is bounded at 4 MiB alongside the 64 control
  slots. Success, error, teardown and cancellation release each reservation once.
- [x] Post-admission cancellation requests independent close despite full UI or
  command queues; bytes already enqueued cannot be retracted and are not replayed.
- [x] Acknowledgment means original FIFO enqueue, not peer receipt. Transport
  rejection after native lookup returns its typed cause and fails closed.
- [x] Paste respects control order; raw writes may overtake waiting controls.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`; agent code style,
  structure, dependencies, dependency rules and error policy.
- IN-0057 native LLD, DEC-0023, US-0231 and `docs/terminal-backend.md`.
- NativeChannel read/drive/resize/teardown and authenticated loopback fixtures;
  SshTransport byte budget/close admission; shared native paste and local guards.

### Documentation Action

Update required: native LLD, backend migration note, probe README and US-0208
handoff. No production paste/auth/trust, persisted schema or external VT API change.

### Reconciliation

Updated native LLD, backend migration note, probe README and US-0208 handoff.
No authentication/trust, original SshTransport behavior, dependencies, production
TerminalSession/factory or external VT contract changes. The same shared paste
policy and native query from US-0231 remain authoritative.

## Plan

- [x] Add bounded acknowledged native SSH paste with cancellation guard.
- [x] Prove authenticated loopback bytes/order, mode/filter, pressure/rejection,
  cleanup, cancellation and transport failure.
- [x] Run focused regressions, native Clippy and full gate; doc checks below.

## Decisions

DEC-0023 and the native LLD. Preserve direct-connection lifetime and original
transport FIFO. No retry, physical-delivery promise or native preemption deadline.

## Verification Plan

Authenticated real loopback tests, shared pump tests and native local regressions;
native SSH/local all-target Clippy; ci-local.ps1, doc-path/English/dependency/diff
checks. Preserve existing PowerShell/orphan failures without weakening assertions.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11:

- Four new SSH tests pass. Authenticated direct loopback proves plain CRLF/LF
  normalization without bracketed-only filtering, remote mode enable/disable,
  bracketed marker/ESC/ETX filtering, empty plain no-write, empty bracketed markers,
  exact last-good pointer/no paste capture or UI event, oversize recovery and
  closed admission with no retained text reservation.
- UI-blocked loopback queues four 1-MiB pastes, rejects the fifth without closing,
  fills all 256 raw commands, cancels an admitted paste and observes independent
  Requested/Cancelled teardown, peer Closed with no replay and both budgets zero.
- UI-release loopback proves queued resize40x6, paste text, Enter, resize80x12
  order; raw probe overtakes waiting controls while native batch is backpressured.
  Acknowledgment releases the paste text budget. Existing 64-slot full-control
  test now proves paste rejection recovers and releases its text reservation.
- Isolated owner acknowledgment test uses a real authenticated channel writer
  but deliberately injects an empty drive-command source while holding the
  original transport receiver full. This deterministically proves native FIFO
  QueueFull is acknowledged with its typed cause, stops the owner operation,
  retains last-good and releases active text; it is not a network failure test.
- `cargo test -p oneterm-ssh -p oneterm-terminal --features
  oneterm-ssh/ghostty-native --quiet`: SSH140/terminal319 pass.
- `cargo test -p oneterm-local-shell --features ghostty-native native_local_
  --quiet`: 17 pass. Native local/SSH all-target Clippy passes with -D warnings.
- ci-local.ps1 passes fmt and both workspace Clippy variants, stops at unchanged
  core shell.rs:916 PowerShell Enter guard: empty verdict vs INSTALLED:AcceptLine,
  92 pass/1 fail. Later gate commands not reached. Log: `.tmp/us0232-ci-local.log`.
  The prior full native local suite's two conhost 0x6d orphan failures are not
  fixed or rerun by this SSH-only slice; no assertion is weakened or ignored.
- Doc-path, English text (1240 files), dependency graph and diff checks pass.

Focused functional criteria have proof; complete acceptance remains gate-blocked.
No production/full TerminalSession or GUI/Unix acceptance, clipboard-family OSC,
IME, jump/SFTP/tunnel lifetime or physical-delivery guarantee. Cancellation does
not retract already enqueued/sent bytes or preempt active native work by deadline.
No Harness tool is exposed in this session; Markdown record retained.

## Handoff

Full native session/content/render/IME/selection/search, remaining OSC/security
parity and local/SSH production cutover/GUI acceptance remain separate work.
