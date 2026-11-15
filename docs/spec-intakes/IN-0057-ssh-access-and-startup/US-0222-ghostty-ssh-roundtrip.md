# Work: Ghostty real SSH transport roundtrip

ID: US-0222
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
- Spec Intake: IN-0057; depends on US-0221.

## Outcome

Prove the native async pump with the existing SshTransport and a real encrypted
loopback russh channel, distinguishing FIFO admission from peer receipt. No
legacy terminal is constructed for the tested stream.

## Scope

- [x] In scope: opt-in native SSH fixtures, DSR/input FIFO at the peer, real
  channel reads/writes, resize propagation, reliable-event backpressure,
  cancellation and explicit channel/native teardown.
- [x] Out of scope: shipping ssh_main_task or local PTY cutover, session/UI/SFTP
  lifecycle parity, production cancellation wiring, missing native semantics.

## Acceptance

- [x] A real authenticated loopback channel sends output to only Ghostty; peer
  observes prior input followed by its exact DSR response before frame publish.
- [x] Native resize and existing coalesced transport resize agree at the peer
  and in the next owned frame.
- [x] A full reliable-event queue prevents capture; releasing it delivers title
  and permits capture, while cancelling instead retains last-good and closes
  admission. Explicit channel close is observed by the peer.
- [x] Tests have deadlines, clean up the listener, use isolated known_hosts,
  and run only under the default-off ghostty-native feature.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`; agent code style,
  structure, crate dependency rules, dependencies and error policy.
- IN-0057 HLD/LLD, DEC-0023, US-0208/0221, `docs/terminal-backend.md`.
- SSH task/transport/test_support/task_tests and local event-loop/session seams;
  native async pump, shared model and checked event delivery.

### Documentation Action

Update required: LLD, probe README and US-0208 handoff with exact proof boundary.
No change to shipping backend contract: tests use explicit fixture I/O steps,
not ssh_main_task, not a production TerminalSession and not a local PTY.

### Reconciliation

Updated LLD, probe README and US-0208 handoff with the real channel proof and
explicit fixture/production boundary. No shipping session contract change.

## Context

PtyTransport success is atomic queue admission, not network delivery. Test code
drains the same Cmd FIFO into channel.data and returns its byte budget exactly
once. Reliable UI backpressure must not be confused with peer write completion.
Existing strict host-key helpers isolate trust files under project TMP.

## Plan

- [x] Add opt-in feature forwarding and real-channel fixtures.
- [x] Run focused native SSH/terminal regression and Clippy plus full gate.
- [x] Reconcile evidence and next production/local integration gaps.

## Decisions

DEC-0023; no new dependency or production selector.

## Verification Plan

Run native SSH fixtures, full native SSH suite, native terminal regression and
all-target SSH/terminal Clippy. Run ci-local.ps1, doc-path, English, dependency
graph and whitespace checks. Record blockers without claiming full acceptance.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11:

- `cargo test -p oneterm-ssh --features ghostty-native --quiet`: 111 pass,
  including both real-channel native fixtures; no ignored tests.
- `cargo test -p oneterm-terminal --features ghostty-native --quiet`: 313 pass.
- `cargo clippy -p oneterm-ssh -p oneterm-terminal --all-targets --features
  oneterm-ssh/ghostty-native -- -D warnings`: pass.
- `pwsh -NoProfile -File scripts/ci-local.ps1`: fmt and both workspace Clippy
  variants pass; workspace tests stop at unchanged core shell.rs:916 PowerShell
  Enter guard (92 pass/1 fail, empty verdict instead of INSTALLED:AcceptLine).
  Later steps are not reached; full acceptance remains blocked.
- Initial compilation caught a fixture assertion treating Frame.rows as a scalar;
  corrected to rows.len() before runtime proof. No production fix was required.
- Doc-path, English contributor-text (1222 files), dependency-graph and
  `git diff --check`: pass.

Each fixture has a 20-second deadline and listener abort guard. Native-only
stream uses the real SSH channel and production transport queue, but test code
performs I/O steps explicitly; it does not exercise shipping ssh_main_task or
claim automatic physical close while that task awaits reliable UI backpressure.
Real local PTY, production session contracts and native semantic parity remain
open. Harness catalog has no exposed tool; Markdown record only.

## Handoff

Real local PTY proof, production native session contracts and both shipping read
loops remain open; the fixture is not permission to select native in production.
