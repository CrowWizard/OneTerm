# Work: Shared render rows for Ghostty

ID: US-0212
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

The existing GPUI row reader consumes application-owned render values, including
real Ghostty viewport rows, without creating an oneterm-vt Terminal or coercing
opaque native identity into an ordered RowId.

## Scope

- [x] In scope: borrowed shared row contract, cache equality, bounded native
  projection into resolved runs/clusters, existing GPUI cell/scanner integration.
- [x] Out of scope: full TerminalContent/model replacement, native history anchors,
  selection/hyperlinks/images/semantic regions, modes, production backend switch.
  Missing capabilities remain explicit cutover blockers, not shipping fallbacks.

## Acceptance

- [x] Shipping rows use the same cell/run/cluster reader without added row copies.
- [x] Native rows enter that reader preserving text, widths, wrap and styles.
- [x] Cache keys distinguish native content changes, not just row identity;
  equality never implies ordering or history position.
- [x] Native rows expose no fabricated logical RowId or graphics placement.
- [x] Projection validates dimensions/text bounds and reserves fallibly.
- [x] Native renderer fixtures exercise the existing cell and semantic text reader.
- [x] External oneterm-vt source/public API is unchanged; no second Terminal is run.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`.
- `docs/agents/code-style.md`, `docs/agents/structure.md`,
  `docs/agents/crate-dependency-rules.md`, `docs/agents/error-policy.md`,
  `docs/agents/dependencies.md`.
- IN-0057 Ghostty HLD/LLD, DEC-0023, US-0208 through US-0211.
- `crates/vt/src/snapshot/row.rs`, `crates/vt/src/snapshot/state.rs`,
  `crates/terminal/src/content.rs`, existing frame/plan-cache/image readers.
- Pinned native `style.h`: flags, underline encoding and color contracts.

### Documentation Action

Update Ghostty LLD and US-0208 handoff with the shared-row boundary and its
remaining whole-frame/model gaps. No persistence change or GPUI widget API change.

### Reconciliation

Updated Ghostty LLD, US-0208 handoff and probe README. The shipping frame's row
reader now uses RenderRow; whole-frame/session documentation remains unchanged
because no production selector or metadata/interaction contract changed.

## Context

US-0209 publishes owned native viewports, but GPUI FrameRow borrows SnapshotRow
directly. Native row identity is opaque and has no sequence number. Use exact
owned-row equality for staging cache keys, not a lossy hash or invented RowId.

## Plan

- [x] Introduce a borrowed application row view and equality-only key.
- [x] Project native cells into bounded application runs/clusters.
- [x] Route the current GPUI row reader through that view and prove native rows.

## Decisions

DEC-0023 and Ghostty LLD own the one-engine and opaque-identity constraints.
Exact native row keys retain an Arc to immutable projected values. This is a
correctness-first staging cost, not the final production performance contract.

## Verification Plan

Run default terminal/renderer tests, native terminal tests and native renderer
fixtures, all-target Clippy, ABI probe, docs/graph/English/whitespace checks and
the complete ci-local.ps1 gate. Record known unrelated failures without hiding
them. No live shell or SSH acceptance is claimed by these tests.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

2026-11-11: native terminal tests passed 238 (the previous 233, four native
projection tests and one shared snapshot-row borrowing test). Projection fixtures
cover combining/ZWJ/wide text, wraps, exact keys across captures and after native
destruction, cell/style/default-color key changes, explicit attribute/underline
mapping, run merging, malformed width, selected-cell rejection and text bounds.
The shared-row fixture verifies pointers still borrow the original cells, runs
and cluster arena. No second state engine is used by native fixtures.

The C ABI/runtime probe and standalone Rust owner runner passed 31 tests after
adding explicit guards for every native style flag and underline discriminant
used by the new projection. The standalone runner intentionally does not compile
application-only row projection; Cargo owns that integration proof.

Dependency graph, documentation paths, English text and whitespace checks passed.
Native renderer frame tests passed 9 (including the real Ghostty row/cell/text
scanner fixture). Native all-target Clippy for terminal and terminal-view passed
with warnings denied. The complete default renderer suite finished with 446
passed, 1 failed and 3 ignored. The unchanged
`render::glyphs::tests::ascii_fast_path_matches_directwrite` failed on Fira Code:
actual `(1, 1)`, expected `(0, 1)` at glyphs.rs:857, matching the previously
recorded failure. This is not a green renderer regression gate. Do not weaken
the assertion or count the focused native fixture as full renderer acceptance.
The complete ci-local.ps1 run passed fmt and both workspace all-target Clippy
variants, then stopped at `cargo test --workspace`: unchanged core test
`config::shell::tests::powershell_enter_guard_decides_the_four_bindings` failed
at shell.rs:916 (92 passed, 1 failed). `powershell.exe` with the default binding
returned an empty verdict instead of `INSTALLED:AcceptLine`, matching the prior
gate failure. Later gate commands did not run. Both background verification
commands have now finished; the overall gate remains failed.
The initial foreground renderer compilation
exceeded the 120-second tool timeout; a background run continues verification.

Harness tool discovery returned no tool in this session. Packet follows the
editable template manually. No actual GUI/local/SSH was exercised. Native
projection rejects selection rather than guessing geometry. Missing hyperlinks,
graphics and semantic marking remain migration blockers. Default/native RGB
resolution preserves paint values but does not yet retain default-vs-exact color
intent for theme contrast policy; do not select it as a production frame.

## Handoff

US-0208 retains complete production cutover acceptance.
Next whole-frame integration must atomically capture modes, viewport metadata
and rows on the owner worker and project before publication, not perform native
calls or fallible full-row conversion on GPUI. Replace SnapshotCursor's absolute
id and selection/placement RowId arithmetic with real tracked anchors/display
coordinates; never fill them with defaults merely to satisfy TerminalContent.
TerminalRender::snapshot_into currently has no error return, so preserve the last
valid publication with explicit session failure rather than hiding projection
errors behind empty/default content. Keep existing local/SSH sessions unchanged
until that contract and shipped interaction/OSC/graphics parity are proven.
