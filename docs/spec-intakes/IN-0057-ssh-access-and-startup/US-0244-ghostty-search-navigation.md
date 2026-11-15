# Work: Native terminal search and navigation

ID: US-0244
Intake: IN-0057
Created: 2026-11-12

> Pre-code gate: complete Outcome, Scope, Acceptance, Documentation, and Verification Plan before editing implementation files. Harness synchronizes only the marked status/proof blocks; keep authored checklists current.

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

- Change type: new capability
- Risk lane: normal (bounded native interaction; no production cutover)
- Spec Intake: IN-0057, accepted HLD and `low-level-design/ghostty-engine.md`.

## Outcome

The independent native surface provides an accessible, bounded in-buffer search
bar with next/previous navigation and correct highlights over native rows, without
retaining match coordinates across terminal mutations or enabling production use.

## Scope

- [x] In scope: Ctrl+F toggle, query editing/debounce, bounded incremental native
  search, match count/state, next/previous navigation, current/all match painting,
  stale-result rejection/refresh, close/clear/focus handling, and local/SSH seam.
- [x] Out of scope: case-sensitive/whole-word options unsupported by pinned native
  search, regex, search-history persistence, legacy view changes, production
  factory/panel selection, broader menu/broadcast/settings work, and desktop UAT.

## Acceptance

- [x] Search is serialized through each real local/SSH SurfaceSession owner path;
  rejected/full/closed requests are visible and never implicitly replayed.
- [x] Needle is limited to 4096 UTF-8 bytes and result display to 4096 matches;
  unsupported option semantics are not silently claimed.
- [x] Incremental search terminates at Complete/FeedRequired policy or a bounded
  UI work limit; typing is debounced and superseded queries cannot overwrite newer
  results.
- [x] UI navigation wraps next/previous, updates active highlight/counter, and
  reveals the chosen match using native viewport scrolling, not legacy RowIds.
- [x] Match coordinates are tied to the query generation and native source/grid
  revision; terminal mutation invalidates them before paint/navigation. Refresh
  never follows stale coordinates, and active index handling is defined on refresh.
- [x] Clear/close supersedes pending UI work without cancelling admitted endpoint
  futures, clears native search state, and
  restores terminal focus; search input keystrokes do not leak to the shell.
