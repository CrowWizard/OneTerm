# Work: Native session viewport publication

ID: US-0233
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
- Spec Intake: IN-0057; native LLD; depends on US-0213/0221/0225/0229/0230.

## Outcome

Explicit native local/SSH sessions can scroll history and publish an owned frame
and coalesced repaint without needing subsequent process or network output.

## Scope

- [x] In scope: guarded pump scroll-and-capture, synchronous bounded scroll
  admission in the existing local/SSH control FIFOs, local session forwarding,
  publication/repaint ordering, independent close and retained failure state.
- [x] Out of scope: mouse-wheel policy/platform mapping, automatic bottom-on-key,
  selection/search navigation, full TerminalSession/render adapter, production
  selectors, GUI/Unix acceptance and configurable history capacity.

## Acceptance

- [x] Top, bottom, native signed delta and absolute row use real native history;
  frame metadata and rows agree after each control, even without further output.
- [x] Prior reliable batch events deliver before scroll/capture and repaint;
  model-only mutation is not confused with a published session operation.
- [x] Local/SSH scroll shares the existing 64-slot FIFO. Success means admission,
  not capture completion. QueueFull/Closed reject without implicit retry/close.
- [x] Scroll publishes a new owned frame with independent reader damage and a
  coalesced Output hint using the existing explicit consumer release contract.
- [x] Independent close works with full UI/control queues, drops waiting scrolls
  without replay; cancelled active pump publication fails closed with last-good.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`; agent code style,
  structure, dependency rules/dependencies and error policy.
- IN-0057 native LLD, DEC-0023, US-0229/0230/0231/0232 and backend migration note.
- Native Scroll/metadata, owner history tests, shared model mutation vs frame
  readers, async pump reply/event/capture rules; local/SSH control loops and tests.

### Documentation Action

Update required: native LLD, backend migration note, probe README and US-0208
handoff. No persisted schemas, production viewport contract or external VT change.

### Reconciliation

Updated native LLD, backend migration note, probe README and US-0208 handoff.
SharedModel::scroll rustdoc now explicitly distinguishes low-level mutation from
session publication. No production viewport policy, dependencies, persisted schema
or external VT contract changes. Standalone mutation-only pump scroll is retained
for existing staging users; session endpoints call scroll_viewport instead.

## Plan

- [x] Add guarded scroll-and-publication and explicit session control admission.
- [x] Prove native history/damage/repaint without new output, local/SSH FIFO and
  UI pressure/full queue/close/cancellation behavior.
- [x] Run native regressions/Clippy and complete gate; doc checks recorded below.

## Decisions

DEC-0023 and native LLD. SharedModel::scroll remains a low-level mutation only;
session callers must route scroll through the backend-owned pump to preserve
reliable delivery and publication. Do not allow UI capture to bypass the pump.

## Verification Plan

Shared native pump fixtures for history/events/damage/cancel and repaint release;
real local session and authenticated SSH loopback history without more output;
full-control-queue/blocked UI and independent close tests. Native terminal/SSH/
local suites, native all-target Clippy, ci-local.ps1 and doc/English/graph/diff
checks. Keep existing gate failures explicit; do not weaken assertions.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11:

- Six new tests pass: three async-pump fixtures, two authenticated SSH loopback
  fixtures and one real configured ConPTY session fixture.
- Pump tests prove top/bottom, signed delta and absolute row history with no new
  feed, consistent rows/scrollbar metadata, two independent damage readers,
  coalesced Output hints and explicit release. No transport writes occur.
- Full reliable event queue prevents scroll/capture/publication; release delivers
  the pending title before new viewport Output. Cancellation while pending retains
  last-good and closes native/transport admission, with no implicit retry.
- Real local session generates 60 history rows, waits for input, then scrolls
  top/delta/absolute row/back/bottom and posts repaint without more child output.
  Authenticated SSH proves the same native history transitions without peer bytes.
  A second SSH test proves UI backpressure holds scroll controls, raw writes still
  progress and two controls execute in native order after release.
- Extended existing full/closed control tests prove scroll QueueFull/Closed without
  false shutdown. Local/SSH cancellation fixtures now include queued scroll; full
  UI/command pressure still closes and retains last-good without replay.
- An initial SSH proof consumed an older output-batch repaint after frame polling.
  Tests now use empty plain paste as a no-wire control barrier before releasing the
  hint: frame publication alone does not acknowledge batch completion. The shared
  repaint contract and product behavior are unchanged, not weakened to pass.
- Native SSH142/terminal322 pass; native local filter18 pass (12 owner, five
  session, manual ConPTY); native local/SSH all-target Clippy passes -D warnings.
- Final doc-path, English text (1241 files), dependency graph and diff checks
  pass. The two new SSH scroll tests also pass three consecutive repeat runs.
- Full native local: 51 pass/2 existing conhost 0x6d orphan failures/3 ignored,
  session_orphan_tests.rs:252 at immediate drop and first output.
- ci-local.ps1 passes fmt and both workspace Clippy variants, then fails unchanged
  core shell.rs:916 PowerShell Enter guard, empty verdict vs INSTALLED:AcceptLine,
  92 pass/1 fail. Later gate commands not reached. Log: `.tmp/us0233-ci-local.log`.

Functional criteria have focused proof; complete acceptance remains gate-blocked.
No full TerminalSession/content/render/IME, production GUI/Unix, configurable
history, platform wheel/auto-bottom policy or selection/search navigation claim.
Repaint is still a coalesced best-effort hint, not reliable paint acknowledgment.
No Harness tool is exposed; Markdown work record retained.

## Handoff

Full native session/content/render/IME/selection/search and remaining OSC/security
parity still precede simultaneous production local/SSH selection and GUI acceptance.
