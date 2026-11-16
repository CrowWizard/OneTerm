# Work: Native cloud artifact build

ID: US-0262
Intake: IN-0057
Created: 2026-11-16

## Classification

- Change type: maintenance of the existing native acceptance build
- Risk lane: scoped CI packaging; read-only repository permissions, no release publishing
- Spec Intake: existing IN-0057 native-workspace contract

## Outcome

Build and validate the Windows x64 native desktop in GitHub Actions and provide
a downloadable ZIP plus SHA256 without compiling locally or replacing Release.

## Scope

- In scope: dedicated workflow, pinned native inputs, regression gates, explicit
  runtime-file packaging, provenance and operator documentation.
- Out of scope: production release/version/tag changes, application cutover,
  user data, global proxy settings, automatic publication or credentials.

## Acceptance

- [ ] Manual dispatch and relevant main pushes can start the build.
- [ ] Pinned Ghostty/Zig archives are hash-checked before execution.
- [ ] Existing full CI remains; native lint/tests gate artifact generation.
- [ ] ZIP contains only the executable, ConPTY pair, notices and provenance.
- [ ] ZIP checksum and artifact are uploaded with bounded retention.
- [ ] No further local compilation or replacement of the accepted executable.

## Documentation

### Owning Docs Reviewed

- AGENTS.md, docs/HARNESS.md, docs/templates/work.md and docs/PROJECT.md.
- docs/agents/code-style.md, structure.md, crate-dependency-rules.md,
  error-policy.md and dependencies.md: workspace boundaries and tool policy.
- scripts/ghostty-vt/README.md and build.ps1: pinned inputs and native build.
- .github/workflows/ci.yml and release.yml: existing gates and production
  publication boundary; .github/actions/setup-rust/action.yml: shared setup.
- crates/app/Cargo.toml and build.rs: native entry and ConPTY staging.
- IN-0057/low-level-design/native-workspace.md: isolated native acceptance entry.

### Documentation Action

Update required: scripts/ghostty-vt/README.md documents cloud invocation,
artifact contents, isolated data and the non-production boundary.

### Reconciliation

Updated scripts/ghostty-vt/README.md with dispatch, artifact retrieval,
runtime allowlist and isolated-data/non-production boundaries. Existing CI
and production release workflow remain unchanged.

## Context

Reuse the existing native-desktop feature and pinned build script, not the
version-bumping production Release workflow. Harness tool discovery in the
owning Tab task returned no tool; this packet does not claim database status.

## Plan

- [ ] Add dedicated native Windows workflow and package allowlist.
- [ ] Repair the shared Tab fixture under US-0261 for cloud verification.
- [ ] Run non-compiling static checks; leave compilation/testing to Actions.

## Decisions

No production architecture or persistence contract changes.

## Verification Plan

Locally parse PowerShell/YAML and run whitespace, English, doc-path and
dependency-policy checks only. In Actions run the bundled full gate plus native
workspace lint/tests, then build the exact native binary and verify package
entries before upload. Preserve existing multi-platform CI unchanged.

## Evidence and Gaps

### Cloud acceptance rework

Run 43815762923 at e9febd94 failed in native workspace tests after the full
workspace gate, pinned native build and native strict Clippy passed. The user
provided the failure tail: local-shell 75 passed, one failed, three ignored;
configuration_events_controls_and_lifecycle timed out at line 843 while
waiting for sanitized title and cwd publication. Public job metadata confirms
the failed step; anonymous full-log access returned 403 and per-step access
404, so no additional unseen diagnostics are claimed.

Rework plan: make the Unicode title fixture's Console.OutputEncoding explicitly
UTF-8, retain its bidi sanitization assertion and 25-second deadline, add
phase-specific title/cwd/state/output diagnostics, and retain native test logs
as an always-uploaded diagnostic artifact without allowing failed tests to
produce a runtime package. The existing custom PowerShell fixture bypasses
the normal PowerShell initialization, so its encoding is locale-dependent;
this is a candidate cause, not yet a proven cloud root cause.

Owning docs reviewed for rework: native-workspace.md, scripts/ghostty-vt/README.md,
ghostty_session_tests.rs, core/config/shell.rs, security_policy.rs and native
event-policy regressions. No runtime contract change: preserve shell config,
title sanitization, timeout and all gates; only test setup/diagnostics change.
Harness reopening tool remains unavailable; this is acceptance rework of the
same packet, not a new shipped-defect record. Verify with local static checks
only, then the cloud native workspace test step; no local compilation.

Applied the deterministic output-encoding candidate repair and phase-specific
diagnostics for metadata, configured startup and resize/input waits. The
25-second deadlines, expected sanitized title and all runtime assertions stay
unchanged. The workflow saves native workspace output with Tee-Object and
propagates Cargo's exit status, then uploads only that explicit log even on
failure. Hidden-file inclusion is scoped to the exact .tmp log, not caches
or user data. Runtime build/package/upload still require preceding success.

Rework static checks PASS: rustfmt --check for the changed test, PowerShell AST
parsing of four workflow blocks, English/doc-path/dependency-policy checks,
and git diff --check. No local compiler/test invocation occurred. Actual cloud
verification of the candidate repair is pending commit/push and a new run;
the first failure remains unresolved until that evidence is available.

The previous owned Zig/download worker was stopped (completion exit 255).
Implemented .github/workflows/native-build.yml with manual dispatch and main
push triggers, pinned actions and downloads, bundled full gate, native strict
lint/tests, release-profile native binary, runtime allowlist and 14-day upload.
No local compilation was performed after the cloud-only direction.

Passing non-compiling verification (2026-11-16):

- PyYAML parsing and structural assertions for triggers, read-only permissions,
  SHA-pinned external actions, mandatory gates and success-only artifact upload.
- PowerShell AST parsing of all three multiline workflow scripts.
- Executed the exact packaging block against isolated dummy fixture files:
  11 allowlisted files in the ZIP, runtime data/PDB excluded, checksum matched.
  Fixture output stays ignored under .tmp; it is not an application artifact.
- rustfmt --edition 2024 --check for the shared Tab fixture.
- check-doc-paths.py, check-english.py, verify-dependency-graph.py and
  git diff --check.
- Existing target/release/oneterm.exe remains SHA256
  45B15423B49BA5C0309CDCCDE81792DD0875A27784211BEFA22A620DE145178C.

Actual cloud compilation/tests and artifact availability remain unverified
until these uncommitted changes are committed and pushed to GitHub. No
version/tag/Release, native executable, successful test run or GUI acceptance
is implied by static validation or the packaging fixture.
