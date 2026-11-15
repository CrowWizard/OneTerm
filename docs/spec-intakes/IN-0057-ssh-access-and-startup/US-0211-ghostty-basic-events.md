# Work: Ghostty title, directory and bell events

ID: US-0211
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

Capture native title/directory/bell callbacks into bounded owned events and route
them through existing sanitization and reliable session delivery before repaint.

## Scope

- [x] In scope: native callback ABI, allocation-free bounded capture, worker drain,
  pump batch bounds, existing title/CWD policy, reliable events then repaint.
- [x] Out of scope: production selection, clipboard, program status, shell marks,
  graphics/history/interaction and async integration.

## Acceptance

- [x] Callback pointers never escape; no allocation or unwind in callbacks.
- [x] Captured-event count/text bounds fail explicitly, never truncate callback
  payloads. Native parser length parity is a separate unresolved gap below.
- [x] Split input preserves title/directory/bell order and ownership.
- [x] Existing title/CWD sanitization is reused; URI decoding preserves Windows
  drive handling and never accesses local files.
- [x] Replies enqueue before reliable events; reliable events precede repaint.
- [x] Queue backpressure occurs outside native work; no second state engine.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/agents/code-style.md`,
  `docs/agents/error-policy.md`, `docs/terminal-backend.md`.
- Ghostty HLD/LLD and US-0208/0209/0210: staged ownership and cutover gaps.
- `backend/osc_router.rs`, `backend/event_sink.rs`, `backend/pump.rs`,
  `vt/src/terminal/dispatch.rs`: existing title/CWD policy, URI semantics and
  reliable-event ordering. Only pure value/policy code is reused, not Terminal.
- Pinned `terminal.h`/`types.h`: title/PWD getters inside callbacks are permitted.

### Documentation Action

Update Ghostty LLD, probe README and migration handoff. Production behavior and
persisted schemas do not change; keep overall cutover acceptance open.

### Reconciliation

Updated Ghostty LLD, probe README, US-0210 extension handoff and US-0208 migration
handoff. Production documentation remains accurate: neither backend selects
Ghostty. Full-gate and long-OSC parity gaps prevent overall completion.

## Context

US-0210 proves transport replies but has no reliable event delivery. Native
directory callback supplies raw URI or path; it does not grant filesystem access.

## Plan

- [x] Add fixed native event arena (256 events, 64 KiB text) and owned drain.
- [x] Route title/CWD/bell through existing router and bounded pump batch.
- [x] Verify native lifetime/splits/overflow and reliable ordering/backpressure.

## Decisions

Each feed drains native events after reply enqueue. A pump batch retains at most
256 events and 64 KiB raw text across reads; overflow is fail-stop. Native callback
query failure is fatal. Raw strings are decoded only after leaving C. Reliable
events are sent at finish_batch, then capture and coalesced Output. The staged
pump now requires an OscRouter so transport and policy cannot diverge. Clipboard
callbacks remain uninstalled pending their safe synchronous bridge.

## Verification Plan

Run native-feature tests/Clippy, C ABI and standalone Rust probes, docs/graph/
English/whitespace checks and ci-local.ps1. Prove native event order and ownership,
count/text overflow, URI/security handling, reply-first delivery, bounded pending
batches and backpressure without native locks. Real GUI/SSH remains unverified.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

2026-11-11: `cargo test -p oneterm-terminal --features ghostty-native --quiet`
passed 233 tests: 183 existing/default-engine tests, 31 native owner/render/event
tests and 19 native pump tests. Native all-target Clippy passed with warnings
denied. Expanded C ABI guards verify title/PWD/bell keys and GhosttyString layout;
the C runtime probe and standalone actual Rust owner/render/event runner passed
31 tests. Initial standalone compilation caught unexercised event drain APIs;
an actual worker event transfer fixture fixed that coverage gap without allowing
dead code or relaxing warnings.

Fixtures verify every two-chunk split of multibyte title, URI and bell, callback
payload ownership after mutation/drop, exact-capacity capture, sticky overflow,
invalid pointer rejection, worker event transfer, reliable events before Output,
reply enqueue before event delivery, Windows percent-decoded URI path, inherited
control/BiDi stripping, accumulated count/text limits across reads, explicit-close
flush and queue backpressure with readable publication state. A fixture initially
assumed cwd control bytes were rejected: the reviewed existing policy strips
them; the fixture now asserts that same preserved policy, not a new permission.

Native parsing ignored 2048-byte title inputs in a first text-budget fixture.
512-byte varying titles do produce callbacks; 128 of them reach the 64 KiB batch
limit and the next title fails explicitly. Do not confuse this wrapper-budget
proof with native parser limit parity. Long-title/OSC behavior requires further
work before production cutover; no upstream source or parser limits were changed.

The complete `ci-local.ps1` run passed fmt and both workspace Clippy variants,
then stopped at unchanged core test
`config::shell::tests::powershell_enter_guard_decides_the_four_bindings`:
empty `powershell.exe` default verdict instead of `INSTALLED:AcceptLine`
(92 passed, 1 failed). Later gate steps did not run. Prior ConPTY orphan and
DirectWrite failures remain unresolved. Dependency graph, doc paths, English
contributor text and diff-whitespace checks passed.

Reliable events already delivered cannot be rolled back if capture subsequently
fails. Explicit close flushes reliable pending values; fail-stop drops the
unsuccessful batch and Drop never blocks for UI delivery. Closed UI channels
retain the existing sink logging/counter behavior. Render consumers still must
release the shared repaint hint; FrameReader is not a UI acknowledgment.

Harness is unavailable in this session; packet follows the editable work template
manually. No real PTY/SSH/GUI was exercised and no production selector changed.
Clipboard, program status, semantic prompts, long OSC, graphics, history and
interaction parity remain open under US-0208.

## Handoff

US-0208 owns full migration and remaining compatibility acceptance.
