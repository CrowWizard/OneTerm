# Work: Administrator Session creation

ID: US-0258
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
- Risk lane: high-risk (authorization and persistence)
- Spec Intake: IN-0057; explicit owner approval on 2026-11-15.

## Outcome

Superseded before final acceptance by the owner's broader US-0259 approval on
2026-11-15. The append-only implementation and its new test module were removed;
administrator Session creation now follows ordinary persistence alongside all
other application functionality. The original scope below is historical, not
the current authorization contract. No independent create-only release is claimed.

Confirmed administrators can create and durably save a new SSH Session from the
Session header plus, action, or blank-area menu, without enabling other mutations.

## Scope

- [x] In scope: consistent create entry points; background append transaction; truthful completion feedback.
- [x] Out of scope: edit/delete/duplicate/group/startup changes, vault writes, settings/layout writes, engine cutover, deferred GPUI shutdown repair.

## Acceptance

- [ ] Confirmed elevated creation appends and reloads with a unique current-disk id.
- [ ] Unknown elevation and unreadable startup stores refuse creation; invalid/unsupported current documents remain untouched.
- [ ] Existing rows, sealed envelopes, startup preference and unknown JSON fields survive append; concurrent appends serialize.
- [ ] Existing elevated snapshot writes and other mutation permissions remain denied.
- [ ] Success appears only after the elevated disk transaction succeeds; errors do not add phantom rows.
- [ ] Header plus, New Session action and blank-area menu share the same permission.
- [ ] Focused proof, native regression, strict Clippy and full local CI pass; rebuilt artifact and data preservation checked.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md` — project invariants.
- `docs/agents/persistence.md` — schema ownership, transactions and sealed backups.
- `docs/decisions/DEC-0019-elevated-shells-open-in-an-elevated-window.md` — M1/M4.
- `docs/spec-intakes/IN-0057-ssh-access-and-startup/high-level-design.md` — administrator workspace.
- `docs/spec-intakes/IN-0057-ssh-access-and-startup/low-level-design/security-and-startup.md` — retained restrictions.

### Documentation Action

Update required: amend authorization/persistence contracts and add the append design.
Reason: explicit owner-approved create-only exception to administrator read-only management.

### Reconciliation

Superseded by US-0259. Current persistence policy and administrator detail design
record the broader approved contract; no append-only runtime path remains.

## Context

The header bypassed the action and reached a silent dialog guard. Lifting it alone
would report success while the store refused persistence. Harness discovery found
no tool; follow this intake's manual-template practice without changing harness.db.
srcwalk is not installed; use the pinned GPUI reference directly.

## Plan

- [ ] Implement create-only background append and consistent entry checks.
- [ ] Add policy, preservation, concurrency, failure and reload proofs.
- [ ] Run gates and reconcile docs; rebuild/smoke isolated Release.

## Decisions

DEC-0019 amendment; low-level-design/administrator-session-create.md.

## Verification Plan

Session-ui default/focused tests, existing ignored elevation proof, native regression
and strict all-target Clippy; scripts/ci-local.ps1; git diff --check. Use temporary
directories for tests. Hash isolated data before/after owned Release startup/close.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Pending. Existing ordinary snapshots retain last-completed-writer semantics;
this exception never submits an administrator snapshot. Physical UI acceptance
and the deferred GPUI shutdown diagnostic remain separate.
