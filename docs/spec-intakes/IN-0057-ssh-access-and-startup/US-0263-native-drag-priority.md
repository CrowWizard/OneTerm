# Work: Native drag priority preference

ID: US-0263
Intake: IN-0057
Created: 2026-11-16

## Status

<!-- HARNESS:STATUS:BEGIN -->
- [x] Planned
- [ ] In progress
- [x] Implemented
- [ ] Changed
- [ ] Reopened (acceptance rework)
- [ ] Retired
<!-- HARNESS:STATUS:END -->

## Classification

- Change type: new capability
- Risk lane: high-risk review for persisted preference compatibility; existing
  persistence mechanics and permission checks remain unchanged.
- Spec Intake: IN-0057, amended HLD and drag-priority LLD.

## Outcome

Users can choose Selection First or TUI Mouse First for native left dragging
in Settings, with live application and persistence across restarts.

## Scope

- [x] In scope: typed preference, existing Mouse group dropdown, native selection
  admission and focused regression coverage.
- [x] Out of scope: legacy session input changes, other mouse buttons, wheel,
  IME, process detection, automatic preference changes and clipboard-policy changes.

## Acceptance

- [ ] Both options are visible and editable in Terminal > Mouse.
- [ ] Missing preference preserves TUI-first; both values round-trip.
- [ ] Selection-first allows plain left drag in mouse-reporting TUIs.
- [ ] TUI-first preserves application mouse routing and Shift selection.
- [ ] Gesture ownership survives mid-drag preference changes.
- [ ] Automatic clipboard copying still follows Copy on Select.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md` and required agent guides.
- `docs/agents/persistence.md`: additive schema and background saves.
- Intake HLD and `low-level-design/drag-priority.md`: accepted ownership.
- Existing mouse config, live mappings, Mouse UI and native selection tests.

### Documentation Action

Amend intake/HLD, add the focused LLD, and document the additive preference in
persistence and native engine contracts. The pinned UI reference checkout is
absent; use the installed 0.8.0 SettingField source and existing Layout dropdown.

### Reconciliation

Updated intake/HLD, added the focused drag-priority LLD, and reconciled native
engine and persistence documentation. Missing settings retain TUI-first; no
existing user configuration was changed.

## Context

The owner explicitly requested the two-option setting after Codebuff acceptance.
Default remains TUI-first to avoid disabling existing application mouse controls.
Harness tools are not exposed; records follow the editable templates manually.

## Plan

- [x] Add preference, settings UI and native selection gate.
- [x] Add config round-trip and native gesture regression coverage.
- [ ] Run static checks only, reconcile, commit and push.

## Decisions

Use the existing settings entity and background writer; do not change actual
user config or introduce application-specific mouse detection.

## Verification Plan

Format and diff checks, English/doc-path checks, and source review of all config
initializers. Add focused tests without executing them locally. Native cloud
compilation and manual Codebuff validation remain outstanding.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [x] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

No local compilation or test execution per user instruction.
Formatting, diff whitespace, documentation-path (220 paths) and English-text
(1331 files) checks passed. Config tests cover old-field absence and both values
through JSON/live settings. Native source tests cover default TUI selection
rejection, Shift override, selection-first without Shift, mid-gesture changes,
and preserving the clipboard when Copy on Select is off. These tests were added
but not executed locally. Runtime acceptance and cloud build status remain open.
