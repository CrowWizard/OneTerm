# Work: Native session IME composition and commit

ID: US-0236
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
- Spec Intake: IN-0057, accepted native LLD and DEC-0023.

## Outcome

Explicit native local and SSH endpoints expose shared, engine-independent IME
composition storage and typed commit admission through their original raw-input
transport FIFO, without starting a legacy parser or pretending to be a complete
TerminalSession.

## Scope

- [x] In scope: owned preedit set/read/clear, bounded UTF-8 values, exact raw
  commit forwarding, typed rejection and real local/SSH integration proof.
- [x] Out of scope: GPUI composition ranges/candidate placement/notifications,
  render/session factory cutover, platform key policy, paste transformations,
  queue retries, and changing the shipping TerminalIme trait.

## Acceptance

- [x] Preedit is session-local owned text, replaceable/clearable without native
  mutation, transport bytes or frame publication; reads return owned snapshots.
- [x] Set and commit reject over 1 MiB UTF-8 before mutation or transport admission;
  rejected values retain existing preedit. The bound is independent of paste.
- [x] Valid commit clears preedit before one exact UTF-8 raw write, like existing
  PtySession behavior; no bracketed-paste wrapping or newline normalization.
- [x] QueueFull/Closed propagate as typed input errors, without retry, restoration
  or automatic close; success means original FIFO admission, not peer receipt.
