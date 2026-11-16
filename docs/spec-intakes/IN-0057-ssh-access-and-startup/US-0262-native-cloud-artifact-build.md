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
