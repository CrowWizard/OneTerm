# Work: Native SSH jump routes

ID: US-0256
Intake: IN-0057
Created: 2026-11-15

## Status
- [x] Planned
- [ ] In progress
- [x] Implemented

## Classification
- Change type: new capability
- Risk lane: high-risk
- Spec Intake: IN-0057

## Outcome
Native SSH uses configured ordered authenticated jump routes.

## Scope
- [x] In scope: existing route reuse, isolated trust and native carrier lifetime.
- [x] Out of scope: new auth/schema/dependencies and production cutover.

## Acceptance
- [x] Ordered hops preserve independent trust/auth and cancellation.
- [x] Target/SFTP teardown precedes reverse carrier destruction.

## Documentation
### Owning Docs Reviewed
docs/PROJECT.md; docs/HARNESS.md; docs/templates/work.md; docs/agents code,
structure, dependency and error policies; IN-0057 HLD/native-ssh LLD; route.rs,
legacy session routing and native channel ownership.
### Documentation Action
Update required: native-jump-routes LLD and verified capability statements.
### Reconciliation
Updated PROJECT, intake HLD and native-ssh ownership contract. Four-hop/cycle
validation remains in the existing session-ui route resolver and its tests.

## Context
Reuse JumpHandles rather than independent connections without carrier owners.
Harness discovery returned no tool; no database registration claimed.

## Plan
- [x] Wire route and native lifetime.
- [x] Verify and reconcile.

## Decisions
Follow low-level-design/native-jump-routes.md.

## Verification Plan
In-process hop success/trust/auth/cancellation/lifetime proof; native regression,
strict native Clippy and scripts/ci-local.ps1.

## Evidence and Gaps
2026-11-15: native integration proves two ordered carriers with SFTP, logging
and all forward kinds on the same target login. Separate native proof checks
unknown hop then unknown target, trust recorded only in the target fixture's
isolated file, rejected hop auth and cancellation during held hop auth before
target authentication. Native owner retains Services through SFTP completion,
drops target handle, then drops reverse-order JumpHandles. Setup failure drops
owned handles and cancels services; original phase/overall deadlines remain.
SSH 176 PASS (.tmp/native-parity-security-tests.log). External-server desktop
and physical active-operation shutdown acceptance remain separate. Final
combined gate/release evidence pending.

Final verification: native library suites PASS (76/109/176/365/522, 7 existing
ignored), strict native app all-target Clippy PASS and complete ci-local PASS.
Logs: .tmp/native-parity-tests.log, native-parity-clippy.log and
native-parity-ci.log. Shared local Release and smoke evidence is in US-0257.
No external saved host or credential was used. Harness database registration
is unavailable; status is recorded in Markdown only.

## Handoff
Same-session implementation.