- [x] Both real backends expose the same adapter semantics; no production selector,
  external VT API or native owner affinity changes.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md` and required agent
  code-style, structure, dependency, crate-rule and error-policy guides.
- DEC-0023 and native LLD: no shadow parser and explicit staged endpoints.
- `crates/terminal/src/session.rs`: existing TerminalIme clears before raw write.
- NativeLocalSession, NativeChannel and existing real transport/session tests.

### Documentation Action

Update required: native LLD, backend migration note, probe README and US-0208
handoff. No persisted schema, dependency or shipping IME contract change.

### Reconciliation

Updated native LLD, backend migration note, probe README and US-0208 handoff.
The Unicode follow-up proves native Windows input while retaining the .NET
Console.ReadLine discrepancy below; it is not full native IME/GUI acceptance.
No persistence or dependency changes.

## Context

IME preedit belongs to the session adapter, not the native parser. The explicit
typed API avoids the shipping void trait's log-only admission failures. One MiB
limits both stored preedit and a single commit; transport budgets still apply.

## Plan

- [x] Add shared composition state and focused limit/admission tests.
- [x] Forward through native local session and direct SSH endpoint, with real I/O proof.
- [x] Run native regressions, Clippy, full gate and documentation checks.
- [x] Acceptance follow-up: compare native ReadConsoleW with .NET ReadLine for
  committed combining/Japanese text; retain exact codepoint evidence and add a
  strict regression at the proven boundary. Do not infer a transport fix or
  weaken existing assertions when attribution is unavailable.

## Decisions

DEC-0023; native LLD. No new production IME policy or public trait change.

## Verification Plan

Unit tests cover Unicode owned snapshots, clear/replace, byte-exact limits,
oversize non-mutation, no preedit writes, exact commit and QueueFull/Closed
clear-before-write behavior. Real ConPTY and authenticated loopback SSH tests
prove committed text uses existing transport, not paste. Run native terminal/SSH/
local regressions and native all-target Clippy; ci-local.ps1 and docs/English/
  dependency/diff checks. Retain existing gate failures explicitly.

Follow-up verification: use a child-side Unicode ReadConsoleW probe alongside
Console.ReadLine with bounded reads and numeric ASCII evidence; both flow through
the unchanged native session commit path. Verify child input independently from
echo. Review pipe write and ConPTY host resolution, but do not change the external
VT API or production input policy without evidence. Update owning notes with the
result and rerun focused/full gates after any test changes.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11:

- Four shared composition unit tests prove owned Unicode snapshots and isolation,
  replacement/empty/clear, exact UTF-8 commit (including LF/CR/ESC/ETX), one write,
  unlocked callback, byte-exact 1-MiB limit, oversize rejection before mutation or
  write, and clear-before-write on QueueFull/Closed/Transport without retry.
- Real authenticated SSH fixture proves preedit/oversize do not admit peer bytes
  or change the published frame. Commit delivers exact Unicode/control bytes,
  no paste framing/normalization; closed admission returns the typed error and
  leaves preedit cleared. The existing full-command/UI-pressure fixture also
  proves IME QueueFull clears preedit but does not close the endpoint.
- Real ConPTY session fixture proves preedit/oversize do not alter captured text
  or submit a line. ASCII commit reaches Console.ReadLine with exact codepoints
  105,109,101, then closed admission returns typed Closed and clears preedit.
- Additional Unicode experiment submitted e + U+0301 and Japanese codepoints
  26085,26412,35486. The native viewport echoed the original text, but the child's
  Console.ReadLine reported 101,0,16,16,16 with its default input encoding; with
  explicit UTF8Encoding(false), it reported 101,0,112,112,112 instead. No raw PTY
  capture or console-input API trace attributes this difference. This is a local
  Unicode end-to-end compatibility gap; shared exact-byte/SSH proof is not proof
  of Unicode child input on ConPTY. The focused real local fixture asserts its
  proven ASCII boundary, not complete Unicode compatibility. No previously
  accepted test was weakened or ignored.
- Native SSH149/terminal335 tests pass; local native filter23 passes; native
  local/SSH all-target Clippy passes with -D warnings. No native FFI changed.
- Full native local:56 pass/2 existing conhost 0x6d orphan failures/3 ignored at
  session_orphan_tests.rs:252. Log: `.tmp/us0236-native-local.log`.
- ci-local.ps1 passes fmt and both workspace Clippy variants, then fails unchanged
  core shell.rs:916 PowerShell Enter guard (empty verdict vs INSTALLED:AcceptLine),
  92 pass/1 fail. Later steps are not reached. Log: `.tmp/us0236-ci-local.log`.
- Final doc paths (220 paths), English contributor text (1247 files), dependency
  graph (20 workspace packages), and `git diff --check` pass; these are not a
  replacement for the blocked full gate.

Unicode acceptance follow-up (same unaccepted packet, 2026-11-11):

- Reviewed the existing bounded Windows pipe writer and ConPTY host resolution.
  No transport, encoding, production input policy or external VT API changed.
- A child-side C# P/Invoke probe calls ReadConsoleW with a 256-UTF-16-unit buffer
  and reports integer code units as ASCII. The same live NativeLocalSession first
  commits e + U+0301 + Japanese; ReadConsoleW returns exactly
  101,769,26085,26412,35486. A following Console.ReadLine on the same input returns
  101,0,48,48,48 despite explicit UTF8Encoding(false). This confines the reproduced
  discrepancy to the .NET/console-reader path after native Unicode input is
  demonstrably available; it does not identify a specific runtime implementation
  defect or prove all console reading APIs/hosts correct.
- Added strict `native_local_session_ime_unicode_reaches_read_console_w`:
  combining/Japanese text must reach child ReadConsoleW exactly, and a second IME
  commit of U+1F600 + e-acute must return UTF-16 units 55357,56832,233. The second
  commit also clears existing preedit. Both use the original raw-input FIFO;
  there is no legacy parser, child echo substitution or paste conversion.
- The earlier ASCII/.NET fixture remains unchanged. The new regression asserts
  the correct native API boundary rather than accepting corrupted runtime output.
  Broad .NET Console.ReadLine compatibility remains open, but native Unicode
  commit delivery is no longer an unproven local boundary for these samples.
- New Unicode regression passes three consecutive additional runs. Latest
  SSH149/terminal335/local native24 and native all-target Clippy pass. Full gate
  again stops at the same PowerShell Enter guard after fmt/both Clippy variants;
  log `.tmp/us0236-unicode-ci-local.log`. Later steps are not reached.
- Final follow-up doc-path, English (1247 files), dependency graph and
  `git diff --check` pass. Full native-local suite was not repeated in this
  follow-up; the earlier 56/2/3 result above remains the last whole-suite result.

Functional adapter criteria have focused proof; full acceptance remains blocked.
No GUI candidate placement/ranges/painting, Unix or production engine-switch proof.
Harness tool is not exposed; retain this Markdown work record.

## Handoff

Complete native session/content/render and GPUI IME integration remain cutover
prerequisites. US-0235's ConPTY OSC110 and full-gate gaps remain open.
