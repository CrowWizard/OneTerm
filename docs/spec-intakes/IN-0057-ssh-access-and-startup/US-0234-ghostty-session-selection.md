# Work: Native session selection publication

ID: US-0234
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
- Spec Intake: IN-0057; native LLD; depends on US-0216/0230/0231/0232/0233.

## Outcome

Explicit native local/SSH sessions acknowledge selection set/clear and publish
owned selected-cell frames without requiring further process/network output.
Invalid selection endpoints are typed recoverable rejections, not session faults.

## Scope

- [x] In scope: typed selection update, guarded pump validation/publication,
  acknowledged local/SSH controls in their existing 64-slot FIFO, local session
  forwarding, cancellation close, highlight/copy consistency and cleanup proof.
- [x] Out of scope: mouse hit-testing/drag policy, semantic/line/select-all modes,
  search navigation, clipboard I/O, new anchor lifetime rules, production selector,
  full TerminalSession/content/render adapter, IME and GUI/Unix acceptance.

## Acceptance

- [x] Set/clear publishes selected-cell frames and coalesced repaint without new
  output. Ack means publication complete, not physical paint or clipboard write.
- [x] Prior reliable events precede selection mutation/capture; controls share
  FIFO order with scroll/resize/input, never bypassing the backend pump.
- [x] Foreign/released/expired/inactive selection anchors return typed rejection,
  retain the old selection/frame and leave the native owner and session usable.
- [x] Admission QueueFull/Closed is recoverable; an unpolled future does nothing.
  Cancelling admitted work requests independent close, without replay/rollback.
- [x] Fatal native/publication errors are acknowledged then fail-stop, retaining
  last-good; explicit close drains queued result senders under UI/queue pressure.
- [x] Existing bounded native selection_text returns consistent linear/rectangle
  content; no native pointer or legacy engine/RowId is introduced.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`; agent code style,
  structure, dependencies/dependency rules and error policy.
- IN-0057 native LLD, DEC-0023, US-0231/0232/0233 and backend migration note.
- Native anchors/selection and their recoverable error/lifetime tests; selected
  render cells; bridge recoverable dispatch; pump operations; local/SSH queues,
  acknowledgment/cancellation guards, lifecycle and real transport fixtures.

### Documentation Action

Update required: native LLD, backend migration note, probe README and US-0208
handoff. No persistence, external VT API, production selection or clipboard change.

### Reconciliation

Updated native LLD, backend migration note, probe README and US-0208 handoff.
SharedModel set/clear rustdoc explicitly distinguishes mutation from publication.
No persistence, dependency, external VT API or production clipboard/selection
contract changes. Native selection errors reuse the existing tracked-anchor rules.

## Plan

- [x] Add guarded recoverable selection update/capture and bounded acknowledged
  local/SSH control adapters with cancellation close.
- [x] Prove selected frames/copy, invalid recovery, event/FIFO order, pressure,
  cancellation and failure acknowledgment with real local/SSH fixtures.
- [x] Run native regression/Clippy and complete gate; doc checks recorded below.

## Decisions

DEC-0023 and native LLD. Reuse tracked anchor provenance and native inclusive
selection rules. Keep low-level SharedModel mutation/copy separate from session
publication; never let UI capture bypass the owner pump. Only typed native
Selection validation errors are recoverable after lookup; other failures fail-stop.

## Verification Plan

Native pump highlight/clear/rectangle/invalid/event/cancel fixtures; real configured
local session and authenticated SSH loopback without more output; queue/close
cleanup and recovery. Native terminal/SSH/local suites and all-target Clippy;
ci-local.ps1 and doc/English/graph/diff checks. Retain existing failures honestly.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11:

- Eleven new tests: five async-pump, four authenticated SSH loopback/owner and
  two real local session/owner fixtures. Extended the existing local mock test
  to inject fatal reliable delivery and prove the original error acknowledgment.
- Pump tests prove linear and reversed rectangle copy/selected cells; clear
  publishes even with released endpoint tokens. Repaint coalesces until consumer
  release; no process/peer bytes or system clipboard effect is introduced.
- Foreign/released and inactive/expired rejection each retains the previously
  published Arc and selection text, does not close transport and allows clear
  and normal shutdown. Owner copy-limit tests remain the 1-MiB boundary proof.
- Prior reliable title delivery precedes selection publication/Output. A full UI
  queue blocks mutation/capture; cancellation retains last-good and fails closed.
- Real configured ConPTY session proves rectangle highlight/owned copy and clear
  without further child output; released-token rejection leaves it alive. Real
  authenticated SSH proves linear copy/highlight/clear, no peer writes and healthy
  recovery. SSH UI-pressure fixture also proves resize precedes selection ack,
  raw transport progress while blocked and publication at the resized dimensions.
- Local/SSH 64-slot selection pressure tests prove QueueFull without shutdown,
  unpolled no-op, admitted future cancellation's independent close, all remaining
  waiting acknowledgments resolve Closed after queued senders are dropped, and
  last-good remains published. Closed admission rejects without retry.
- Local mock and SSH owner fixture close the reliable event receiver with a title
  pending. Active selection returns EventsClosed through its ack before the owner
  error, closes the pump/transport and retains last-good. No native allocation or
  projection-failure injection is added; those use the same fail-stop error path.
- Native `cargo test -p oneterm-ssh -p oneterm-terminal --features
  oneterm-ssh/ghostty-native --quiet`: SSH146 and terminal327 pass.
- `cargo test -p oneterm-local-shell --features ghostty-native native_local_
  --quiet`: 20 pass. Native local/SSH all-target Clippy passes with -D warnings.
- All four new SSH selection tests pass three additional consecutive runs.
  Doc-path, English text (1242 files), dependency graph and diff checks pass.
- Full native local: 53 pass/2 existing conhost 0x6d orphan failures/3 ignored at
  session_orphan_tests.rs:252, immediate drop and first output. Assertions unchanged.
- `pwsh scripts/ci-local.ps1` passes fmt and both workspace Clippy variants, then
  fails unchanged core shell.rs:916 PowerShell Enter guard (empty verdict versus
  INSTALLED:AcceptLine), 92 pass/1 fail. Later gate steps are not reached. Log:
  `.tmp/us0234-ci-local.log`.

Functional criteria have focused proof; complete acceptance remains gate-blocked.
No GUI/Unix or production selector/full TerminalSession claim. Ack confirms frame
publication only, never paint; repaint remains a coalesced best-effort hint.
No Harness tool is exposed; Markdown work record retained.

## Handoff

Production full-session/content/render/IME, selection interaction policy, remaining
OSC/security parity and simultaneous local/SSH cutover/GUI remain separate work.
