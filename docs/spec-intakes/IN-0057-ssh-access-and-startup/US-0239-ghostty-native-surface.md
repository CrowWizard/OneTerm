# Work: Native session surface and typed input

ID: US-0239
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

A backend-independent native GPUI surface renders and accepts typed key/IME input
through real local/SSH endpoints without requiring any implementation of legacy
TerminalSession. The shared native surface contract preserves admission failures,
published-state absence and owner-thread encoding.

## Scope

- [x] In scope: object-safe SurfaceSession on both native endpoints, damage-free
  published queries, direct native render/resize route, focus/key release and IME
  handler/candidate geometry, typed observable input errors and focused proof.
- [x] Out of scope: production panel/factory selection, event/security consumer
  replacement, application shortcuts/broadcast/completion, paste/selection/search,
  wheel/mouse gesture policy, auto-bottom/history/logging and manual GUI UAT.

## Acceptance

- [x] Native surface needs no legacy session or power-on query/mode defaults.
  Local/SSH implement the same object-safe contract with exact error forwarding.
- [x] Non-render query retains one atomic published viewport/state and cannot
  advance render damage, capture, acknowledge repaint or enter native code.
- [x] Native prepaint uses atomic resize metrics and reports admission errors;
  render reads keep independent damage and never call legacy snapshots.
- [x] Functional key press/repeat/release use native owner encoding. Layout keys
  never fabricate physical locations; plain primary-screen text stays on IME.
- [x] IME preedit/commit retains typed limits and errors, requests redraw, and
  candidate bounds conservatively anchor at the published native cursor, not
  legacy history coordinates or guessed Unicode cell widths. Exact preedit-range
  geometry remains deferred until native grapheme layout is available.
  Alternate-screen input does not produce duplicate text.
- [ ] Headless surface and real local/SSH trait-object fixtures pass; native
  regressions/Clippy/full gate run and documentation records remaining gaps.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`, required agent
  guides, DEC-0023, native HLD/LLD and US-0237/US-0238 packets.
- NativeLocalSession/NativeChannel input/IME/control admission, SharedModel and
  Content, TerminalElement/Frame, GPUI input and existing TerminalView IME.
- GPUI Kit pinned reference: srcwalk is unavailable; inspect existing local
  input-handler/render patterns and pinned sources instead of web research.

### Documentation Action

Update required: native LLD, backend notes, probe README and US-0208 handoff.
No dependency/persistence or external oneterm-vt API change.

### Reconciliation

Updated native LLD, `docs/terminal-backend.md`, `scripts/ghostty-vt/README.md` and
US-0208 handoff. Exact preedit-range geometry and inline preedit painting are
explicitly deferred instead of using legacy width tables as a native substitute.
No persistence/dependency/external oneterm-vt API change. Packet remains in progress
because full-gate and real-window acceptance are not green.

## Context

The old TerminalSession requires legacy history/query/selection APIs that native
endpoints cannot honestly implement yet. The native surface is an embeddable
render/input core, not a second parser or a replacement for all panel behavior.
Its caller retains lifecycle and reliable SessionEvent/security handling and
must invalidate it on publication. The surface must not take/drop those events.

## Plan

- [x] Add native surface/query contract and implement both endpoints.
- [x] Connect the native render path and GPUI key/IME surface without legacy fakes.
- [ ] Verify real endpoints and headless surface, run gates and reconcile docs.

## Decisions

Native LLD and DEC-0023. Do not claim the optional surface is production parity.

## Verification Plan

Prove trait-object key/commit/resize admission using real local and SSH fixtures.
Headless GPUI exercises render/query/functional keys/repeat/release, composition,
alternate-screen text ownership, Unicode cursor anchoring and typed rejection.
Run terminal/SSH/local/view native regressions, native all-target Clippy and full
ci-local.ps1, then doc-path/English/dependency/whitespace checks. Retain known
PowerShell Enter guard, Fira Code and conhost failures if reproduced.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Harness tool discovery returns no tool; this packet retains the work record.

- `cargo test -p oneterm-ssh -p oneterm-terminal --features
  oneterm-ssh/ghostty-native --quiet`: SSH150 and terminal339 pass, including new
  real SSH SurfaceSession resize/key/exact Unicode commit/invalid/closed proof and
  query capture/damage/atomic retained publication/closed-absent proof.
  Log: `.tmp/us0239-native-backends.log`.
- `cargo test -p oneterm-local-shell --features ghostty-native native_local_
  --quiet`: 24 pass. Strict ReadConsoleW fixture now uses the SurfaceSession trait
  object for atomic resize, preedit, combining/Japanese commit and owner-encoded
  Enter, then checks closed key admission. Supplementary Unicode still passes.
  Log: `.tmp/us0239-native-local.log`.
- `cargo test -p oneterm-terminal-view --features ghostty-native native_surface
  -- --nocapture`: 3 pass, two GPUI tests and pure mapping proof. Fixture implements
  only SurfaceSession with a real native model, not legacy TerminalSession. It
  observes actual prepaint/paint/resize/repaint ack, press/repeat/release/cleanup,
  primary IME, candidate-key ownership, Unicode cursor anchoring, alternate-screen
  no duplicate commit, absent metadata and typed key/IME/resize rejection. Preedit
  survives oversize rejection. Log: `.tmp/us0239-native-surface.log`.
- Full native view: 453 pass, 1 fail, 3 ignored. Existing Fira Code DirectWrite
  assertion at `render/glyphs.rs:857`, actual `(1, 1)`, expected `(0, 1)`.
  Log: `.tmp/us0239-native-view.log`. No weakening or ignoring this regression.
- Native local/SSH/view all-target Clippy with `-D warnings` passes; terminal is
  included as their dependency. Log: `.tmp/us0239-native-clippy.log`.
- Full `pwsh scripts/ci-local.ps1`: fmt and both workspace Clippy variants pass;
  workspace tests stop at existing core PowerShell Enter guard, `shell.rs:916`,
  powershell.exe default stdout empty instead of `INSTALLED:AcceptLine`. Later
  gate steps do not run. Log: `.tmp/us0239-ci-local.log`. An initial fmt failure
  was corrected before this rerun. The complete gate is not green.
- Doc paths220, English1255, dependency graph20 and `git diff --check` pass.
- Real-window focus/blur subscription is not proven by GPUI's inactive test
  window; the drain operation is proven independently. Manual desktop IME/render
  acceptance, exact Unicode preedit layout, inline preedit painting and full
  production event/security consumer remain open. No manual GUI claim or selector.
- Full native local orphan suite was not rerun this packet; previous US-0238
  conhost 0x6d failures remain unresolved, not newly verified or fixed here.

## Handoff

Selection/search/paste/shortcuts/broadcast/scroll policy, full panel event/security
integration and remaining parity still precede production local/SSH cutover.
Next add native interaction contracts and caller event/security integration without
restoring legacy RowId/mode guesses. Resolve the full-gate blockers, then switch
local and SSH production factories together and perform desktop GUI acceptance.
