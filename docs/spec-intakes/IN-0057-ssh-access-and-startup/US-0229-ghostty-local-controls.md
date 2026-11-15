# Work: Coordinated native local controls

ID: US-0229
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
- Spec Intake: IN-0057; depends on US-0228 and US-0225/0226.

## Outcome

NativeLocal admits bounded key/mouse/resize controls, encodes using the native
state and coordinates native and physical local size before publishing a frame.

## Scope

- [x] In scope: 64-slot control FIFO, pre-admission validation, batch-boundary
  execution, coordinated PTY resize, error/close cleanup and real ConPTY proof.
- [x] Out of scope: platform key mapping, production selector, logging and GUI.

## Acceptance

- [x] Invalid input/size is rejected before admission without stopping the owner.
- [x] Key/mouse/resize share bounded FIFO order; encoded bytes use the original
  local FIFO. Raw writes can overtake waiting controls during UI/native waits.
- [x] Resize updates native then physical PTY then captures; a following control
  cannot pass physical resize. Physical failure is fail-stop, not rollback.
- [x] Full control queue cannot block independent close; queued controls drain
  without replay or extra output buffering and writes reclaim exact byte budget.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`; agent code style,
  structure, dependencies, dependency rules and runtime error policy.
- IN-0057 HLD/native LLD, DEC-0023, US-0228 and `docs/terminal-backend.md`.
- Native local owner/notifier, local session and PTY resize contract; native pump,
  key/mouse validators; native SSH control/resize admission and lifecycle tests.

### Documentation Action

Update required: native LLD, probe README and US-0208 handoff. No external VT API,
dependencies, persistence or shipping session behavior changes.

### Reconciliation

Updated native LLD, probe README, backend migration note and US-0208 handoff.
Shipping session selection and the original notifier semantics are unchanged.

## Plan

- [x] Add validated bounded controls and native/physical resize coordination.
- [x] Prove native encoding, real dimensions, ordering, bounds and failure/close.
- [x] Run focused regressions, Clippy and full gate; doc checks recorded below.

## Decisions

DEC-0023. Controls serialize at batch boundaries, not raw-write order. Physical
resize completion is platform API success, not a child acknowledgment. One resize
is active at a time; the original pending-size slot cannot coalesce two controls.

## Verification Plan

Real ConPTY size/ACK and native frame assertions; pre-admission invalid controls,
UI pressure/full queue release and close; mock physical failure and partial input
budget regression. Native local/terminal/SSH suites and all-target Clippy;
ci-local.ps1 and documentation/diff checks. Preserve existing failure evidence.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11:

- Ten native owner tests pass, four new plus full-queue/closed-admission assertions
  added to the earlier UI-pressure fixture. The manual ConPTY fixture also passes.
- Real resize 90x26 then 100x30 followed by native key text/Enter yields child
  SIZE:100:30:ACK:ping and a 100x30 native frame. Invalid zero/over-budget sizes,
  oversized key and nonfinite mouse are rejected while the owner stays alive.
- A blocked title batch accepts 64 resize controls, rejects another key with
  QueueFull, still writes raw input and closes without changing last-good or
  retaining budget. Releasing UI instead executes queued sizes then key, yielding
  child SIZE:93:30. Continuous-output fixture proves resize/close progress.
- Native pump + mock PTY proves exact raw, DECCKM arrow, SGR mouse and Enter FIFO;
  requested cell metrics reach the physical resize. A subsequent physical error
  returns typed I/O failure and retains last-good without capture/rollback.
- Native full local suite: 47 pass/2 existing conhost 0x6d orphan failures/3 ignored.
  Default local suite: 33 pass/2 same failures/3 ignored. Native local Clippy and
  native SSH136/terminal316 pass. Full gate passes fmt/both workspace Clippy then
  stops at unchanged core shell.rs:916 PowerShell Enter guard (92 pass/1 fail,
  empty verdict vs INSTALLED:AcceptLine); later gate steps not run.
- Final focused native local filter: 14 pass (ten owner, three session, manual).
  Doc-path, English text (1238 files), dependency graph and diff checks pass.

The owner now services controls between each bounded read/capture rather than
waiting for the entire stream to become idle. Windows empty-ring and immediate
redrain rules remain intact. This is not platform key mapping, production GUI,
Unix proof, physical delivery acknowledgment or a forced-drop orphan guarantee.
Full acceptance remains gate-blocked. Harness tool unavailable; Markdown used.

## Handoff

US-0230 owns native session lifecycle/configuration adaptation separately.