- [x] Focused owner/pump/surface tests, native endpoint regressions, native Clippy,
  doc/English/dependency/diff checks and full-gate result are recorded. Desktop UAT
  and unrelated pre-existing gate failures are explicit.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`, required agent
  guides, native HLD/LLD, DEC-0023 and US-0208/0234/0242/0243.
- Legacy `terminal_view/search.rs`, native owner/search/model/bridge, SurfaceSession
  endpoints, native surface/input/overlay, relevant tests and backend notes.
- Pinned GPUI Kit reference lookup was attempted first, but `reference/gpui-kit` is
  absent and `srcwalk` is unavailable; use the existing legacy search UI as local
  API/pattern reference without adding dependencies.

### Documentation Action

- Update required: native LLD before implementation, then terminal backend notes,
  probe README and US-0208 handoff. No schema/dependency/public VT API change.

Reason: this extends the explicitly staging-only native surface with interactive
search while honoring pinned native search limitations and coordinate lifetime.

### Reconciliation

Updated native LLD (serial driver, coordinate provenance, reveal, release cleanup,
limits), HLD search wireframe, docs/terminal-backend.md, scripts/ghostty-vt/README.md
and the US-0208 handoff. No persisted schema, dependency, external VT API or
production selector changes. The accepted native architecture and DEC-0023 remain
unchanged; the implementation refines their staging interaction seam.

## Context

Native snapshots return newest-to-oldest active-screen coordinates, with ASCII
case-insensitive matching, 4096-byte needle and 4096-match caps. Coordinates are
ephemeral; upstream native search options do not support case-sensitive or
whole-word search. Do not convert coordinates to durable anchors. UI search work
must stop/refresh on owner-published grid revision changes.

The native search object's internal result storage is not bounded by the 4096
response cap. A bounded UI progress budget and owner-side per-response cap do not
claim native internal memory parity; this remains an explicitly accepted staging
limitation, not a promise to retain arbitrary long-lived search instances.

## Plan

- [x] Specify and implement a bounded owner search seam with explicit cancellation
  policy and real
  local/SSH forwarding.
- [x] Add independent-surface search state, bar, query debounce and navigation;
  bind results to source/revision and paint native highlights.
- [ ] Prove limits, ordering, invalidation, navigation/scroll, cleanup and reconcile
  checks and handoff documents.

## Decisions

DEC-0023 and accepted native LLD. Search coordinates are not durable anchors.

## Verification Plan

Native search/pump tests for caps, progressive status, clear, source/revision
staleness and concurrent mutation; native surface UI tests for debounce, query
supersession, limits, no-result state, wrapping navigation, visible reveal,
highlight state, close/focus and no key leakage. Real local/SSH trait-object
operations, native terminal/SSH/local/surface regressions, all-target native
Clippy, full ci-local and doc paths/English/dependency/diff checks.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Acceptance continuation: exercise the real InputState EntityInputHandler for
ordinary edits, Unicode composition/commit and clearing, including its Change
subscription rather than assigning Search.query in the test. This is GPUI widget
integration proof, not OS IME/UAT. Also release a non-root surface while its search
acknowledgment is held, prove no early second operation, then release the gate and
prove ordered clear and continued native owner usability. Re-run focused proofs,
native Clippy and the bundled gate; retain existing gate failures as blockers.
Window-key routing additionally exposed unchanged Change after PressEnter,
superseding navigation and resetting the active index. The Change subscription
must compare input text before replacing the query; retain Enter/Shift+Enter
window-event regression proof. This is acceptance rework in the still-in-progress
packet, not a separate shipped defect.

Native build uses the existing .tmp/ghostty-migration/install with
GHOSTTY_VT_PREFIX, project-local Cargo cache, target and temp, and four build jobs.
The initial missing-artifact conclusion was wrong: the hidden directory was
omitted from the search. No native rebuild was required.

Automated evidence:

- Native terminal/SSH regression: 347/154 passed in .tmp/us0244-backends.log.
- Focused native local regression: 26 passed in .tmp/us0244-local.log (before
  adding the new queue-pressure test; not the full local-shell/orphan suite).
- Final search suite across local/SSH/view: 2/2/17 passed in
  .tmp/us0244-search-lifecycle-proof.log. Real local and SSH tests prove trait
  forwarding, reveal, clear, bounded admission, QueueFull without cancellation,
  and fail-stop cancellation after admission with queued acknowledgments closed.
- The surface fixture retains real native snapshot provenance, but scripts Running
  and FeedRequired statuses: it proves exactly 63 follow-up steps after one Set,
  no autonomous continuation after the 64-operation budget, and no Step after
  FeedRequired. This is UI policy proof, not native performance or memory proof.
- Delayed endpoint acknowledgment proves supersession and close preserve the
  accepted future and serialize clear afterward. It is not transport admission
  pressure proof; the separate real endpoint tests provide that evidence.
- The same surface proof checks mutation before capture, navigation wrap and
  refresh index reset, oversized Unicode needle cleanup, focus return, bubbled
  shortcut isolation, and actual idle non-root entity release/native clear.
  Soft-wrap and full-width endpoint highlighting has a separate native test.
- Final full native view: 462 passed, 1 failed, 3 ignored in
  .tmp/us0244-view-final.log. The introduced render boundary failure was fixed by
  moving native cell-width interpretation into render/frame.rs; its structural
  test now passes. The remaining failure is the previously recorded Fira Code
  DirectWrite mismatch at glyphs.rs:857, (1, 1) versus (0, 1).
- Final four-crate native all-target Clippy with -D warnings passes in
  .tmp/us0244-native-clippy-lifecycle-final.log.
- fmt check, doc paths (220 paths), English (1265 files), dependency policy
  (20 packages) and git diff --check pass. No dependency additions.
- Full ci-local previously passed fmt and both workspace Clippy variants, then
  stopped in core PowerShell Enter guard: 92 passed/1 failed, empty output versus
  INSTALLED:AcceptLine at shell.rs:916 (.tmp/us0244-ci-local.log). The final rerun
  after release cleanup fails at the same assertion with 92 passed/1 failed
  (.tmp/us0244-ci-lifecycle-final.log). Later gate steps did not run.

First-run/rework context: the original deterministic UI test did not advance the
GPUI clock; advancing it fixed the timeout. The idle release test initially
dropped its entity outside an application update and did not flush release effects;
dropping inside the update passed. Test compilation also caught a missing
AppContext import, the VisualTestContext update closure arity, and use of the SSH
endpoint after consuming wait; these were corrected before the final proofs.

Input/lifetime continuation results:

- .tmp/us0244-input-routing-fixed.log: all three native search tests pass.
  The new widget test uses simulated window typing, real InputState UTF-16
  composition/replacement APIs, supplementary-plane/combining characters, and
  Change subscription delivery. Simulated Enter/Shift+Enter now navigate 0/1/0
  without native key, commit, or paste leakage. Empty input clears native results.
- The first window Enter test failed despite observing PressEnter: an unchanged
  Change followed it and reissued the query. Comparing the new input text fixes
  the root cause; the test retains event diagnostics, not a timing workaround.
- The lifecycle test now releases an entity during its delayed search operation.
  The weak entity expires, no second operation starts before acknowledgment, then
  exactly one clear follows and the native owner accepts another nonempty search.
  The delayed endpoint fixture does not itself model native admission cancellation;
  the already-passing real local/SSH pressure tests cover that separate boundary.
- Final full native view is now 463 passed, 1 failed, 3 ignored in
  .tmp/us0244-input-view-full.log; only the same Fira Code mismatch remains.
  Four-crate native all-target Clippy passes (.tmp/us0244-input-clippy-final.log).
  Full gate rerun (.tmp/us0244-input-ci-final.log) again passes fmt and both
  workspace Clippy variants, then fails core Enter guard, 92 passed/1 failed,
  empty output versus INSTALLED:AcceptLine. Later gate steps did not run.

Remaining gaps: OS IME/click interaction acceptance, desktop
search/pointer/wheel/focus/clipboard/render UAT, complete gate and earlier
conhost/OSC110/parity blockers. The native internal result allocation is still
not bounded by the response cap. No production migration or desktop acceptance
is claimed. Harness tool discovery returned no available harness tool in this
session; machine-owned proof/status blocks were not manually rewritten.

Desktop entry limitation: NativeTerminalSurface is currently instantiated only
by unit-test fixtures, not by an application or standalone GUI probe. Compiling
the normal app would still exercise the legacy production surface. Desktop native
UAT therefore needs a dedicated opt-in launcher with real event/security/lifecycle
ownership; do not falsely present a normal app run as native acceptance.

US-0245 continuation supersedes the entry limitation above: a separate default-off
local native launcher now builds and has policy/event/owner-shutdown proof. Its
actual launch in this environment was refused by the restricted-token gate, so
real search/IME/pointer/clipboard desktop UAT and GUI lifetime proof remain open.

## Handoff

Keep US-0244 In progress. Widget input/IME entry points and busy entity destruction
now have automated proof. Next: establish a deliberate staging desktop launcher,
resolve or attribute gate failures, and complete real desktop/OS IME acceptance.
US-0208 still requires panel/events/security, broader menu/broadcast/history/theme
and transport/parity work before simultaneous local/SSH production cutover. All
changes remain uncommitted; preserve the other in-flight work in this checkout.
