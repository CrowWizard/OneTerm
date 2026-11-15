# Work: Administrator functional parity

ID: US-0259
Intake: IN-0057
Created: 2026-11-15

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
- Risk lane: high-risk (authorization, secrets, persistence and execution)
- Spec Intake: IN-0057; informed owner approval on 2026-11-15.

## Outcome
Confirmed administrators have ordinary-window application functionality. Unknown
tokens stay fail-closed; real token identity and administrator marking remain.

## Scope
- [x] In scope: Session CRUD/groups/startup/passwords, settings/layout, vault, logging, configured Shell/editor execution, updates and production startup effects.
- [x] Out of scope: bypassing UAC/UIPI/ACLs, plaintext secrets, host-key bypass, native engine cutover, isolated entry external startup effects, deferred GPUI shutdown defect.

## Acceptance
- [x] Confirmed ordinary/elevated tokens have identical application permissions; Unknown remains restricted.
- [x] Preserve token marking, explicit UAC launch, separate crash storage and all ordinary domain/integrity checks.
- [x] Retire create-only Session runtime branch; ordinary save workflow applies.
- [x] Enable production startup/update/crash UI, not isolated test-entry external effects.
- [x] Focused and full CI/native regression/strict native Clippy pass; Release build and owned data-preserving smoke checked.

## Documentation
### Owning Docs Reviewed
- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md` — invariants and gate.
- `docs/agents/persistence.md` — owner writes and unreadable-file protection.
- `docs/decisions/DEC-0019-elevated-shells-open-in-an-elevated-window.md` — policy.
- `docs/spec-intakes/IN-0057-ssh-access-and-startup/high-level-design.md` — architecture.
- `docs/spec-intakes/IN-0057-ssh-access-and-startup/low-level-design/security-and-startup.md` — previous policy.
- `docs/spec-intakes/IN-0057-ssh-access-and-startup/low-level-design/administrator-session-create.md` — superseded exception.
### Documentation Action
Update required: permission/persistence contracts and affected user-facing descriptions.
Reason: owner selected full parity after being warned about credential, execution and update risks.
### Reconciliation
Updated PROJECT, DEC-0019, intake HLD/security and administrator detail designs,
persistence contract, GUI layout, logging, authentication, update and crash docs.
Replaced the persistence guide's contradictory historical create-only/deny table
with the current three-token contract. US-0258 is explicitly superseded.

## Context
Preserve identity; change the central permission predicate to Unknown-only, and
audit identity callers. Do not spoof elevation. Harness tool discovery previously
returned none; use manual template records, never modify harness.db directly.

## Plan
- [x] Amend central policy and retire narrow create-only branch.
- [x] Audit identity callers; prove parity and Unknown fail-closed.
- [x] Run gates, reconcile and build/smoke Release.

## Decisions
DEC-0019 amendment; low-level-design/administrator-functional-parity.md.

## Verification Plan
Three-token permission/action tests; isolated persistence/Unknown tests; full
scripts/ci-local.ps1; native local/SSH/terminal/session/view/app regressions; strict
native Clippy; Release build; owned window close and isolated data hash comparison.
No real credentials, actual update install or user running instance is touched.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Final verification on 2026-11-15 PASS: scripts/ci-local.ps1 (all mandatory
checks, including serial ignored token tests), app-inclusive native library suites
(44 app + 76 local + 109 Session + 176 SSH + 365 terminal + 522 view = 1,292
passed, 7 existing ignored), strict native all-target Clippy, optimized Release
and git diff --check. Evidence: .tmp/us0259-final-ci.log,
.tmp/us0259-final-native-tests.log, .tmp/us0259-final-native-clippy.log,
.tmp/us0259-final-release-build.log. Optional cargo-deny was not invoked.

Release: target/release/oneterm-native-desktop.exe, 46,438,400 bytes, SHA256
217688E1BABC848CCEC08A06B5549B25F99022D0F1FB25C840ACC151799A79FE.
Owned actual Elevated-token startup/WM_CLOSE smoke PASS: PID 21820, HWND
2233482, exit 0; ownership rechecked before close. Evidence:
.tmp/us0259-release-smoke-20261115-130541.json and stderr/stdout companions;
.tmp/us0259-release-manifest.json records hashes and data changes. Wrapper
initially raised a stale LASTEXITCODE false failure after successful child exit;
the saved smoke result was independently re-read and verified, no relaunch needed.

Before smoke all four original data hashes matched their baseline. After smoke
known_hosts and the original lock remain byte-identical; docks.json and docks.bak
were written by the ordinary layout owner (restore/write now permitted), both
remain valid v2 documents with terminal center, SSH dock and SFTP table settings.
Missing ui_config.json, terminal.json and update_config.json plus locks were
created through ordinary first-run paths. No prior settings/vault/Session file
existed in the recorded isolated baseline; no user data was restored or deleted.
Do not interpret normal layout serialization/backup rotation as byte preservation.
The predecessor executable remains hash-verified at the path in
.tmp/us0258-before-release.json. Existing GPUI ERROR window not found reproduced
despite exit 0 and remains explicitly deferred. No production engine cutover,
publication, commit or push occurred.

The remaining paragraphs preserve intermediate failure/rework history; final
results above supersede their pending statements. Harness database status/proof
blocks are not updated manually because the Harness tool remains unavailable.
Implemented the shared Unknown-only predicate and known-token action parity.
Direct identity audit found production identity checks in app/lib.rs and
crash_report.rs, retained separately from permission guards. Both native entries
initialize the actual token; NativeTest still suppresses production startup effects.
Added real administrator Session snapshot round-trip and dock write/read proof,
saved-layout restore proof, update preference enqueue proof, and Unknown denial.
Legacy ignored test names are retained to avoid unrelated census/history churn;
their assertions now explicitly cover both confirmed administrator and Unknown.

Focused default tests passed (.tmp/us0259-focused.log). The first isolated run
identified an incorrectly restricted positive dock fixture; corrected to actual
Elevated. Final complete verification with strengthened persistence proofs runs
to .tmp/us0259-final-{ci,native-tests,native-clippy,release-build}.log; results and
owned-window smoke are pending. Earlier baseline logs do not prove later edits.
Final gate attempt caught E0425 in the new Session snapshot assertion: fixture
helpers belonged to a sibling test module. Kept fixture ownership there and
exposed a single pub(super) round-trip assertion; rerunning the entire pipeline,
not only the previously failed command. No runtime permission workaround added.
Harness discovery still exposes no Harness tool; no database state is claimed.

Windows integrity/ACL differences remain OS constraints. Existing last-completed
snapshot writer semantics are not redesigned. Physical administrator UI, real
credential mutation, actual updater installation and external-server acceptance
are not automated here. Deferred GPUI window-not-found shutdown diagnostic is
not part of this change. Preserve data; never restore hashes over user changes.
