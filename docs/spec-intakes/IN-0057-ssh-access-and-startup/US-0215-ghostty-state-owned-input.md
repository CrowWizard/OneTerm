# Work: State-owned Ghostty input encoding

ID: US-0215
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

Encode native keyboard and mouse events on the same worker that owns terminal
state, using upstream set_from_terminal rather than stale UI mode snapshots.
The transport pump delivers encoded input once through its originating transport.
Paste retains OneTerm's existing size/sanitization policy and reads native mode.

## Scope

- [x] In scope: owned bounded key/mouse requests, confined RAII event/encoder
  handles, native state-derived encoding, worker serialization and pump delivery.
- [x] Out of scope: production selector, complete platform physical-key mapping,
  selection gestures, Kitty clipboard paste events, asynchronous command bridge.

## Acceptance

- [x] Keyboard encoding follows live cursor/backarrow/modifyOtherKeys/Kitty state
  without a legacy mode fallback or second terminal.
- [x] Mouse encoding honors live tracking/format including SGR pixel coordinates.
- [x] Input requests and encoded output are bounded; invalid requests cause no
  transport write, truncated output or terminal mutation.
- [x] Native handles/text borrows never escape worker calls; errors remain explicit.
- [x] Pump delivers generated input exactly once and fail-stops on write rejection.
- [x] Paste reads native mode while preserving existing paste policy and cap.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`, agent conventions,
  structure, dependency rules, error policy and dependencies.
- IN-0057 HLD, `low-level-design/ghostty-engine.md`, US-0208/0213/0214.
- `terminal/src/session.rs`, `model.rs`, `paste.rs`, `ghostty/owner.rs`,
  `backend/ghostty_pump.rs`, `vt/src/input/key.rs`, `terminal-view/src/input/keys.rs`.
- Pinned `key/event.h`, `key/encoder.h`, `mouse/event.h`, `mouse/encoder.h`,
  `paste.h`: native set_from_terminal, borrowed UTF-8, sized geometry and encoding.

### Documentation Action

Update required: Ghostty LLD, US-0208 handoff and probe README. Production
local/SSH facts remain unchanged until their complete contracts can be switched.

### Reconciliation

Ghostty LLD, US-0208 handoff and probe README now describe state-owned keyboard,
mouse and paste encoding and its limits. Standalone native coverage includes
named functional keys, every exposed mouse button, padding/modifiers and live
paste mode. No production contract change is implied; the full gate remains red.

## Context

Native key encoder reads modifyOtherKeys and all native keyboard flags directly,
including modes absent from Metadata. Mouse input supplies explicit surface pixel
geometry and button state. Stateless per-request encoder handles avoid stale
mode/deduplication state; motion deduplication is not claimed. No clipboard
callback is installed by this work and native file media remain disabled.

## Plan

- [x] Implement bounded typed native key/mouse requests and C ABI guards.
- [x] Serialize encoding with feed/resize and deliver through GhosttyPump.
- [x] Preserve paste policy using live native bracketed-paste mode.
- [x] Verify modes, screen/reset, ownership, bounds and rejected delivery.

## Decisions

DEC-0023. Native parser/state ownership and existing transport ownership remain.

## Verification Plan

Native terminal tests, native terminal/view all-target Clippy, C ABI and standalone
probes, renderer regressions, full ci-local.ps1 and documentation/graph checks.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

2026-11-11: native terminal suite 263 passed; native terminal/view all-target
Clippy passed with warnings denied; renderer frame fixtures 9 passed. The C ABI
and runtime probe passed; standalone Rust runner compiled with warnings denied
and passed 48 tests. Added real functional-key, wheel/button, modifier/padding
and live paste-mode fixtures instead of suppressing standalone dead-code errors.

The complete ci-local.ps1 rerun passed fmt and both workspace Clippy variants,
then failed at unchanged config::shell::tests::powershell_enter_guard_decides_the_four_bindings:
powershell.exe default verdict was empty, expected INSTALLED:AcceptLine (92 core
tests passed, 1 failed). Subsequent gate steps did not run. Shell log:
sh_1c8e85f5b001ZWOU8IYXW2j2Cx. Production input mapping and session cutover remain
unimplemented; these results prove staging only. Harness tool discovery returned
no tools, so no database status/proof changes are claimed.
Doc-path, English contributor-text, dependency-graph and whitespace checks passed.

## Handoff

US-0208 retains production local/SSH cutover and complete platform input mapping.
US-0216 adds the real tracked-history position seam for subsequent model work.
Do not treat local focused acceptance as a green project quality gate.
