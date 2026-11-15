# Work: Native output logging

ID: US-0255
Intake: IN-0057
Created: 2026-11-15

## Status
- [x] Planned
- [x] In progress
- [ ] Implemented
- [x] Reopened (acceptance rework)

## Classification
- Change type: new capability
- Risk lane: high-risk
- Spec Intake: IN-0057

## Outcome
Native terminals use the existing printable logging service and UI.

## Scope
- [x] In scope: automatic/manual local/SSH logging, capture and indicators.
- [x] Out of scope: schema/dependency/theme changes and production cutover.

## Acceptance
- [x] App factory regressions assert supported native logging and explicit native selection, not the retired unsupported-logging preflight.
- [x] Printable-only automatic/manual capture, final flush and failure notices.
- [x] Native capabilities and recording indicators preserve existing semantics.

## Documentation
### Owning Docs Reviewed
docs/PROJECT.md; docs/HARNESS.md; docs/templates/work.md; docs/agents code,
structure, dependency and error policies; docs/terminal-logging.md; IN-0057 HLD;
native pumps, local/SSH owners, existing logging controller and menu.
### Documentation Action
Update required: native-logging LLD and verified native capability statements.
Existing logging contract remains authoritative.
### Reconciliation
Updated PROJECT, intake HLD and native-ssh capability statements. Existing
terminal-logging.md contract remains unchanged; its controller is reused.

## Context
Existing controller strips escapes without a second terminal screen engine.
Harness discovery returned no tool; no database registration claimed.

## Plan
- [x] Wire native owners/pumps and reuse controls.
- [x] Verify and reconcile.

## Decisions
Follow low-level-design/native-logging.md and existing logging contract.

## Verification Plan
Focused capture/lifecycle/UI proof; native regression, strict native Clippy
and scripts/ci-local.ps1.

## Evidence and Gaps

Acceptance rework verified 2026-11-15: both native app factory regressions now
prove explicit native selection and supported output logging, not obsolete
pre-spawn rejection. Capability assertion precedes consuming complete().
App-inclusive native suite PASS (44 app tests, 1,292 combined passed / 7 existing
ignored), strict native Clippy and full ci-local PASS. Final evidence is
.tmp/us0259-final-native-tests.log, .tmp/us0259-final-native-clippy.log and
.tmp/us0259-final-ci.log. Release/smoke evidence is in US-0259; physical logging
menu/indicator acceptance remains deferred. This closes the automated factory
acceptance rework, not the separate physical UAT. Harness database unavailable.

2026-11-15 acceptance rework discovered during US-0258: the native app factory
tests were omitted from the earlier library-suite command and still expected
logging rejection. Update these two tests against the accepted native logging
contract, with project-local temporary logs and complete owner/event cleanup.
Do not change runtime selection or permissions. Re-run native app tests, strict
native Clippy and full local CI. Harness remains unavailable; manual record only.
2026-11-15: local automatic logging test captures actual child output and
observes final flush. Async pump tests prove fragmented control exclusion,
manual start, partial final flush and drop flush with a retained UI controller.
The combined two-hop SSH/SFTP/forward test proves automatic SSH logging excludes
OSC title data and stops at completion. Native Log submenu uses existing
Start/Stop and elevation guard; native rendering consumes existing one-shot
errors and exposes the split recording indicator, while tab state reuses
capabilities. Existing logger tests retain failure/collision/precedence proof.
Native terminal 365/SSH 176 PASS (.tmp/native-parity-security-tests.log);
local 76 PASS/3 existing ignored in the earlier full native run. Physical
menu/indicator acceptance remains separate. Final gate/release evidence pending.

Earlier verification (before app-factory acceptance rework): native library suites PASS (76/109/176/365/522, 7 existing
ignored), strict native app all-target Clippy PASS and complete ci-local PASS.
Logs: .tmp/native-parity-tests.log, native-parity-clippy.log and
native-parity-ci.log. Shared local Release and smoke evidence is in US-0257.
No production cutover or physical menu acceptance is claimed. Harness database
registration is unavailable; status is recorded in Markdown only.

## Handoff
Same-session implementation.
