# Work: Native clipboard editing

ID: US-0242
Intake: IN-0057
Created: 2026-11-12

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
- Spec Intake: IN-0057; accepted native HLD/LLD and DEC-0023.

## Outcome

The independent native surface copies native selection text and pastes clipboard
text using object-safe session operations, native live bracketed-paste mode and
the existing filter/bounded original transport FIFO. UI work never blocks GPUI.

## Scope

- [x] In scope: clipboard copy/paste actions and platform chords, bounded async
  edit task, visible-paste session operation, real local/SSH adapters, error
  reporting, cancellation and focused proof.
- [x] Out of scope: creating selections by pointer, select-all, context menu,
  search bar/navigation, broadcast and production panel/factory selection.
  Those are subsequent independent interaction outcomes, not implied by copy.

## Acceptance

- [x] SurfaceSession exposes object-safe async visible paste with typed pump
  errors. Both endpoints use their existing 64-slot control FIFO and paste budget.
- [x] Visible paste validates before mutation, filters with existing policy,
  chooses native live mode, scrolls/captures/repaints before writing nonempty
  accepted bytes; empty/no-op and oversize input do not scroll or write.
- [x] Native copy formats the current live native selection with its 1 MiB bound;
  absent/empty selection preserves the clipboard. No clipboard read occurs for
  copying, no clipboard write occurs for pasting, and errors are observable.
- [ ] Action/chord events cannot also reach native keys/IME. One edit task is
  active per surface; busy admission rejects observably without unbounded tasks,
  payload retention or implicit replay. Held paste/copy does not auto-repeat.
- [ ] Native tests and Clippy run; full gate and docs checks are reconciled with
  existing Enter guard/Fira Code/conhost failures and desktop UAT gaps explicit.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, work template and required agent guides:
  ownership, security, object-safe public contracts and error reporting.
- Native HLD/LLD, DEC-0023 and US-0208/0233/0241: bounded live paste, no replay,
  independent native surface and unchanged production boundary.
- `docs/terminal-backend.md`, legacy edit/key handlers, native model selection
  formatter, pump, local/SSH paste reservations/acknowledgment and GPUI task tests.

### Documentation Action

Update native LLD before implementation; reconcile backend notes, probe README
and US-0208 handoff. No persistence/dependency/external oneterm-vt API change.

### Reconciliation

Updated native LLD, `docs/terminal-backend.md`, `scripts/ghostty-vt/README.md`
and US-0208 handoff. No production selector, persistence or dependency change.
The copy/paste slice does not complete the requested selection/search interaction.

## Context

Existing low-level paste remains capture-free. A separate visible-paste entry
uses the same control with an explicit reveal flag so scrolling and paste cannot
be separated by another endpoint control. Raw IME commit is not paste. Copy reads
native selection text without guessing row identities or driving a second engine.

## Plan

- [x] Add visible-paste pump/session seam and real endpoint trait implementations.
- [x] Add bounded native clipboard task and action/chord listeners.
- [x] Prove ordering/filtering/copy/errors, run gate and reconcile.

## Decisions

DEC-0023 and native LLD clipboard contract.

## Verification Plan

Pump tests: rejected/empty paste preserves history/publication, accepted visible
paste reveals bottom with live bracketed mode and filtered exact bytes, reliable
events precede mutation, transport failure/cancellation fail-stop. Real endpoint
trait-object paste proof on SSH/local. GPUI clipboard action/chord tests including
native selection copy, no selection, busy/repeat/errors and no key/IME duplicates.
Run native regressions/all-target Clippy, full ci-local, doc/English/dependency/diff
checks. Desktop clipboard/IME interaction remains manual UAT.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Native regression evidence (2026-11-12):

- `.tmp/us0242-backends.log`: SSH153 and terminal345 pass, including real SSH
  trait-object plain/bracketed exact bytes and reservation recovery.
- `.tmp/us0242-local.log`: focused native local25 pass, including real ConPTY
  trait-object visible paste and policy-rejection recovery.
- `.tmp/us0242-paste.log`: two new visible-paste tests pass. Empty/oversize input
  retains last publication/history; live bracketed mode filters exact bytes;
  publication is already at bottom when transport is called; reliable-event
  backpressure blocks mutation/write; cancellation and transport QueueFull close
  without retry. Existing capture-free low-level paste regression still passes.
- `.tmp/us0242-surface.log` plus final focused rerun: surface8 pass. Real native
  selection copy, absent selection preserving clipboard, paste chord, held-repeat
  suppression, busy rejection, endpoint failure, oversize and empty clipboard,
  and no duplicate native key/IME commits are covered. GPUI's test executor allows
  parking because the native dispatcher is a real OS thread.
- `.tmp/us0242-native-clippy.log`: native local/SSH/view all-target Clippy passes.
- `.tmp/us0242-ci-local.log`: fmt and both workspace Clippy variants pass; full
  gate stops at the pre-existing core PowerShell Enter guard (shell.rs:916,
  stdout empty instead of INSTALLED:AcceptLine). Later gate commands did not run.
- Final separate checks: doc paths220, English1261, dependency graph20 and
  `git diff --check` pass.

Harness discovery returned no tool again. Packet stays in progress: desktop
clipboard/action-dispatch UAT, cancellation during actual view teardown, complete
menu/broadcast/settings policy and acceptance remain unverified. Existing Fira Code
DirectWrite and conhost orphan failures were not rerun in this slice and are not
claimed resolved. Production engine selection remains unchanged. Bracketed paste
with nonempty text stripped to empty still sends the existing wrapper; only empty
original text is a no-op. Plain mode normalizes newlines, not ESC/ETX filtering.

## Handoff

Next: tracked-anchor drag selection, then search navigation. Complete panel/event/
security parity and GUI acceptance still precede production local/SSH cutover.
