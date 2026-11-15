# Work: Ghostty tracked history anchors

ID: US-0216
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

Supply real native tracked cell anchors across output, scroll and reflow, with
opaque terminal-scoped tokens crossing the worker boundary instead of fabricated
ordered RowId values. This establishes the position seam for later model work.

## Scope

- [x] In scope: bounded worker-owned anchor registry, typed coordinate spaces,
  create/resolve/release, explicit absent positions, provenance and RAII cleanup.
- [x] Out of scope: production session switch, selection/search, native cell
  pointer export, GPUI/tokio dispatch and arbitrary arithmetic on native anchors.

## Acceptance

- [x] Tracked locations survive output/viewport movement and native reflow.
- [x] Coordinates explicitly identify their screen and requested space; an anchor
  on primary must never resolve as if it belonged to the active alternate screen.
- [x] No native pointer/untracked reference crosses the worker boundary.
- [x] Foreign/released tokens, invalid points and registry capacity are explicit
  recoverable errors and cannot mutate another anchor or stop a healthy worker.
- [x] At most 256 tracked handles live per worker; released slots can be reused
  without reviving stale tokens. All handles are destroyed on the owning thread.
- [x] Missing native coordinates remain None, never zero or an invented RowId.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`, agent conventions,
  structure, error policy and crate/dependency rules.
- IN-0057 HLD, `low-level-design/ghostty-engine.md`, DEC-0023 and US-0208/0215.
- `terminal/src/model.rs`, `ghostty/owner.rs`, `ghostty/state.rs` and render rows.
- Pinned `terminal.h`, `point.h`, `grid_ref.h`, `grid_ref_tracked.h`, `types.h`:
  tracked refs follow native page lists; untracked snapshots expire on mutation.

### Documentation Action

Update required: Ghostty LLD, probe README and US-0208 handoff. Current production
engine/model documentation remains accurate and must not claim cutover.

### Reconciliation

Ghostty LLD, probe README and US-0208 handoff record the registry, native boundary
finding and remaining production integration work. No current production-engine
contract was changed. Project-wide acceptance remains blocked on the full gate.

## Context

Native tracked references remain attached to their original screen across screen
switches. Resolving into active or viewport coordinates is relative to that
owning screen, not necessarily the currently displayed one. Tokens retain only
a Rust provenance marker and a never-reused per-worker identifier, not native
addresses. Native out-of-bounds is recoverable; actual unexpected native errors
retain the owner's existing fail-stop policy.

## Plan

- [x] Bind tagged point coordinates and native tracked-reference lifetime.
- [x] Add bounded registry and serialized worker requests.
- [x] Prove history, screen ownership, reflow, stale tokens, bounds and cleanup.
- [ ] Run probes, focused regressions, Clippy, complete gate and doc checks.

## Decisions

DEC-0023; native state and transport ownership do not change.

## Verification Plan

Pinned C ABI layout/value guards; standalone native tests; native terminal suite;
native terminal/view all-target Clippy; renderer frame regressions; ci-local.ps1;
doc-path, English, dependency graph and whitespace checks.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Focused verification: 272 native-feature terminal tests passed; C ABI/runtime
probe and all 56 standalone native tests passed; native terminal/view all-target
Clippy passed with warnings denied; renderer frame tests passed 9/9. Eight new
native fixtures cover history, viewports, bidirectional reflow, screen identity,
released/foreign tokens, 256-handle cap, history erasure/reset, ABI and lifetime
after terminal destruction. A pump fixture proves serialized resize/resolve,
recoverable stale-token rejection, no extra writes and no premature publication.

Testing found that pinned PageList::pin/pointFromPin enforce only some coordinate
space bounds: History(0, 0) can incorrectly address active content, and viewport
conversion can return rows below its visible height. The adapter now requires
exact native round-trip coordinates, excludes native Active positions from
History, and bounds Active/Viewport rows by native terminal height. Native resize
updates both screens. No legacy row arithmetic or synthetic ordered id is used.
History erasure resolves to None and does not reattach after RIS/new output.

Complete gate rerun passed fmt and both workspace Clippy variants, then stopped
at unchanged config::shell::tests::powershell_enter_guard_decides_the_four_bindings
(powershell.exe default verdict empty, expected INSTALLED:AcceptLine; 92 core
tests passed, 1 failed). Later gate steps did not run. Log:
sh_1c8f16038001JNAaxz20mJf1Cv. Existing ConPTY/DirectWrite failures are not resolved
by focused native proof. Doc-path, English contributor-text, dependency-graph
and git diff --check passed. Harness tool discovery returned no tools; records
are Markdown only. The work remains in progress rather than marking a red
project quality gate complete.

## Handoff

US-0208 retains actual model/input/OSC/graphics parity and production cutover.
Next integrate these real anchors into history/selection/search rather than
converting them to ordered legacy RowIds. Backend-side projection and an async
command/close bridge are still required before GPUI/tokio can use the staging
interfaces safely. No changes were committed; prior SSH/startup work is retained.
