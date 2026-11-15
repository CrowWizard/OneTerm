# Work: Production-shaped PowerShell guard probe

ID: US-0246
Intake: IN-0044
Created: 2026-11-12

## Status

Focused outcome verified; workspace gate blocked by existing ConPTY orphan tests.
Harness tool discovery returned no tool; database synchronization is not claimed.

## Classification

- Change type: maintenance
- Risk lane: normal; test-only invocation and diagnostics
- Spec Intake: IN-0044

## Outcome

The real-host Enter guard regression probe uses the production inline -Command
route and reports process failures rather than silently treating them as absence.

## Scope

- In scope: probe invocation, process diagnostics, obsolete fixture removal.
- Test child processes use host-default module discovery, not the invoking
  pwsh's inherited PSModulePath (which can select its incompatible script module
  inside Windows PowerShell). Production environment handling is unchanged.
- Out of scope: production init, binding behavior, execution policy, native cutover.

## Acceptance

- [x] Both installed hosts retain all four original binding assertions.
- [x] Only executable NotFound is treated as absent; other launch and exit errors fail visibly.
- [x] Probe uses -Command without policy bypass or temporary script files.

## Documentation

### Owning Docs Reviewed

- docs/terminal-backend.md section 6.1.2: inline startup and Enter chaining/decline.
- docs/spec-intakes/IN-0044-semantic-highlighting-phase-2/US-0136-shell-integrations-emit-the-full-osc-133-set.md: original four-binding regression purpose.
- docs/PROJECT.md and docs/HARNESS.md: production boundaries and verification.

### Documentation Action

No contract change: production already uses -Command, and the four required
verdicts remain unchanged. Record the test correction and evidence here.

### Reconciliation

The reviewed owning contracts remain correct. Only the test route/module
discovery and diagnostics changed; no production code or execution policy changed.

## Context

The bundled gate gets empty stdout from the -File fixture with effective Windows
PowerShell policy Restricted. Read-only inline probes passed all eight host/case
combinations. Capture the original -File failure before attributing it to policy.

## Plan

- [x] Capture original script-route diagnostics.
- [x] Replace fixture-file invocation and retain strict binding assertions.
- [x] Run focused/core tests and bundled gate; reconcile native launcher evidence.

## Decisions

No new architecture or production behavior decision.

## Verification Plan

Run the focused four-binding test, all core tests, and scripts/ci-local.ps1.
Run latest native app tests/build independently; report later unrelated gate
failures without weakening assertions or changing system policy.

## Evidence and Gaps

- .tmp/us0246-file-diagnostic.log: Windows PowerShell -File exits 1 because
  scripts are disabled; pwsh exits 0. No policy was changed.
- The first Rust inline run exposed inherited PSModulePath selecting
  D:\program\powershell\7\Modules\PSReadLine\PSReadLine.psm1 inside Windows
  PowerShell, which was blocked by policy. Direct pwsh child invocation had
  normalized module discovery and therefore hid this second fixture problem.
- The test now removes only its child PSModulePath, letting each host discover
  its default installed module. Import errors and stderr fail explicitly, in
  addition to launch/nonzero-exit failures. No absent-host or binding assertion
  was relaxed; only executable NotFound returns None.
- .tmp/us0246-core-tests.log: 93 core tests pass, including both installed hosts
  across all four original bindings; doc tests pass.
- .tmp/us0246-ci.log: fmt and both workspace Clippy variants pass. Workspace
  tests advance past core, then local-shell reports 33 passed, 2 failed, 3 ignored:
  both pre-existing orphan checks observe conhost.exe exit 0x6d (at immediate drop
  and first output). The script stops there; later gate steps are unrun.
- Latest native app proof is retained in US-0245: 32 tests and explicit binary
  build pass, with six launcher-specific tests including actual timeout injection.

## Handoff

Preserve all pre-existing worktree modifications; production remains legacy.
