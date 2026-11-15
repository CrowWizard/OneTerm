# Work: Native session viewport content

ID: US-0237
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
- Spec Intake: IN-0057, accepted native LLD and DEC-0023.

## Outcome

Local and SSH native endpoints supply the same reusable per-consumer viewport
content with atomic rows/cursor/modes/colors/selection/scrollbar and independent
damage. Consumers do not enter native code, reproject rows, create a legacy
terminal or invent RowId/history positions.

## Scope

- [x] In scope: shared native render-source trait and viewport content consumer,
  immutable typed view of one publication, safe damage-free window reads, real
  local/SSH source proof, and renderer scanner integration proof.
- [x] Out of scope: complete TerminalSession/factory, shipping TerminalContent
  conversion, legacy-mode fallback, graphics/links/semantic fabrication, GPUI
  widget/candidate rendering, search/input policy and production cutover.

## Acceptance

- [x] Both native endpoint types expose the same object-safe render-source
  contract; each fresh content owns its reader/watermark and first-frame damage.
- [x] Content polls one prepared publication, returns typed absent-frame state
  before initialization, and retains last-good frame with closed/failed state.
- [x] Rows, cursor, raw native modes, scrollbar, effective colors/palette and
  exact half-open selection spans belong to the same immutable frame.
- [x] Content queries and bounded display-row windows never advance damage or
  request capture; unknown rows return None, not a fabricated blank row.
- [x] Repeated polls, independent consumers, resize/history/selection/colors,
  close/failure and real local/SSH content have focused proof.
- [x] Existing view cell/text scanners consume content rows without legacy state;
  production session selection and public external VT API remain unchanged.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`, required agent
  style/structure/crate/dependency/error guides, DEC-0023 and native LLD.
- SharedModel ModelReader, ProjectedFrame, native render/state/owner damage.
- Shipping TerminalRender/TerminalContent, GPUI Frame/FrameRow/PlanCache and
  existing native row-scanner test; NativeLocalSession and NativeChannel.

### Documentation Action

Update required: native LLD, backend migration note, probe README, US-0208 handoff.
No persistence, dependency, production factory or external VT schema/API changes.

### Reconciliation

Native LLD records the typed source/content boundary. Backend notes, probe README
and US-0208 handoff record the verified scope and retained cutover blockers.
Content exposes effective captured colors only: caller defaults remain session
configuration, never reconstructed from effective OSC-overridden colors.

## Context

The shipping frame uses legacy RowId and ModeSnapshot, which cannot truthfully
represent all native state. Preserve a typed native viewport boundary instead of
inventing a full TerminalSession or converting missing fields to power-on values.
Prepared projection already runs on the bridge dispatcher; consumer reads only
retain immutable Arcs and compute their own damage.

## Plan

- [x] Add shared render-source and reusable native content with typed viewport.
- [x] Connect both endpoints and prove scanner/content behavior on native frames.
- [x] Run regressions/native Clippy/full gate; reconcile docs and remaining work.

## Decisions

DEC-0023 and native LLD; the first coherent part of the full render/session plan.

## Verification Plan

Unit/bridge tests for absent publication, one-frame metadata, damage independence,
damage-free bounded reads, retained old frames, failed/closed source, resize and
scroll. Real local/SSH fixtures consume the shared source, selection/colors where
existing fixtures already publish them. View scanner test reads through content.
Run native terminal/SSH/local/view focused tests and Clippy, ci-local.ps1 and final
doc-path/English/dependency/diff checks. Record unrelated gate failures explicitly.

<!-- HARNESS:PROOF:BEGIN -->
- [x] Unit proof
- [x] Integration proof
- [ ] E2E proof
- [x] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Harness tool is not exposed; retain this Markdown work record.

- Three native_content bridge tests pass: absent/closed-without-frame, atomic
  cursor/modes/colors/selection/scrollbar, independent Full/None/Rows damage,
  clamped zero/overflow windows, retained old viewports across resize/scroll and
  failed/closed state with last-good publication. No legacy state is constructed.
- Real ConPTY and in-process russh fixtures construct Content through the endpoint
  RenderSource trait object, compare effective colors/palette, independent damage,
  selection set/clear and retained selection spans. No new parser or wire traffic.
- Existing view FrameRow cell/text scanner test now reads a Content viewport after
  the native worker closes; wide/combining text, styles, RGB, wrapping and scanner
  coordinates pass. This is scanner proof, not a full GPUI frame/session adapter.
- Native terminal338/SSH149/local focused24 pass. All-target native Clippy for
  terminal, local-shell, SSH and terminal-view passes with warnings denied.
- Full native view: 447 pass/1 fail/3 ignored. Existing Fira Code DirectWrite test
  still fails at glyphs.rs:857, actual (1,1), expected (0,1).
  Log `.tmp/us0237-native-view.log`.
- Full native local: 57 pass/2 fail/3 ignored. Existing orphan tests still fail at
  session_orphan_tests.rs:252 on conhost exit 0x6d.
  Log `.tmp/us0237-native-local.log`.
- Full gate passes fmt and both workspace Clippy variants, then stops in core's
  PowerShell Enter guard at shell.rs:916: empty output versus INSTALLED:AcceptLine.
  Log `.tmp/us0237-ci-local.log`; later gate commands are not reached. Overall
  verification and acceptance remain incomplete; no production/GUI success claim.
- Final doc-path (220 paths), English (1249 files), dependency graph (20 packages)
  and git diff whitespace checks pass. No dependency or persistence changes.

## Handoff

Full renderer/session and interaction wiring still precede production cutover;
US-0235 OSC110, US-0236 .NET reader discrepancy and full-gate failures remain open.
