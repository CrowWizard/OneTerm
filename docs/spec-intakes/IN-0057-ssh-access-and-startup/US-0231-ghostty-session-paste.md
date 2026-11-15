# Work: Acknowledged native local session paste

ID: US-0231
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
- Spec Intake: IN-0057; depends on US-0229/0230 and US-0221.

## Outcome

Native local session paste uses current native bracketed-paste mode, existing
OneTerm paste filtering and size policy, and returns an explicit FIFO-admission
result. Cancellation after control admission requests independent close.

## Scope

- [x] In scope: native bridge live paste-mode query, async pump encoding/write,
  bounded local paste controls and acknowledgment, cancellation/budget cleanup.
- [x] Out of scope: production selector, full TerminalSession, IME composition,
  SSH paste adapter, clipboard read/write OSC parity and GUI acceptance.

## Acceptance

- [x] Paste reads unpublished current native mode; plain newline normalization
  and bracketed marker/ESC/ETX filtering match existing paste policy.
- [x] More than 1 MiB is rejected before native mutation/admission; local pending
  paste text has an aggregate 4 MiB bound in addition to 64 control slots.
- [x] Paste executes in control FIFO order and returns transport enqueue result,
  not physical receipt. Raw writes can overtake waiting controls as documented.
- [x] Cancelling after accepted control requests close; queued/in-flight paste
  reservations release once on success, error, close and cancellation. No replay.
- [x] Transport rejection after mode lookup is typed and fail-stop; invalid size
  is recoverable. Last-good frame stays intact and no capture is required.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`; agent code style,
  structure, dependencies, dependency rules and error policy.
- IN-0057 HLD/native LLD, DEC-0023, US-0229/0230 and `docs/terminal-backend.md`.
- Existing paste policy and TerminalSession paste; native owner PasteMode,
  bridge cancellation/delivery, async pump, local controls and session tests.

### Documentation Action

Update required: native LLD, backend migration note, probe README and US-0208
handoff. Existing paste policy and production session contracts do not change.

### Reconciliation

Updated the native LLD, backend migration note, probe README and US-0208 handoff.
Shipping paste policy/TerminalSession, production selectors, dependencies and the
external VT API are unchanged. New bridge query reuses the existing owner mode
operation; boolean replies have no delivery/capture side effect.

## Plan

- [x] Add live mode query and bounded policy-aware async pump paste.
- [x] Add acknowledged bounded local paste with cancellation close guard.
- [x] Prove filtering, unpublished modes, FIFO/error/budget/cancel and real ACK;
  run focused regression, native Clippy, complete gate and doc checks.

## Decisions

DEC-0023. Future construction does not enqueue. Cancellation after poll/admission
requests close, but cannot retract bytes already accepted by the original FIFO.
Success means FIFO enqueue only, not child receipt or frame publication. Empty
plain paste performs no empty transport write; bracketed empty paste keeps markers.

## Verification Plan

Native pump mode/filter/oversize/transport-error/cancellation fixtures; real local
session paste ACK and local full-budget/control-queue/cancellation cleanup proof.
Native local/terminal/SSH suites and all-target Clippy; ci-local.ps1, doc-path,
English, dependency and diff checks. Preserve known PowerShell/orphan failures.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11:

- Three new async-pump tests prove unpublished mode enable/disable, plain CRLF/LF
  conversion without bracketed filtering, bracketed ESC/ETX/embedded-marker
  removal, empty plain no-write and empty bracketed markers. Paste retains the
  exact last-good frame without capture/repaint. Oversize recovers; transport
  QueueFull is typed fail-stop with no retry. A blocked mode lookup can be
  cancelled without writing or replacing last-good. Unpolled paste has no effect.
- Two new local owner tests prove four 1-MiB queued reservations, fifth refusal,
  rejection without shutdown, cancellation independent of blocked reliable UI,
  disposal returning the entire text budget, and active success/transport failure
  returning typed acknowledgments with exact release. Saturating the original
  physical-input budget does not bypass it with a paste side writer.
- Extended full-control-queue proof checks paste QueueFull and Closed admission
  release reservations without falsely cancelling the session. UI-release proof
  orders four resizes, paste text and Enter; child reports SIZE:93:30:ACK:queued.
- New real NativeLocalSession test proves oversize rejection then successful
  paste("ping\n") acknowledgment and child PASTE_ACK:ping through ConPTY.
- Native local filter: 17 pass (12 owner, four session, manual ConPTY). Native
  SSH136/terminal319 pass. Native local/SSH all-target Clippy passes warning-free.
- Final doc-path, English text (1239 files), dependency graph and diff checks
  pass. Focused native local17 and native all-target Clippy pass again after the
  closed-admission assertion was added.
- Full native local suite: 50 pass/2 existing conhost 0x6d orphan failures/3
  ignored (session_orphan_tests.rs:252, immediate drop and first output).
- ci-local.ps1 passes fmt and both workspace Clippy variants, then stops at the
  unchanged core shell.rs:916 PowerShell Enter guard: empty verdict instead of
  INSTALLED:AcceptLine, 92 pass/1 fail. Later gate steps are not reached. Output
  retained locally in `.tmp/us0231-ci-local.log`. No assertions were weakened.

All functional criteria above have focused proof; complete acceptance remains
gate-blocked. No production selector/full-session, SSH paste, GUI/Unix, clipboard
OSC or IME-composition parity claim. An admitted future cancelled after bytes
enqueue closes but cannot retract those bytes; transport success is not physical
receipt. Active native work has no preemption/deadline guarantee. Harness tool
discovery returned no available tool; Markdown work record retained.

## Handoff

Native full-session/content/render/IME/selection/search and remaining OSC/security
parity still precede simultaneous local/SSH factory cutover and GUI acceptance.
