# Work: Administrator SSH access

ID: US-0206
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

- Change type: existing-contract change
- Risk lane: high-risk
- Spec Intake: IN-0057

## Outcome

Confirmed administrator windows can explicitly connect to SSH and use SFTP.

## Scope

- [x] In scope: action policy, SSH dock, connection menus, read-only session management.
- [x] Out of scope: config writes, updates, custom elevated commands, automatic elevated SSH.

## Acceptance

- [ ] SSH/SFTP actions and saved connections work in confirmed administrator windows.
- [ ] SSH dock and connection entries are present; redundant runas submenu stays absent.
- [ ] Config writes and session management remain unavailable.
- [x] Unknown elevation fails closed for network actions.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/agents/code-style.md`, `docs/agents/structure.md`.
- `docs/agents/crate-dependency-rules.md`, `docs/agents/error-policy.md`.
- `docs/agents/dependencies.md`, `docs/agents/persistence.md`.
- `docs/gui-layout.md`, `docs/ssh-client-connect.md`.
- `docs/sftp-browser-design.md` — explicit transfers and external-editor restrictions.
- `docs/decisions/DEC-0019-elevated-shells-open-in-an-elevated-window.md`.
- This intake's HLD and security/startup detail design.

### Documentation Action

Update required: amend M1, current GUI behavior, and persistence's fixed-layout rationale.

### Reconciliation

Updated DEC-0019 M1, GUI layout and menus, SSH connection usage, SFTP editor
restriction and persistence's fixed-layout rationale. M2-M7 remain unchanged.

## Context

Manual Harness records; no harness.db modifications. Existing persistence guards remain.

## Plan

- [x] Revise policy and layout/menus; add regression tests.
- [ ] Reconcile contracts and run the full gate.

## Decisions

Owner selection amends DEC-0019 M1 only, as detailed in this intake.

## Verification Plan

Action policy tests, session/workspace/menu regressions, ignored elevation tests,
and `pwsh scripts/ci-local.ps1`. Manual Windows SSH/SFTP smoke testing remains required.

<!-- HARNESS:PROOF:BEGIN -->
- [x] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Supplemental verification passed the remaining ci-local portion: VT paranoid,
regex and transport-free tests, feature-matrix examples, the six-leaf dependency
assertion, headless example, warning-free rustdoc, API/nameability/platform
checks, package contents, rustdoc self-containment and trailing policy checks.
This was a partial gate invocation; its success banner does not mean the full
gate passed. The ConPTY/font failures and manual acceptance gaps remain.

Implementation includes explicit network action classification, the administrator
SSH dock and connection menus, read-only session menus/Quick Connect, and a
pre-download SFTP Edit refusal backed by a core launcher guard. Unknown token
status still denies network actions. Regression tests cover policy, menu counts,
dock construction and editor launch refusal.

The first full gate stopped at a missing startup field in the existing saved-password
test fixture (`us0158_verify_tests.rs`); fixed under US-0207 and gate restarted.
`python scripts/check-doc-paths.py` and `python scripts/check-english.py` passed.
Both workspace Clippy passes (default and terminal-diagnostics) passed. The
workspace test run passed the action policy tests and restricted editor-launch
test, then stopped in the unchanged PowerShell Enter-binding probe because the
host's default execution policy refused its script. A focused rerun with only
process-scoped `PSExecutionPolicyPreference=Bypass` passed all four bindings in
both PowerShell hosts; no user/machine policy was changed. The full gate has
been restarted with that process-scoped environment.
The restarted full gate again passed both Clippy modes and the PowerShell probe,
then stopped at two unchanged local-shell orphan regressions:
`a_session_dropped_before_its_shell_starts_leaves_no_orphan` and
`a_session_dropped_after_its_shell_is_up_leaves_no_orphan`. Their watched conhost.exe
processes exited with `0x6d`, outside the tests' accepted exit-code set. No local-shell
or VT implementation/test has been changed and no assertion has been relaxed;
this is a full-gate blocker, not a successful complete verification.

The focused affected-crate run passed actions (5), core (93), session-ui (108),
settings-ui (60) and sftp-ui (78) tests. It stopped in terminal-view's unchanged
`render::glyphs::tests::ascii_fast_path_matches_directwrite` font probe: 446
passed, one failed, three ignored; its Fira Code comparison returned (1, 1)
instead of (0, 1). This is a second regression blocker, not proof that the new
menus failed. All three serial elevation suites passed separately: workspace
(3), session-ui (1), settings-ui (1). These prove the administrator SSH dock is
built, saved layout is never read, and dock/session/update-config writes remain
refused. The ignored-test census passed: 19 ignored tests, all recorded.
Dependency graph, doc paths, both Python policy-test modules, English text,
completion catalogs and theme contrast checks passed. Notices validation initially
timed out downloading cross-platform metadata; the background retry passed:
`THIRD-PARTY-NOTICES.md is up to date.`

Manual Windows acceptance remains unperformed:
- [ ] Open a marked administrator window, connect a saved SSH session and Quick Connect.
- [ ] Toggle the right dock and perform an explicitly chosen upload/download.
- [ ] Confirm only Open is offered on saved-session rows and no save checkbox is offered.
- [ ] Confirm SFTP Edit warns without downloading/launching an editor.
- [ ] Confirm config files remain unchanged and the ordinary window still supports editing.

## Handoff

Finish the full gate and manual administrator SSH/SFTP acceptance; do not claim
the live GUI/network behavior verified from unit tests alone.
