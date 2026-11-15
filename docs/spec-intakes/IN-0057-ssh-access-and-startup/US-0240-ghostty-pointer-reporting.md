# Work: Native surface pointer reporting

ID: US-0240
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

The independent native GPUI surface forwards pointer press/motion/release through
the same object-safe session contract and existing bounded local/SSH owner queue.
Only the live native encoder chooses reporting modes; no legacy mode default or
pre-encoded bytes are supplied by the UI.

## Scope

- [x] In scope: typed mouse admission on SurfaceSession, GPUI three-button
  reporting, device-pixel grid geometry, owned press/release cleanup, error callback
  and native owner/real endpoint/headless proof.
- [x] Out of scope: wheel routing, local selection, clipboard, URL opening,
  context menu, application settings binding, extra buttons, production panel and
  factory selection, desktop UAT. These need independently verified policies.

## Acceptance

- [x] Local and SSH forward exact typed validation/admission errors on the same
  bounded input FIFO; successful admission is not delivery confirmation.
- [x] Native surface uses last-painted grid geometry converted to device pixels,
  retains fractional positions, excludes padding/outside cells, and ignores absent
  or differently sized publications rather than inventing a grid.
- [x] Live native encoder selects mouse modes. Shift bypasses pointer press and
  motion for future local selection; an admitted press still receives a matching
  release after Shift or pointer departure. Unowned releases are not fabricated.
- [x] Held button cleanup on blur is bounded, observable, once-only and never
  retried implicitly. Rejected presses are not held; typed errors reach caller.
- [ ] Native model/headless pointer tests and real SSH/local trait-object proof
  run; native Clippy, regressions and full gate evidence are reconciled.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`, required agent
  guides, native HLD/LLD, DEC-0023 and US-0239.
- `crates/terminal-view/src/input/mouse.rs` and `render/metrics.rs`: existing
  Shift override and fractional grid hit test; no legacy mouse machine reuse.
- Native input validation/encoder, local/SSH input queues and SurfaceSession.
- GPUI Kit reference/srcwalk missing as recorded in US-0239; inspect existing
  project handlers and cached pinned GPUI event declarations instead.

### Documentation Action

Update native LLD, backend notes, probe README and US-0208 handoff. No persistence,
dependency or external oneterm-vt public API change.

### Reconciliation

Updated native LLD, `docs/terminal-backend.md`, probe README and US-0208 handoff.
No dependency/persistence/external oneterm-vt API change. Work remains in progress
until full-gate and desktop acceptance gaps are closed.

## Context

SurfaceSession already carries native rendering, atomic resize and keys/IME.
Both explicit endpoints already admit typed MouseEvent; this packet connects
that boundary, not another parser. Pointer coordinates describe a padding-free
device-pixel grid, consistent with the same cell metrics sent by prepaint resize.
Old content can remain visible after resize, so dimension mismatches reject input
until a matching publication. Caller still owns lifecycle/reliable events/security.

## Plan

- [x] Extend the shared contract and both endpoint implementations.
- [x] Add native pointer geometry/state/handlers and focused tests.
- [ ] Run proof, regressions/gates and reconcile records.

## Decisions

DEC-0023 and native LLD. Mouse reporting is not local selection or wheel parity.

## Verification Plan

Headless native surface: finite fractional/device coordinates, bounds/resize gaps,
Shift, motion with/without owned buttons, release outside/blur, rejection and
error callback. Real SSH server checks native SGR bytes after enabling/disabling
reporting; local trait-object admission and terminal encoder tests cover both
transport implementations. Run native terminal/SSH/local/view tests and all-target
Clippy; run `pwsh scripts/ci-local.ps1` and doc/English/dependency/diff checks.
Retain existing core Enter guard/Fira Code blockers rather than weakening tests.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Harness tool is unavailable; packet is the durable record.

- Native SSH151/terminal339 pass: `cargo test -p oneterm-ssh -p oneterm-terminal
  --features oneterm-ssh/ghostty-native --quiet`. New real SSH SurfaceSession test
  checks disabled reporting, live SGR enable, exact `ESC[<0;2;2M` and release `m`,
  disable again, invalid NaN and closed admission. Log: `.tmp/us0240-backends.log`.
- Native local focused24 pass: `cargo test -p oneterm-local-shell --features
  ghostty-native native_local_ --quiet`. Existing strict Unicode trait-object
  fixture now also admits mouse with reporting disabled, rejects NaN and forwards
  Closed without disturbing child ReadConsoleW Unicode delivery.
  Log: `.tmp/us0240-local.log`.
- Native surface focused5 pass: `cargo test -p oneterm-terminal-view --features
  ghostty-native native_surface -- --nocapture`. New pointer test uses real model
  and no legacy TerminalSession. It checks absent/mismatched grid, Shift,
  fractional device coordinates, press/move/release, outside-up after Shift,
  duplicate suppression, cleanup and rejection. It also dispatches a real GPUI
  middle-button press and outside release to prove callback wiring. Geometry test
  covers four DPI scales, padding, bottom/right boundaries and NaN/infinity.
  Final focused rerun also proves release during a publication resize gap keeps
  admitted geometry, and rejected cleanup reports once without retaining/retrying.
  Log: `.tmp/us0240-surface.log`.
- Native local/SSH/view all-target Clippy with `-D warnings` passes.
  Log: `.tmp/us0240-native-clippy.log`.
- Full native view455 pass/1 fail/3 ignored. Existing Fira Code DirectWrite
  assertion at `render/glyphs.rs:857` remains actual `(1, 1)`, expected `(0, 1)`.
  Log: `.tmp/us0240-native-view.log`.
- Full `pwsh scripts/ci-local.ps1`: fmt and both workspace Clippy variants pass.
  Workspace tests fail unchanged core PowerShell Enter guard at `shell.rs:916`,
  empty stdout instead of `INSTALLED:AcceptLine`. Later steps do not run.
  Log: `.tmp/us0240-ci-local.log`. Complete gate is not green.
- Doc paths220, English1257, dependency graph20 and whitespace checks pass.
- Real desktop blur dispatch remains unproven; headless tests prove the drain
  operation and outside-up wiring, not active-window UAT. Full local orphan suite
  was not rerun; previous conhost failures remain open. No wheel/local-selection/
  clipboard/menu behavior or production selection is implied.

## Handoff

Wheel routing, selection/search/paste, complete panel/event/security integration
and remaining parity precede simultaneous production local/SSH cutover and GUI UAT.
