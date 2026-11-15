# Work: Atomic Ghostty frame state and viewport interaction

ID: US-0213
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

Owned frames carry native mode and scrollbar state captured with their rows on
the owner thread. Viewport commands use the same serialized owner, not a shadow
grid or UI native calls. This establishes the next shared-session prerequisite.

## Scope

- [x] In scope: typed mode/scrollbar ABI, atomic capture, metadata damage, native
  top/bottom/delta/absolute-row interaction and worker command lifetime.
- [x] Out of scope: production selector, complete keyboard/mouse encoder, clipboard,
  selection/search/graphics/history anchor integration. Local/SSH session switching
  remains under US-0208 until their full contract is implementable.

## Acceptance

- [x] Rows, cursor, modes and scrollbar belong to the same capture operation.
- [x] Mode-only and viewport-only changes cannot disappear as unchanged damage.
- [x] Native viewport commands serialize with feed/resize/capture; no raw handles
  or invented ordered row identity reach the UI.
- [x] Native errors are explicit and use existing worker fail-stop behavior.
- [x] Tests prove scrolling, alternate screen, paste/cursor/mouse modes, ownership
  and skipped publication damage without spawning a second state engine.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md` and agent
  style/structure/dependency/error policies reviewed in the preceding slice.
- IN-0057 HLD and `low-level-design/ghostty-engine.md`, DEC-0023, US-0208/0212.
- `ghostty/owner.rs`, `ghostty/render.rs`, local-shell session spawn,
  `session.rs` render/input facade and `vt/src/snapshot/modes.rs`.
- Pinned native `terminal.h` and `modes.h`: exact getters, mode packing,
  scrollbar coordinates and scroll tagged-union ABI.

### Documentation Action

Update Ghostty LLD and migration handoff. No persisted schema or shipping
session behavior changes in this prerequisite; do not claim local/SSH cutover.

### Reconciliation

Updated `low-level-design/ghostty-engine.md`, the US-0208 handoff and probe
README. Production facts remain unchanged: neither backend selects Ghostty.

## Context

US-0212 permits native rows in the GPUI cell reader but lacks frame metadata.
Native modes are retained explicitly, not guessed from output or coerced into
legacy input defaults. Scrollbar offsets are numeric positions, not stable ids.

## Plan

- [x] Capture owned native metadata with every frame.
- [x] Add serialized viewport interaction and metadata damage.
- [x] Prove actual native state and update evidence.

## Decisions

DEC-0023 and Ghostty LLD retain one state engine and dedicated native ownership.

## Verification Plan

Native terminal tests/Clippy, ABI and standalone probes, renderer integration
tests, docs/English/whitespace checks and complete ci-local.ps1. Record known
PowerShell/DirectWrite failures and unreached gate steps without weakening tests.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Final US-0213 focused verification on Windows: 244 native-feature terminal
tests passed, terminal/view native all-target Clippy passed with warnings denied,
expanded C ABI/runtime probe and 36 standalone native tests passed. Renderer
frame tests passed 9/9. Added scrollbar-height consistency and Kitty keyboard
flags across primary/alternate screen restoration and RIS reset. The cursor
visibility regression was fixed without weakening the existing narrow damage
assertion. Native scrolling changes publish only at pump finish_batch.

Full `scripts/ci-local.ps1` rerun (shell sh_1c8cf147b001PEv3Z2IkXnSt1f): fmt
and both workspace Clippy variants passed; workspace tests failed at unchanged
`config::shell::tests::powershell_enter_guard_decides_the_four_bindings`,
`core/src/config/shell.rs:916`, empty default powershell.exe verdict instead of
`INSTALLED:AcceptLine` (92 core tests passed, 1 failed). Later gate checks were
not reached. The preceding run stopped at formatting during the final edits;
that formatting was corrected before this rerun. No complete gate or actual
GUI/local/SSH session acceptance is claimed. Harness tool is unavailable;
records follow the editable template.

## Handoff

US-0208 still owns actual local/SSH shared model and production migration.
