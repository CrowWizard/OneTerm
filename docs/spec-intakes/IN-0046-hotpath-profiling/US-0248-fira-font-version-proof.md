# Work: Version-aware Fira Code shaping proof

ID: US-0248
Intake: IN-0046
Created: 2026-11-12

## Status

Focused outcome and bundled local gate verified. Harness tool discovery previously
returned no tool. No database synchronization claimed.

## Classification

- Change type: maintenance, test portability and diagnostics
- Risk lane: normal; production glyph algorithm unchanged
- Spec Intake: IN-0046

## Outcome

Real-font proof checks actual shaping behavior rather than assuming every installed
Fira Code version has the same required contextual substitution.

## Scope

- In scope: Fira Code test, contextual witness and exact layout parity, owning docs.
- Out of scope: production fast path, font installation, font selection, dependencies.

## Acceptance

- [x] Detect the backtick substitution using actual short DirectWrite layouts.
- [x] A witnessed contextual substitution still requires fast-path rejection.
- [x] Installed Fira variants compare fast/fallback output exactly against DirectWrite,
  forced and unforced, including contextual and longer ligature probes.
- [x] Existing unconditional synthetic contextual-rejection tests stay intact.

## Documentation

### Owning Docs Reviewed

- US-0146-ascii-shaping-bypass.md and evidence/US-0146-verify.md: contextual rejection contract and measured version-dependent behavior.
- docs/spec-intakes/IN-0018-rebuild-terminal-render-engine/low-level-design/render-pipeline.md: behavioral self-check, not a family denylist.
- docs/agents/dependencies.md: real Windows platform text system requirement.
- docs/PROJECT.md and docs/agents/code-style.md: no dependency patch or production cutover.

### Documentation Action

Update required: render-pipeline LLD must distinguish observed contextual Fira
versions from a blanket family rejection. Record correction in US-0146 without
rewriting its historical verification results.

### Reconciliation

Updated render-pipeline.md to describe version-dependent Fira behavior and the
behavioral check, not a family denylist. Added a dated correction to US-0146;
historical evidence remains unchanged. Production code is unchanged.

## Context

Focused original test reproduces at glyphs.rs:857: (1,1) versus (0,1), meaning
one fast-path layout and one cache miss, not differing glyph counts.
Installed user FiraCode-Regular.otf reports Version 1.207, has no GSUB ccmp
feature (fontTools inspection); SHA256:
84dc3c109aa4f9f5eebba292c1e9c073e65c6836dbbc494b1b516f1066a5d578.
The old assertion assumes the later ccmp backtick rule for every family version.
No font installation or global state will change.

## Plan

- [x] Measure the contextual witness and compare real layouts for all tested variants.
- [x] Reconcile version-dependent documentation; retain synthetic regression.
- [x] Run focused/full view tests and bundled gate, preserving actual failures.

## Decisions

No production algorithm or architecture decision.

## Verification Plan

Run the real DirectWrite test and synthetic contextual/fast-path tests, then full
terminal-view tests and scripts/ci-local.ps1. Do not declare workspace success
based on a font probe alone. DirectWrite equality remains field-for-field.

## Evidence and Gaps

- .tmp/us0248-glyph-tests.log: all 10 glyph tests pass, including unconditional
  synthetic contextual rejection and real DirectWrite parity. Regular, bold and
  italic Fira all report witness=None, fast=true on this installed version.
  Short witness contexts are A-backtick, Z-backtick and double backtick, compared
  with isolated-character glyph IDs. Exact layout comparisons cover contextual
  and longer probes, unforced and forced to 8/9.5 px.
- .tmp/us0248-view-tests.log: terminal-view 447 passed, 3 existing ignored;
  doc tests pass. No tests were skipped or assertion tolerances introduced.
- .tmp/us0248-ci.log: scripts/ci-local.ps1 exits 0, all checks passed, including
  both workspace Clippy variants, workspace tests, serial ignored elevation tests,
  ignored-test census, VT feature/API/docs/package checks and contributor checks.
  Theme contrast retains five pre-existing owner-accepted status-glyph exceptions.
  Optional --full cargo-deny was not requested or run.
- This host does not supply a contextual Fira version. Its real-font rejection
  branch is therefore not exercised here; synthetic rejection remains unconditional.

## Handoff

The local default-feature gate is green. This test-only correction does not prove
native feature parity or ordinary-token desktop acceptance and does not authorize
production engine cutover. Continue US-0245 desktop UAT and US-0208 parity work.
No commit or push was made; existing worktree changes are preserved.
