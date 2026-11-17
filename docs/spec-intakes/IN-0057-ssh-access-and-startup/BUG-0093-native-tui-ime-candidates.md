# Work: Native TUI IME candidates

ID: BUG-0093
Intake: IN-0057
Created: 2026-11-16

## Status

<!-- HARNESS:STATUS:BEGIN -->
- [x] Planned
- [x] In progress
- [ ] Implemented
- [ ] Changed
- [x] Reopened (acceptance rework)
- [ ] Retired
<!-- HARNESS:STATUS:END -->

## Classification

- Change type: bug
- Risk lane: normal; private native UI routing only.
- Spec Intake: IN-0057.

## Outcome

Native TUI applications such as Codebuff can open platform IME candidates and
receive committed Unicode once, including while using the alternate screen.

## Scope

- [x] In scope: native printable-key ownership, composition acceptance, cursor
  anchoring and focused regression coverage.
- [x] Out of scope: legacy input, mouse reporting, selection policy, native
  encoding protocols, dependencies, persistence and inline preedit rendering.

## Acceptance

- [ ] Primary and alternate screens both accept IME preedit and commit.
- [ ] Printable keys yield to platform text input without duplicate native keys.
- [ ] Candidate navigation stays with composition; functional keys and control
  chords outside composition retain native routing.
- [ ] Candidate placement uses the published cursor on either screen.
- [ ] Returning from a TUI preserves ordinary shell input.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md` and `docs/HARNESS.md`: native staging and proof boundaries.
- `docs/spec-intakes/IN-0057-ssh-access-and-startup/low-level-design/ghostty-engine.md`: native key and candidate ownership.
- `docs/spec-intakes/IN-0057-ssh-access-and-startup/US-0236-ghostty-session-ime.md`: bounded preedit and clear-before-write commit.
- `docs/spec-intakes/IN-0057-ssh-access-and-startup/low-level-design/native-workspace.md`: workspace surface integration.

### Documentation Action

Update the native engine LLD: alternate-screen presence must not disable IME
or route printable text directly around the platform text service. Existing
session bounds, transport and functional-key contracts remain unchanged.

### Reconciliation

Updated the native engine LLD to give printable text and composition the same
ownership on both screen buffers. No public traits or transport rules changed.

## Context

The reported symptom is English letters instead of an IME candidate window
after starting Codebuff. The surface explicitly rejects composition and cursor
geometry on the alternate screen and directly encodes its printable keys.
No Codebuff-specific process detection or exception is necessary.

## Plan

- [x] Remove alternate-screen IME exclusion and align printable-key ownership.
- [x] Cover preedit, candidate geometry, commit, shortcuts and screen transitions.
- [ ] Run static checks, then commit and push under the existing user request.

## Decisions

IME ownership follows event type and active composition, not screen buffer.

## Verification Plan

Add focused native surface regression coverage for both screen buffers and
transition back to primary. Run formatting, diff, documentation-path and English
checks only. User explicitly requested no local compilation or tests.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [x] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

### Acceptance rework: 2026-11-17

Native workflow run 43899389224 failed in the native workspace test step. Its
authenticated failure log is unavailable, so the exact cloud failure list is
not claimed. Source review found a definite contradictory regression:
`native_surface_alt_screen_owns_text_and_absent_publication_has_no_modes` still
requires alternate-screen IME rejection and raw printable-key delivery, while
this packet deliberately enables alternate-screen IME and platform-owned text.

Reopen this packet for acceptance rework, not a new shipped-behavior bug. The
Harness tool is not exposed in this client catalog; status is recorded here.
The reviewed native engine LLD already specifies the intended behavior and
needs no contract change. Update the obsolete regression to assert alternate
screen preedit and exact single commit, retain absent-publication rejection,
and use a functional key to retain idempotent held-key cleanup assertions.
Do not change production routing or disable tests. Run static checks only,
then commit and push; cloud execution and manual IME acceptance remain open.

The obsolete regression now asserts alternate-screen composition acceptance,
preedit clearing on commit, exactly one ASCII commit and no printable native
key. Arrow Up retains the native press and idempotent cleanup-release proof.
Absent-publication assertions additionally reject preedit, text commits and
selection-range admission. No production source or workflow was changed.

Rework static checks passed: `cargo fmt --all -- --check`, `git diff --check`,
`python scripts/check-doc-paths.py` (220 paths) and
`python scripts/check-english.py` (1331 files). Local build, Clippy, tests and
the full CI gate remain unexecuted per owner instruction. This fixes a definite
source-level test contradiction, not a claim that all cloud failures are resolved.

Native Windows IME and Codebuff acceptance require the cloud-built application
and manual testing. No runtime success is claimed from source inspection.

Static checks passed: `cargo fmt --all -- --check`, `git diff --check`,
`python scripts/check-doc-paths.py` (220 paths) and
`python scripts/check-english.py` (1329 files). Local compilation, tests and
the full CI script were not run per the user's instruction. The new regression
drives primary, alternate and restored primary screen states; it asserts
preedit acceptance, cursor bounds, candidate navigation isolation, exact Unicode
commit, ASCII non-duplication, cancellation and native functional/control chords.
These assertions have not been executed locally.
