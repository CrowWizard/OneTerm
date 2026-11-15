# Work: Recognize the legacy system console host close reason

ID: US-0247
Intake: IN-0031
Created: 2026-11-12

## Status

Focused outcome verified; full workspace gate blocked by the existing Fira Code
DirectWrite test. Harness tool discovery previously returned no tool; database
status synchronization is not claimed.

## Classification

- Change type: maintenance of test portability
- Risk lane: high-risk validation seam; production termination is unchanged
- Spec Intake: IN-0031

## Outcome

The orphan regression accepts the measured legacy system console host broken-pipe
close reason without weakening shell exit checks or the no-survivor guarantee.

## Scope

- In scope: test-only process-role identification, exit validation and focused tests.
- Enable the existing windows-sys SystemInformation API only on the Windows dev
  dependency for GetSystemDirectoryW; no new package or runtime dependency.
- Out of scope: production teardown, process termination scope, time bounds,
  native migration, bundled-host acceptance, execution policy.

## Acceptance

- [x] Accept 109 only for a watched handle whose full image path is the system conhost.exe.
- [x] Shell 109, non-system same-name images and unknown exit reasons remain rejected.
- [x] Retain five-second liveness, handle-based ownership and no-probe-termination checks.
- [x] Re-run real orphan regressions and bundled gate; retain actual failures.

## Documentation

### Owning Docs Reviewed

- docs/terminal-backend.md sections 6.2/6.3: owner teardown and no-survivor guarantee.
- docs/decisions/DEC-0016-terminate-a-shell-that-outlives-its-pseudo-console.md: bounded child-only escalation.
- docs/spec-intakes/IN-0031-startup-spawns-two-local-shells/BUG-0055-a-discarded-local-session-leaves-no.md: original orphan regression purpose.
- docs/spec-intakes/IN-0031-startup-spawns-two-local-shells/evidence/conpty-20348-investigation.md: 30-session measurement and upstream source explanation.
- docs/PROJECT.md and docs/agents/error-policy.md: preserve safety and observability.
- docs/spec-intakes/IN-0031-startup-spawns-two-local-shells/low-level-design/orphan-exit-validation.md: strict identity and exit-reason validation seam.

### Documentation Action

No production contract change. Record the platform-specific test correction here;
the owning contract already guarantees liveness, not a universal host exit code.

### Reconciliation

Production owning contracts remain unchanged; the validation LLD records the
system-host-only exception and the safety checks that must remain strict.

## Context

Windows 20348 system conhost exits with ERROR_BROKEN_PIPE after pseudo-console
closure. The read-only investigation found no survivors in 30 sessions. The old
common whitelist fails on that exit reason. Historical Microsoft source explicitly
uses 109 during the signal-pipe shutdown path.

## Plan

- [x] Resolve the watched handle's full image path; classify only the system host.
- [x] Keep the existing common whitelist and add a host-only broken-pipe branch.
- [x] Test positive/negative classifications, run real regressions and bundled gate.

## Decisions

DEC-0016 unchanged. No new production authorization or termination behavior.

## Verification Plan

Run the orphan module serially, all local-shell tests and scripts/ci-local.ps1.
Pure decision tests must reject cmd.exe 109, a foreign conhost path, missing image
identity and unrelated host errors. Query identity through the already-open owned
handle to avoid PID reuse; identity query failure must fail the real fixture.

## Evidence and Gaps

- .tmp/us0247-orphan-tests.log: both original real-host regressions and the
  positive/negative decision test pass (3 passed, 1 existing ignored diagnostic).
- .tmp/us0247-local-tests.log: all 36 local-shell tests pass, 3 existing ignored;
  doc tests pass. No ignored tests were added or removed.
- .tmp/us0247-native-local-tests.log: with ghostty-native enabled, all 63
  local-shell tests pass, 3 existing ignored; doc tests pass. Both original
  orphan regressions also pass in that feature configuration.
- .tmp/us0247-ci.log: fmt and both workspace Clippy variants pass. Workspace
  tests pass core and local-shell (including both orphan regressions), then stop
  in terminal-view: 446 passed, 1 failed, 3 ignored. The existing
  render::glyphs::tests::ascii_fast_path_matches_directwrite test fails its Fira
  Code comparison at glyphs.rs:857, (1, 1) versus (0, 1). Later gate steps are
  unrun. No glyph code or font assertion was changed in this packet.
- Dependency policy, doc paths, English contributor-text and diff whitespace
  checks pass. No production implementation or termination scope changed.

## Handoff

The ConPTY test-portability outcome is verified. The full gate remains blocked
by the separate pre-existing font comparison; native desktop acceptance and
bundled-host teardown proof are not claimed here. Preserve all existing worktree
changes; no commit was made.
