# Work: Default startup SSH

ID: US-0207
Intake: IN-0057
Created: 2026-11-10

## Status

<!-- HARNESS:STATUS:BEGIN -->
- [ ] Planned
- [x] In progress
- [ ] Implemented
- [ ] Changed
- [ ] Reopened (acceptance rework)
- [ ] Retired
<!-- HARNESS:STATUS:END -->

## Classification

- Change type: new capability
- Risk lane: high-risk
- Spec Intake: IN-0057

## Outcome

An ordinary application launch connects once to the optionally selected saved SSH session.

## Scope

- [x] In scope: persisted stable session id, context-menu selection, deferred startup and credential readiness.
- [x] Out of scope: automatic administrator connections, secret storage changes, background retries.

## Acceptance

- [ ] Exactly one startup session can be selected, changed, or disabled.
- [ ] Selection survives restart; removing its session disables it; duplication does not copy it.
- [x] Missing/dangling/malformed selections never connect to another session.
- [ ] Startup attempts once with ready credentials; otherwise asks through the existing form.
- [ ] Vault, host-key, cancellation and administrator read-only behavior remain intact.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/agents/persistence.md`, `docs/agents/error-policy.md`.
- `docs/ssh-client-connect.md`, `docs/ssh-authentication.md`, `docs/gui-layout.md`.
- This intake's HLD and security/startup detail design.

### Documentation Action

Update required: session document schema description and startup/session-menu usage.

### Reconciliation

Updated session persistence, SSH connection usage, GUI menu/startup behavior and
this intake's designs. Startup remains ordinary-only, once, with existing
credential/host-key handling and no automatic retries. Crash-recovery launches
skip it; repaired/migrated identities disable the preference.

## Context

Session-ui owns the id; app wires startup. Manual Harness records; no database writes.

## Plan

- [x] Extend session-document loading/saving and selection behavior with tests.
- [x] Add the menu and reuse the connect form for startup.
- [ ] Reconcile docs and run the full gate.

## Decisions

This intake's accepted HLD defines ordinary-only startup and existing security checks.

## Verification Plan

Persistence and selection round trips, dangling/deleted/duplicate id tests, readiness
tests, session menu regressions, and `pwsh scripts/ci-local.ps1`. Manual restart/SSH proof.

<!-- HARNESS:PROOF:BEGIN -->
- [x] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Supplemental verification passed the remaining ci-local portion: VT paranoid,
regex and transport-free tests, feature-matrix examples, dependency assertion,
headless example, warning-free rustdoc, API/nameability/platform checks,
package contents, rustdoc self-containment and trailing policy checks. This was
a partial gate invocation, not a successful full gate; the ConPTY/font failures
and real startup acceptance gaps remain. It verifies the existing oneterm-vt
engine, not the pending libghostty-vt replacement.

Implementation and regression tests cover selection/change/disable, document
round trip, malformed/dangling ids, identity repair, deletion and duplication,
credential readiness and menu placement. Manual attempts and cancellation both
consume any still-pending automatic attempt.

The first full gate stopped at an omitted startup field in the existing
`us0158_verify_tests.rs` document fixture; the fixture now supplies None.
`python scripts/check-doc-paths.py` and `python scripts/check-english.py` passed.
Both workspace Clippy passes (default and terminal-diagnostics) passed. The
workspace test run then stopped in the unchanged PowerShell Enter-binding probe;
the probe passes with process-scoped `PSExecutionPolicyPreference=Bypass` (no
user/machine policy change), and the full gate has been restarted with it.
The restarted full gate passed both Clippy modes and the PowerShell probe, then
stopped at two unchanged local-shell orphan tests because conhost.exe returned
`0x6d`, which is outside their expected exit-code set. No assertion has been
relaxed. The full gate is failing, not complete.

The focused run passed all 108 non-ignored session-ui tests, including the new
startup selection, persistence, identity repair, deletion/duplication and readiness
tests. Actions, core, settings-ui and sftp-ui suites also passed, but the aggregate
command stopped in an unchanged terminal-view Fira Code/DirectWrite comparison
(`ascii_fast_path_matches_directwrite`: expected (0, 1), got (1, 1)). Thus the
aggregate regression command is not green. The serial elevation suites passed
separately (workspace 3, session-ui 1, settings-ui 1); the ignored-test census
passed with all 19 ignored tests recorded. Dependency graph, doc paths, both Python policy-test modules,
English text, completion catalogs and theme contrast checks passed. Notices
validation passed on retry after its cross-platform metadata downloads completed.

Manual Windows acceptance remains unperformed:
- [ ] Select Connect on Startup, restart, and observe one connection attempt.
- [ ] Switch the selected row, disable it, and verify each survives restart.
- [ ] Test saved-password vault unlock, missing credentials and host-key approval.
- [ ] Cancel and verify no delayed attempt; fail authentication and verify no automatic retry.
- [ ] Verify administrator and crash-recovery launches do not auto-connect.

## Handoff

Finish the full gate and manual startup acceptance. Engine replacement remains
independently blocked in US-0208, not implemented by these changes.
