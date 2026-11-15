# Work: Native GPUI frame pipeline

ID: US-0238
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
- Spec Intake: IN-0057, DEC-0023 and accepted native LLD.

## Outcome

The existing GPUI Frame/PlanCache/cursor/selection pipeline can paint a native
Content publication through its real prepaint path, without taking a legacy
snapshot or constructing legacy row identities/modes. Shipping factories remain
unchanged until interaction/session and parity acceptance.

## Scope

- [x] In scope: optional native render-source on TerminalRender, one retained
  per-consumer content in Frame, native rows/damage/cursor/selection/reverse-video,
  explicit absent-frame and legacy-mode boundaries, headless GPUI pipeline proof.
- [x] Out of scope: complete native TerminalSession implementations, native GUI
  key/mouse/IME dispatch, theme setter mapping, history/logging/gutter identity,
  graphics/links/semantic parity, production factory selection and manual GUI UAT.

## Acceptance

- [x] Frame chooses a native source before legacy snapshot_into and owns one
  persistent reader; repeated prepaints preserve independent damage progress.
- [x] Native metadata and rows drive existing row plans and cursor/selection
  overlays; selection spans stay exact, not guessed inclusive endpoints.
- [x] Native absent-frame is an empty paint viewport, never a power-on legacy
  snapshot. Read failure retains last-good content and is reported.
- [x] Native input is never accidentally encoded with stale legacy mode defaults;
  unsupported legacy metadata/graphics/gutter is not fabricated as native parity.
- [x] Headless GPUI proves first paint, unchanged, partial row update, selection,
  cursor and resize through the actual shared render path. Existing regressions
  and native Clippy run, with full-gate gaps retained.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`, required agent
  guides, DEC-0023, native HLD/LLD and US-0237 content packet.
- TerminalRender/TerminalContent, native Content/Viewport, Frame/FrameRow,
  PlanCache, RenderState, TerminalElement, cursor/overlay and input callers.

### Documentation Action

Update required: native LLD, backend migration notes, US-0208 handoff and native
probe README. No dependency, persistence or external oneterm-vt API change.

### Reconciliation

Native LLD, backend notes, probe README and US-0208 now record the real prepaint
path and evidence, separate from complete native session/input/production parity.
Source ownership comparison also prevents a reused view from retaining another
session's reader. Effective captured colors drive padding background and cursor;
this does not implement production theme-to-native default setter mapping.

## Context

US-0237 proves typed immutable content and the cell scanner but does not route
the actual GPUI frame or plans through it. Native state must remain independent
of the legacy snapshot. The default-off render-source hook is not a production
selector, and native GUI input is explicitly not yet a supported session path.

## Plan

- [x] Add the render-source hook and connect native frame queries to prepaint.
- [x] Prove native row plans, cursor and selection in the headless GPUI fixture.
- [x] Run focused/native/default checks and full gate; reconcile evidence/gaps.

## Decisions

Follow DEC-0023 and native LLD. Input/session cutover is not inferred from paint.

## Verification Plan

Native frame unit tests and headless TerminalElement tests assert no legacy
snapshot calls, independent consumers, plan damage, cursor/selection, resize and
retained last-good/closed content. Run terminal/SSH/local/view regressions and
native all-target Clippy, scripts/ci-local.ps1, documentation and whitespace checks.
Known Enter guard, Fira Code and conhost failures remain explicit if reproduced.

<!-- HARNESS:PROOF:BEGIN -->
- [x] Unit proof
- [x] Integration proof
- [ ] E2E proof
- [x] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Harness tool is not exposed; this packet retains the work record.

- TerminalRender's default-off optional source is consumed by the real Frame
  snapshot path before legacy snapshot_into. Content is persistent per frame and
  compares publication ownership when the session changes. Engine/source changes
  force Full even if an old legacy snapshot itself reports Unchanged.
- Native damage maps None/Full/Rows to the existing plan-cache tri-state. Native
  rows keep exact projected keys; no scroll delta or ordered row id is guessed.
  Modes remain unavailable to the old input encoder (Option::None); early returns
  prevent legacy key encoding, not a claim of functioning native GUI input.
- Exact selected-cell spans feed overlays independently of glyph plans. Native
  cursor shapes/visibility/position/blinking feed the shared cursor builder; a
  steady native cursor survives the application's blink-off phase. Captured
  background and optional cursor RGB feed padding/cursor paint, respecting an
  explicit configured cursor color. Native defaults already resolve reverse-video
  in the pinned library; RGB rows are not reversed a second time by theme lookup.
- The visible paint range is clamped to both geometry and the retained viewport,
  important while native resize is asynchronous or no frame is published. Native
  ordered-history gutter labels are unavailable, not made-up line identities.
  Graphics/links remain unsupported in this native paint route, not legacy fallback.
- Three new tests pass: two real headless TerminalElement tests exercise native
  row-plan creation, Unchanged/no rebuild, one-row update, independent readers,
  steady cursor and actual cursor RGB, selection without glyph rebuild, clear,
  resize/scroll-hidden cursor, closed frame, absent initial frame and failed owner.
  Every native draw asserts zero legacy snapshot calls using the existing probe.
  A separate unit test proves native-to-native and native-to-legacy source changes
  reset damage even when dimensions match. The fixture is FakeTerminalSession
  carrying a real native model, not a complete NativeLocalSession/NativeChannel
  TerminalSession implementation or manual desktop GUI acceptance.
- Poll-error logging/retention is implemented; the headless failure test covers a
  failed owner with last-good publication, not injected ModelReader projection
  corruption. No read error falls back to legacy content.
- Native terminal338/SSH149/local focused24 pass. Native all-target Clippy for
  terminal-view, terminal, SSH and local-shell passes with warnings denied.
- Full native view450 pass/1 fail/3 ignored; default view446/1/3. Both retain the
  Fira Code DirectWrite failure at glyphs.rs:857, actual (1,1), expected (0,1).
  Logs `.tmp/us0238-native-view.log` and `.tmp/us0238-default-view.log`.
- Full native local57 pass/2 fail/3 ignored, retaining both existing conhost 0x6d
  orphan-test failures at session_orphan_tests.rs:252; `.tmp/us0238-native-local.log`.
- Full gate passes fmt and both workspace Clippy variants, then stops at core
  shell.rs:916 PowerShell Enter guard: empty stdout versus INSTALLED:AcceptLine.
  `.tmp/us0238-ci-local.log`. Later gate commands are not reached. Full acceptance
  remains blocked; the packet is not marked implemented and factories are unchanged.
- Final doc-path220, English1250, dependency graph20 and git diff whitespace
  checks pass. No dependency, persistence or external VT API changed.

## Handoff

Full native session/interaction adapters and remaining parity precede simultaneous
local/SSH factory cutover and manual GUI acceptance.
