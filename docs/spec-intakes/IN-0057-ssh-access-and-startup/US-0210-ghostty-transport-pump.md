# Work: Ghostty reply-first transport pump

ID: US-0210
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

Provide a default-off Ghostty backend pump using the existing PtyTransport
interface, enforcing reply enqueue before owned-frame publication without a
second terminal state engine.

## Scope

- [x] In scope: bounded feed chunks, reply ordering, explicit batch capture,
  native/transport resize ordering, typed failure and fail-stop teardown.
- [x] Out of scope: production backend selection, UI notification, OSC event and
  clipboard policy, graphics, history/search/selection, async executor integration.

## Acceptance

- [x] Only Ghostty consumes bytes in this pump; no oneterm-vt Terminal exists.
- [x] Reads larger than 64 KiB are chunked; replies drain between chunks.
- [x] Frame capture follows successful reply enqueue; process alone publishes none.
- [x] A rejected reply (including QueueFull) stops the pump without retrying
  consumed output or publishing a successful partial batch; last frame survives.
- [x] Resize rejects invalid/budget dimensions before native/transport mutation;
  transport/native disagreement stops the pump. Explicit close releases the worker.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/agents/code-style.md`,
  `docs/agents/error-policy.md`: ownership, runtime failures and proof.
- `docs/terminal-backend.md`, `crates/terminal/src/backend/pump.rs`,
  `backend/transport.rs`, `session.rs`: existing reply-first transport seam.
- `low-level-design/ghostty-engine.md`, US-0208, US-0209: native thread ownership,
  owned frames, no dual engine and production cutover blockers.

### Documentation Action

Update required: Ghostty LLD, probe README, US-0208 handoff and this packet.
Shipping backend contracts remain unchanged: this pump is default-off staging.

### Reconciliation

Updated Ghostty LLD, probe README and US-0208 handoff. The production backend
contract remains accurate and unchanged: neither shipping backend uses Ghostty.
Focused proof passes; full-gate acceptance remains blocked.

## Context

EngineOwner currently returns reply bytes but cannot enforce transport delivery.
The existing PtyTransport guarantees FIFO atomic enqueue, not physical delivery.
Queue rejection after parsing cannot be handled by refeeding consumed output.

## Plan

- [x] Add a blocking backend pump with private engine endpoint and explicit batch capture.
- [x] Test ordering, large/split reads, resize and failures using real native state
  and a deterministic fake implementing the existing transport trait.
- [x] Run native terminal tests/Clippy, native probe and full project gate.

## Decisions

The staging pump fails closed on reply rejection, including saturation: no
implicit retry or continued parsing of already-consumed output. It closes the
engine worker and requests out-of-band transport close. FrameReader status
describes engine lifecycle; the pump returns the causal transport error to its
backend caller. It posts no UI events yet, because reliable OSC event adaptation
must precede repaint. This API is blocking, for backend workers only.

## Verification Plan

Native-feature fixtures prove no publication before reply enqueue, DSR order
across arbitrary splits/large reads, no empty transport writes, resize forwarding,
invalid resize preservation, native overflow and each transport failure class,
last-frame retention and fail-stop rejection. Run the native feature suite and
Clippy, standalone ABI/runtime probe, documentation checks and ci-local.ps1.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

2026-11-11: added `backend/ghostty_pump.rs` and 13 native transport-pump fixtures
using the existing PtyTransport interface and a deterministic fake transport.
They prove all two-way input splits, large reads spanning three bounded requests,
reply enqueue before explicit publication, multiple advances in one batch,
empty-write avoidance, invalid size preservation, row/column forwarding,
mode-2048 resize report ordering and rejected resize reports, all three transport
reply failure classes, native overflow, retained last frame and close/drop.
Explicit close reports transport failure after native teardown; fail-stop keeps
the original operation error even when transport close also fails. Cleanup
failures are logged, not silently discarded. A failed close request does not
prove the transport closed; the backend receives the causal error and must own
actual process/channel teardown.

`cargo test -p oneterm-terminal --features ghostty-native --quiet`: 222 passed
(183 existing/default engine tests, 26 native owner/render tests, 13 native
transport-pump tests). Native-feature all-target Clippy passed with warnings
denied. The C ABI/runtime probe and standalone Rust owner/render runner passed
26 tests; the standalone runner intentionally does not import the crate-dependent
backend pump, whose proof is the native Cargo suite.
Dependency-graph, documentation-path, English contributor-text and diff-whitespace
checks also passed after documentation reconciliation.

Full `pwsh -NoProfile -File scripts/ci-local.ps1` rerun passed fmt and both
workspace Clippy variants. Workspace tests again stopped at unchanged core test
`config::shell::tests::powershell_enter_guard_decides_the_four_bindings`:
`powershell.exe` default verdict was empty instead of `INSTALLED:AcceptLine`
(92 passed, 1 failed). Later gate steps were not reached. Earlier ConPTY orphan
and DirectWrite failures remain unresolved. No regression test was weakened.

Harness is unavailable in the session tool catalog; this packet follows the
editable work template manually. Not production integration: no real PTY/SSH
session was created, no UI event was posted, OSC/clipboard/graphics adaptation
and the shared model/content switch remain open. Do not call blocking methods
directly from a tokio executor or GPUI.

## Handoff

US-0208 still owns complete local/SSH production cutover acceptance.
US-0211 extends this staging pump with title/directory/bell policy delivery via
OscRouter; the earlier no-event limitation above describes this packet's initial
proof, not the extended seam. Other OSC families and production selection remain open.
