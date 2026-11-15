# Work: Transactional owned Ghostty paint publication

ID: US-0214
Intake: IN-0057
Created: 2026-11-11

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
- Risk lane: high-risk
- Spec Intake: IN-0057

## Outcome

A native paint consumer retains one immutable publication containing both the
original complete frame and its projected rows. Failed projection leaves its
last good publication and damage watermark intact, with an explicit error.

## Scope

- [x] In scope: transactional FrameReader consumption, owned projected frame
  metadata, cached consumer publication, skipped-frame and failure tests.
- [x] Out of scope: production backend selection, native anchors, complete input,
  OSC and graphics parity. No second terminal, invented RowId or default modes.

## Acceptance

- [x] Metadata and projected rows retain the same immutable native capture.
- [x] Projection failure does not acknowledge damage or replace the last frame.
- [x] Independent consumers and skipped captures retain correct damage.
- [x] Repeated polls reuse the same projection; lifecycle remains observable.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`.
- `docs/agents/code-style.md`, `structure.md`, `crate-dependency-rules.md`,
  `error-policy.md`, `dependencies.md`.
- IN-0057 `low-level-design/ghostty-engine.md`, US-0208/0212/0213.
- `content.rs`, `session.rs`, `ghostty/owner.rs`, `ghostty/project.rs`,
  `backend/ghostty_pump.rs`, and renderer `render/frame.rs`.

### Documentation Action

Update required: Ghostty LLD and US-0208 handoff. This narrows the owned-frame
seam without changing persisted data, the external VT API or production engine.

### Reconciliation

Updated Ghostty LLD, US-0208 handoff and probe README. No production engine,
persisted schema or external VT contract changed.

## Context

TerminalContent still exposes legacy ordered history contracts. Do not hide that
gap by coercing native identities into RowId or publishing empty frames on error.
This consumer is a blocking-backend preparation seam, not a GPUI callback.

## Plan

- [x] Add a fallible prepare-before-watermark-commit reader operation.
- [x] Retain source capture alongside projected rows in one immutable value.
- [x] Prove failures, reuse, skipped updates and lifecycle.

## Decisions

DEC-0023; dedicated native ownership and single-engine invariant remain unchanged.

## Verification Plan

Native terminal tests, native terminal/view all-target Clippy, standalone probe,
renderer frame tests, documentation checks and complete ci-local.ps1.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Native-feature terminal tests passed 248/248; terminal/view native all-target
Clippy passed with warnings denied. Tests verify exact Arc capture retention,
metadata/cell agreement, projection reuse, rejected selected-cell projection
preserving the previous publication, skipped captures and independent damage,
native reply-overflow status retaining the last paint frame, and preparation
outside the publication lock. The native-only worker transaction test is also
included in the standalone probe. Latest rerun (shell
sh_1c8d2c935001ywdV1WvR03BeDQ): renderer frame tests passed 9/9, C ABI/runtime
probe passed, standalone native tests passed 37/37. Complete ci-local.ps1 passed
fmt and both workspace Clippy variants; workspace tests failed at the unchanged
PowerShell Enter-guard assertion in `crates/core/src/config/shell.rs:916` (92
core tests passed, 1 failed; empty powershell.exe default verdict rather than
`INSTALLED:AcceptLine`). Later gate steps did not run. Doc-path, English text,
dependency graph and diff-whitespace checks passed after reconciliation.

This is owned-frame preparation, not production whole-session integration:
TerminalContent, local/SSH sessions, full input/history/search/selection and
security/graphics parity remain under US-0208. No Harness tool is exposed by
the current tool catalog; template used.

## Handoff

US-0208 owns the actual shared local/SSH model and production cutover.
