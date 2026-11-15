# Work: Native wheel routing

ID: US-0241
Intake: IN-0057
Created: 2026-11-12

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
- Spec Intake: IN-0057; accepted native HLD/LLD and DEC-0023.

## Outcome

Native wheel input is ordered with output on the native owner. Live native state
alone selects reporting, alternate-screen cursor keys or history scrolling.

## Scope

- [x] In scope: bounded typed vertical wheel request, atomic owner routing, async
  pump publication/delivery, local/SSH FIFO admission and independent GPUI surface.
- [x] Out of scope: horizontal scrolling, selection, settings binding, production
  factory selection, full panel/security parity and desktop UAT.

## Acceptance

- [x] Requests validate geometry and nonzero signed steps bounded to ten, before
  admission; positive means older/up. No UI mode snapshot chooses the route.
- [x] Scrolled primary history or Shift scrolls locally; otherwise live reporting
  uses native wheel presses without releases. Alternate-screen arrows require
  native alternate-scroll mode; disabled mode is a no-op. No legacy encoding.
- [x] Reliable events precede wheel mutation; bytes use the existing transport
  FIFO. Only history mutations require capture and coalesced repaint. Failures
  and cancellation preserve fail-stop/no-replay behavior.
- [x] Local/SSH expose the same SurfaceSession operation and 64-slot queue.
  GPUI preserves fractional lines, excludes invalid/stale geometry, caps one event
  to ten steps and consumes rejected whole steps without automatic retry.
- [ ] Focused owner/pump/backend/surface proofs and native Clippy run; full gate
  and documentation checks are recorded honestly, including existing failures.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`, required agent
  guides: layering, reference-first, runtime errors and verification.
- Native HLD/LLD, DEC-0023, US-0208 and US-0240: owner confinement, live mode
  selection, reporting coordinates and unchanged production boundary.
- `docs/terminal-backend.md`, pump/bridge/owner, native state/input, legacy wheel
  policy and UI mouse helpers: ordering, scroll sign, bounds and event admission.
- Pinned native mouse encoder maps Four/Five to protocol 64/65. GPUI Kit
  reference and srcwalk are still missing; checked existing handlers and cached
  pinned GPUI ScrollWheelEvent/ScrollDelta declarations instead of web research.

### Documentation Action

Update native LLD before code; reconcile backend notes and US-0208 handoff after
proof. No persistence, dependency or external oneterm-vt API change.

### Reconciliation

Updated native LLD, backend notes, probe README and US-0208 handoff. No dependency,
persistence or external oneterm-vt API change. Packet remains in progress while
complete quality gate and desktop acceptance remain blocked/unproven.

## Context

The legacy wheel rounds each event upward; native surface instead accumulates
fractional lines to avoid turning tiny trackpad deltas into full steps. Alternate
scroll explicitly respects native mode 1007. UI geometry remains the US-0240
padding-free device grid. Native byte generation and route selection are atomic
on one worker; capture is still controlled by the pump.

## Plan

- [x] Add owner operation and bridge reply distinguishing bytes/history/no-op.
- [x] Integrate pump, endpoints and native surface.
- [ ] Prove focused behavior, run gate and reconcile docs.

## Decisions

DEC-0023; native LLD wheel contract.

## Verification Plan

Test signed bounds, mode transitions, SGR exact bytes, normal/application cursor
keys, alternate-scroll off, Shift/history precedence, capture/repaint ordering,
transport failures and fractional UI admission. Run native terminal/SSH/local/view
tests and all-target Clippy, `pwsh scripts/ci-local.ps1`, doc-path/English/dependency
and diff checks. Existing Enter guard/Fira Code/conhost gaps remain explicit.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Harness tool discovery returned no tool; this packet retains the record.

- `cargo test -p oneterm-terminal --features ghostty-native wheel -- --nocapture`:
  five pass, including the pre-existing native mouse encoder wheel test. New owner
  proof checks SGR up/down/control, unpublished normal/application cursor modes,
  mode 1007 disable, tracking precedence, Shift, history boundaries, invalid steps
  (including integer extrema) and NaN. New pump proof checks history publication,
  byte-only damage-free behavior, repaint coalescing, reliable event backpressure,
  cancellation and transport QueueFull/no-replay. `.tmp/us0241-wheel.log`.
- Native SSH152 and terminal343 pass: `cargo test -p oneterm-ssh -p
  oneterm-terminal --features oneterm-ssh/ghostty-native --quiet`.
  New real SSH SurfaceSession proof checks exact SGR/native application-arrow
  bytes, mode-1007 disable/no output, invalid/closed admission and history
  publication/repaint without additional peer output. `.tmp/us0241-backends.log`.
- Native local focused24 pass: `cargo test -p oneterm-local-shell --features
  ghostty-native native_local_ --quiet`. Real ConPTY SurfaceSession wheel moves
  history up two rows and back without child output. Strict ReadConsoleW Unicode
  fixture also checks wheel admission, invalid zero and Closed. Latest rerun
  includes that history proof. `.tmp/us0241-local.log`.
- Native surface focused7 pass: `cargo test -p oneterm-terminal-view --features
  ghostty-native native_surface -- --nocapture`. Real GPUI scroll dispatch turns
  four quarter-lines into one typed step. Tests cover Shift forwarding, absent/
  mismatched/outside geometry, blur fractional reset, admission error without
  retry, direction changes, caps, overflow dropping and nonfinite deltas.
  `.tmp/us0241-surface.log`. Subsequent full view run includes all seven again.
- Native local/SSH/view all-target Clippy with `-D warnings` passes after final
  source changes. `.tmp/us0241-native-clippy.log`.
- Full native view457 pass/1 fail/3 ignored: existing Fira Code DirectWrite
  assertion at `render/glyphs.rs:857`, actual `(1, 1)` versus expected `(0, 1)`.
  `.tmp/us0241-native-view.log`.
- Full gate attempt passes fmt and both workspace Clippy variants, then fails
  existing core `powershell_enter_guard_decides_the_four_bindings` at `shell.rs:916`:
  stdout empty instead of `INSTALLED:AcceptLine`. Later steps do not run. Final
  post-format rerun reproduces the same failure. `.tmp/us0241-ci-local.log`.
- Doc paths220, English1259, dependency graph20 and `git diff --check` pass.
- Full local orphan/conhost suite and desktop wheel/blur/IME UAT were not rerun.
  Previous conhost, OSC110, .NET-reader and precise preedit geometry gaps remain.
  No production cutover or complete interaction/panel/security parity is claimed.

## Handoff

Production remains unchanged. Complete interaction/panel/security parity and GUI
acceptance still precede simultaneous local/SSH cutover.
