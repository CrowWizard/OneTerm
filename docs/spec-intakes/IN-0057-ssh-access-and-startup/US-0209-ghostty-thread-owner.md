# Work: Thread-confined Ghostty owner and shared owned frames

ID: US-0209
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

Provide one thread-confined native engine owner with bounded synchronous worker
requests and shared immutable viewport publication, without moving native handles
between threads or changing either production backend before parity is proven.

## Scope

- [x] In scope: creation/use/destruction on one thread, typed errors, bounded
  worker requests, ordered owned replies, explicit resize and snapshot, shutdown,
  independently polled shared frames and per-consumer viewport damage.
- [x] Out of scope: production model/pump cutover, asynchronous clipboard policy,
  history/anchors, input/search/selection, image projection and GUI acceptance.

## Acceptance

- [x] Native handles never leave their creating worker; no unsafe Send/Sync.
- [x] Requests are serialized with bounded bytes/count; errors never look like
  successful partial feeds. Feed/resize replies are returned before another
  request may publish a frame; the transport caller must deliver replies first.
- [x] Frames fan out through Arc without native work or engine-lock waits on a
  reader; each consumer owns its damage watermark and sees skipped publications.
- [x] A fatal native failure stops the worker and is visible to readers; close
  discards no previously acknowledged reply and frees native handles on-worker.
- [x] Drop does not join/block the caller; explicit close joins on a worker path.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/agents/code-style.md`, `docs/agents/error-policy.md`.
- `docs/terminal-backend.md` and `crates/terminal/src/backend/pump.rs`: reply and
  event ordering; do not turn the native worker into a new transport.
- `crates/terminal/src/handle.rs`: existing engine locking/render demand contract.
- `low-level-design/ghostty-engine.md`, HLD and DEC-0023: thread confinement,
  immutable render data, single-engine production boundary.
- US-0208 and `ghostty.rs`/`ghostty/render.rs`: tested native staging primitives.

### Documentation Action

Update required: Ghostty LLD, probe README, US-0208 handoff and this packet.
Production contracts remain unchanged; record the default-off staging API and
its blocking nature rather than claiming local/SSH integration.

### Reconciliation

Updated the Ghostty LLD, probe README and US-0208 handoff with the blocking
worker/publication contract and focused proof. Production contracts are unchanged
because no production caller selects this endpoint. Full gate remains failing.

## Context

The native terminal and render state are deliberately !Send/!Sync. The owned
Frame is Send/Sync. Separate persistent render consumers were observed to miss
native dirty rows; a single worker will publish one owned capture for fan-out.

## Plan

- [x] Add a bounded native owner request loop with typed startup/operation errors.
- [x] Publish immutable frames and compute independent owned-frame damage.
- [x] Prove ordering, bounds, fatal failure, lifetime and shutdown with native tests.

## Decisions

Use the dedicated-owner option of the accepted Ghostty LLD. Do not widen the
native Send/Sync contract or feed production bytes into two engines. The first
worker API is blocking and belongs on a backend worker, never GPUI or a tokio
executor thread. Async integration remains a separate application cutover step.

## Verification Plan

Native tests for thread lifecycle, split writes/replies, feed/resize/snapshot
ordering, caller-thread transfer, frame fan-out and damage across skipped frames,
size rejection without mutation, failure propagation, drop with live readers,
and repeated owner creation/close (close consumes the endpoint). Run native-feature terminal tests/Clippy, standalone C/Rust
probe, documentation/graph checks and the complete project gate; retain actual
failures and do not mark US-0208's cutover acceptance complete.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

2026-11-11 focused proof: native-feature terminal tests passed 209 tests,
including 26 Ghostty tests (11 worker/publication fixtures). Native-feature
all-target Clippy passed with warnings denied. The C ABI/runtime probe and
standalone Rust import of the actual adapter passed all 26 tests. No unsafe
Send/Sync was added. The endpoint's serialized operations are blocking and do
not perform transport I/O or enforce the caller's delivery of replies.

Review corrected two failure paths: a disconnected response now joins before
reading lifecycle state, so panic cannot be mistaken for ordinary closure;
abandoned response delivery no longer overwrites a causal native failure.
Fixtures assert immediate typed panic reporting and both abandoned-response cases.

Full `pwsh -NoProfile -File scripts/ci-local.ps1` rerun: fmt and both workspace
Clippy variants passed. Workspace tests stopped earlier than the previous
ConPTY failures, at unchanged core test
`config::shell::tests::powershell_enter_guard_decides_the_four_bindings`:
`powershell.exe` default returned empty stdout instead of `INSTALLED:AcceptLine`
(92 passed, 1 failed). A targeted rerun reproduced it. Later gate steps were
not reached; prior ConPTY orphan and DirectWrite failures remain unresolved,
not reverified by this gate. No tests were weakened or ignored to bypass it.

Harness is not available in this session's tool catalog; this record is generated
manually from `docs/templates/work.md`. Do not mark the overall migration done:
production local/SSH model/pump integration, sync clipboard bridging, Sixel,
history/anchors/hyperlinks, input/search/selection and GUI acceptance remain open.

## Handoff

US-0208 owns overall migration acceptance; US-0209 owns this independently
verifiable thread/publication boundary only.
