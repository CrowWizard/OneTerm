# Work: libghostty-vt application engine

ID: US-0208
Intake: IN-0057
Created: 2026-11-10

US-0244 in-progress handoff: bounded independent-surface search is wired to real
local/SSH control FIFOs, with query generations, live grid revision validation,
native highlight conversion and owner-checked viewport reveal tokens. UI edits
do not cancel admitted endpoint futures. Idle surface release now drives native
clear; pending close waits for acknowledgment. Search-specific real endpoint
queue/cancellation tests and UI progress-budget/FeedRequired proofs pass; native
all-target Clippy passes. Full native view is 462 passed/1 failed/3 ignored, with
the existing Fira Code mismatch remaining (463 passed after input proof). Full gate still stops at the core
PowerShell Enter guard (92 passed/1 failed). Widget Unicode composition/commit,
window typing/Enter/Shift+Enter and busy destruction now have automated proof.
Unchanged InputState Change after Enter no longer supersedes navigation. OS IME
and desktop acceptance remain open. US-0245 now provides a separate default-off
local native desktop launcher; app tests and build pass, but this environment's
restricted token is correctly refused before GUI/shell creation. No real desktop
interaction acceptance or successful GUI teardown is implied by that refusal.
US-0245's actual Root test-platform fixture now proves idle and pending-debounce
window release with real native local sessions. It exposed a retained search-task
reference; the launcher now keeps explicit-quit processing alive for bounded
surface cleanup before quitting. Six launcher tests and all 32 native app tests
pass, as do native app Clippy/build. Real Windows GUI/admitted-operation close and
OS IME acceptance remain unverified. Actual five-second timeout fault injection
now proves persistent failure even after late reference release and owner cleanup.
US-0246 fixes the test-only core guard invocation/module discovery; core passes
93 tests. The bundled gate now passes the core guard but stops at both existing
local-shell orphan checks (conhost.exe exit 0x6d); later steps remain unrun.
Subsequent US-0247 investigation/correction verifies system-host identity through
its open handle before accepting its measured broken-pipe exit. Default local-shell
36 tests and native local-shell 63 tests now pass (3 existing ignored in each).
The latest .tmp/us0247-ci.log clears core/orphan checks, but workspace tests stop
at the existing Fira Code DirectWrite comparison (446 passed, 1 failed, 3 ignored
in terminal-view); subsequent gate commands remain unrun. Production is unchanged.
US-0248 subsequently corrects the font-version test assumption without changing
production shaping: glyph tests 10 pass, terminal-view 447 pass / 3 existing
ignored, and .tmp/us0248-ci.log completes scripts/ci-local.ps1 with all checks
passed. This clears the default-feature gate blocker, not native parity or the
ordinary-token desktop acceptance gaps. Optional --full cargo-deny was not run.
Do not treat this as accepted parity or switch
either production selector. See US-0244 for evidence and precise proof limits.

US-0243 handoff: independent-surface linear drag now uses the real local/SSH
acknowledged selection operation. Last-painted source/grid revisions reject stale
samples without expiring selection-only captures. One tracked start, temporary
end cleanup, coalesced pending motion and explicit finish bound the gesture.
Shift, outside-up, blur and idle Drag-state teardown have focused automated proof;
copy reads the resulting native text. Terminal346/SSH153/local25/surface9 and
native all-target Clippy pass. Full CI again stops at the core PowerShell Enter
guard (92 passed, 1 failed); later checks did not run. See US-0243 for first-run
SSH failure, cleanup/reflow proof limits and desktop UAT gaps. Next is bounded
native search/navigation; panel/security/parity and desktop acceptance still
precede simultaneous local/SSH production cutover. No selector was changed.

US-0242 handoff: independent-surface clipboard copy/visible paste is connected to
the live native selection formatter and both real endpoint SurfaceSession paste
operations. One asynchronous edit task, policy-only validation, original bounded
FIFO/budget, live bracketed filtering and reveal-before-write are proved without
selecting production sessions. Native terminal345/SSH153/local25/surface8 and
all-target native Clippy pass. See US-0242 for full-gate evidence and remaining
drag-selection/search/menu/broadcast/desktop acceptance gaps.

US-0241 handoff: bounded wheel routing is now connected
on the same real local/SSH control FIFOs. Live native owner modes select mouse
press bytes, mode-1007 alternate arrows, or primary history; Shift/history override
reporting. GPUI accumulates line fractions and uses device-grid geometry, not
published modes. Pump event-before-mutation and original-FIFO delivery are retained.
See US-0241 for proof and gate gaps. Local selection/search/paste UI, complete panel/
security and desktop UAT still precede simultaneous production cutover.

US-0240 pointer handoff: typed SurfaceSession mouse admission connects independent native
surface three-button reporting. Fractional grid-to-device metrics, Shift override,
owned releases outside the control/after resize and blur cleanup do not fabricate
legacy modes or coordinates. Real SSH SGR/live-mode proof and local typed admission
pass (SSH151/terminal339/local24). See US-0240 for that slice's headless/gate evidence.
local selection, clipboard/menu/settings policy and complete panel/security still
remain open. No production factory selection or desktop UAT is claimed.

US-0239 handoff: independent NativeTerminalSurface now renders through the native
element route and uses SurfaceSession implemented by real local/direct SSH. No
legacy TerminalSession or fake mode defaults are required. Atomic damage-free
queries, resize, functional keys/repeat/release/cleanup and typed IME callback are
implemented; layout keys do not fabricate physical codes. Primary text/candidate
keys stay on IME, alternate-screen text cannot double-commit. Candidate geometry
conservatively anchors at the native cursor; exact range layout/inline preedit and
desktop focus/blur acceptance remain open. Three surface tests, real endpoint
trait-object proof, terminal339/SSH150/local24 pass; full native view453/1/3 retains
Fira Code failure. See US-0239 for gate evidence. Caller still owns reliable events,
security and invalidation. Full production session/panel, application interactions,
remaining parity and GUI still precede simultaneous local/SSH selector cutover.

US-0238 handoff: the actual GPUI Frame/prepaint/PlanCache/cursor/selection path now
accepts a persistent native Content via an optional TerminalRender source hook.
Headless tests prove real plans/paint, no legacy snapshot calls, atomic background/
cursor colors, steady native cursor, exact selection spans, independent damage,
resize/absent/failed/closed viewport and source changes. The fixture is a fake
session carrying a real native model, not complete native backend TerminalSession
implementations or desktop GUI acceptance. Native legacy-mode encoding is refused;
native input/IME/session and unsupported history/graphics/parity remain open.
Native terminal338/SSH149/local24 and all-target Clippy pass. Full native view450/1/3,
default view446/1/3 retain Fira Code failure; local57/2/3 retains conhost failures.
Gate still stops at core Enter guard after fmt/both Clippy; US-0238 logs retain
evidence. No factory or production selector changed; first phase remains incomplete.

US-0237 handoff: native local/direct SSH endpoints now share object-safe
RenderSource and per-view Content with independent damage and immutable Viewport.
Rows, cursor, native modes, scrollbar, effective colors and selection spans come
from one prepared publication; queries never capture or acknowledge repaint.
Absent/closed/failed states retain their meaning without legacy RowId/mode guesses.
Three new bridge tests, real local/SSH colors/selection and Content-to-FrameRow
scanner proof pass. Native terminal338/SSH149/local focused24 and four-crate native
Clippy pass. Full view447/1/3 still fails Fira Code; local57/2/3 still fails conhost
0x6d orphan checks. Gate still stops at core Enter guard after fmt/both Clippy;
see US-0237 logs. Full GPUI frame/session adapter, interactions and cutover remain
open; typed content alone does not complete the first phase of the agreed plan.

US-0236 Unicode follow-up: strict child ReadConsoleW proof now receives exact
combining/Japanese and supplementary-plane text through native local commit. The
same-session .NET Console.ReadLine comparison returns wrong codepoints, narrowing
the remaining gap to runtime-reader compatibility rather than unproven native
Unicode delivery for these samples. No production encoding workaround. Regression
passes three additional runs; local native24/SSH149/terminal335/native Clippy pass.
Full gate still stops at unchanged core Enter guard after fmt/both Clippy; see
`.tmp/us0236-unicode-ci-local.log`. No selector or full GUI IME cutover.

US-0236 initial handoff: shared native session IME composition now backs explicit local
session/direct SSH APIs; bounded preedit and exact raw UTF-8 commit have typed
admission results, with clear-before-write matching shipping PtySession semantics.
Four unit tests and local ASCII/SSH Unicode transport fixtures pass, with SSH
QueueFull proof. Native SSH149/terminal335/local23 and Clippy pass. Full gate still
fails unchanged core Enter guard; full local56 pass/2 conhost 0x6d failures/3 ignored.
Local Unicode Console.ReadLine experiment gets wrong codepoints despite correct
echo even with explicit UTF-8 encoding; no attribution or compatibility claim.
See US-0236. Full content/render/GPUI IME, other parity and selector remain open.

US-0235 handoff: native local/SSH default colors now share the acknowledged 64-slot
control FIFO, flush reliable events then set native defaults/capture/repaint. Full
256-entry caller palette and optional cursor preserve OSC overrides; native reset
and color-query bytes have proof, including real SSH reply transport. Independent
readers see full color damage; queue/cancel/pending-result and fatal ack paths pass.
Native SSH148/terminal331/local22 and native Clippy pass; C/standalone probe69 pass.
Full native local55 pass/2 existing conhost 0x6d failures/3 ignored; full gate still
stops at unchanged PowerShell Enter guard after fmt/both Clippy variants. A real
local OSC 110 reset remained ineffective after marker/control barrier (native
reset passes); transport-path parity remains open, with no raw-wire attribution.
No production 16-to-256 palette mapping, color-scheme callback, full content/render/
IME/session adapter or selector. Those, selection interaction/search, remaining
OSC/security parity, jump/SFTP/tunnel lifetime and GUI still block cutover.

US-0234 handoff: native local/SSH acknowledged selection set/clear now publishes
selected-cell frames without more output, in the existing 64-slot control FIFO.
Typed anchor/selection rejection recovers without capture/close; fatal pump errors
acknowledge before teardown. Real local/SSH highlight/copy/clear/recovery and
pressure/cancellation/pending-result cleanup pass; native SSH146/terminal327/
local20 and native local/SSH Clippy pass. Full native local53 pass/2 existing
conhost 0x6d failures/3 ignored; gate still stops at PowerShell Enter guard after
fmt/both Clippy variants. Copy is bounded and clipboard-free; selection interaction
policy, full content/render/IME/session adapter, search navigation, remaining OSC/
security parity, jump/SFTP/tunnel lifetime and GUI still block production cutover.
Older notes about absent selection below describe their historical baseline.

US-0233 handoff: explicit native local/SSH scroll now shares the 64-slot control
FIFO and publishes a native viewport frame/repaint without more child/peer output.
Top/bottom/signed delta/native absolute row, reliable-before-capture order,
independent damage readers, repaint coalescing/release and cancellation/queue
pressure have focused proof. SharedModel scroll remains mutation-only; consumers
must use the session owner path to publish. Native SSH142/terminal322/local18 and
native local/SSH Clippy pass. Full local51 pass/2 existing conhost 0x6d failures/
3 ignored; full gate unchanged PowerShell Enter failure after fmt/both Clippy.
No production content/render/input selector or platform wheel/auto-bottom policy
is added; full session/IME/selection/search and remaining OSC/security parity,
jump/SFTP/tunnel lifetime and GUI acceptance still block cutover.

US-0232 handoff: direct NativeChannel now exposes acknowledged async paste with
the shared native live-mode/policy path. Paste shares 64 control slots and has a
separate 4-MiB text budget, cancellation close and exact RAII cleanup. Original
FIFO enqueue success is not physical peer receipt; transport rejection fails
closed without replay/capture. Four new fixtures prove authenticated loopback
bytes/order/pressure/cancel and isolated typed transport-failure acknowledgment.
Native SSH140/terminal319/local17 and native all-target local/SSH Clippy pass.
Full gate still stops at core's unchanged PowerShell Enter guard after fmt/both
workspace Clippy variants. Auth/trust, direct-only ownership, full TerminalSession,
render/IME/selection/search, remaining OSC/security parity and factory/GUI cutover
remain open. References below to absent SSH paste describe their earlier baseline.

US-0231 handoff: explicit native local session async paste now uses live native
mode rather than published metadata, shared 1-MiB filtering policy and original
transport FIFO acknowledgment without capture. Pending/active text is bounded at
4 MiB alongside the 64-slot controls; post-admission cancellation requests close.
Pre-admission rejection recovers; transport rejection fails closed without replay.
Native local17, SSH136/terminal319 and native all-target local/SSH Clippy pass.
Full native local50 pass/2 existing conhost 0x6d failures/3 ignored; full gate still
fails unchanged PowerShell Enter guard after fmt/both workspace Clippy variants.
Production full-session/content/render/IME/selection/search, SSH paste and remaining
OSC/security parity still block cutover. The older handoff below is its baseline,
not a claim that this explicit local paste seam is still absent.

US-0229/0230 handoff: NativeLocal now has validated bounded key/mouse/resize
controls, native then physical resize before capture and progress between bounded
reads under continuous output. NativeLocalSession reuses guarded shell Options,
passes caller policy, owns a single event receiver and implements TerminalLifecycle
with explicit native model/input/wait methods. No full TerminalSession or factory
selection yet; do not fill missing render/IME/selection semantics with defaults
or a second engine. Ten owner/three session/manual ConPTY tests pass; native local
Clippy and SSH136/terminal316 pass. Full native local suite: 47 pass/2 existing
conhost 0x6d failures/3 ignored; default local: 33 pass/2 same failures/3 ignored.
Full gate still stops at unchanged PowerShell Enter guard after fmt/both Clippy.
Logging, configurable scrollback, platform input mapping, native session/content/
render/IME/paste/selection/search integration, remaining OSC/security semantics,
jump/SFTP/tunnel ownership and real local/SSH GUI acceptance still block cutover.

US-0228 handoff (2026-11-11): explicit NativeLocal now autonomously owns a local
PTY, native feed/capture and original bounded FIFO on a dedicated PTY thread.
While native/UI waits it writes partial input, suspends output reads and responds
to independent close; there is no growing Rust output queue. Exited is ordered
before post-teardown Closed. Drop/cancelled wait never join on UI; wait observes
resource teardown, not an OS-thread join or deadline. Six owner fixtures and the
manual ConPTY fixture pass; native local Clippy and SSH136/terminal316 pass.
Full native local suite remains 40 pass/2 existing conhost 0x6d orphan failures/
3 ignored. Full gate still stops at the existing PowerShell Enter guard after
fmt/both workspace Clippy variants. No forced-drop orphan or Unix runtime proof.
Native local resize/physical controls, logging, production session/content/factory
adaptation, jump/SFTP/tunnel ownership and remaining semantic/GUI parity are still
required. This is real autonomous transport staging, not production migration.

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

Run the application terminal on libghostty-vt without sacrificing SSH or engine behavior.

## Scope

- [x] In scope: upstream API feasibility, application adapter, local and SSH transports.
- [x] Out of scope: dependency renaming, upstream forks, hidden PTYs, dual production state engines.

## Acceptance

- [x] A pinned upstream library builds and links on Windows with reproducible instructions.
- [ ] Both local PTY and SSH use libghostty-vt with incremental parsing and reply routing.
- [ ] Render, input, selection/search and OSC security regressions pass.
- [ ] Existing external oneterm-vt contracts remain valid or are separately agreed.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/agents/dependencies.md`, `docs/agents/crate-dependency-rules.md`.
- `docs/terminal-backend.md`, this intake's HLD and detail design.
- DEC-0014 and libghostty-vt's official C headers/build sources at
  fbb6dbe7ea5c1ce6df14ef82e00779a568cdb2a2.
- `low-level-design/ghostty-engine.md` — native ownership and adapter boundary.

### Documentation Action

Update required on implementation: engine ownership decision, backend design, crate
graph policy, dependencies, notices, and architecture. Do not amend current facts
to describe an unimplemented migration.

### Reconciliation

Intake/HLD/LLD and DEC-0023 record the accepted libghostty-vt direction. The
work packet was renamed to reflect that target. Shipping engine contracts remain
unchanged; application cutover is not implemented. PROJECT, terminal-backend,
dependency rules, structure and dependency guidance now distinguish the tested
native staging feature from the still-shipping engine. The probe README records
the exact build and Cargo boundary-validation commands.

## Context

Manual Harness records; no database writes. The owner explicitly selected
libghostty-vt after the initial librio-vt feasibility investigation.

## Plan

- [x] Build and smoke-test the pinned upstream C ABI with the MSVC target.
- [ ] Encapsulate native ownership, reply/event buffers and owned render snapshots.
- [ ] Implement and prove local/SSH and rendering behavior.

## Decisions

Use libghostty-vt directly, retaining transport ownership and GPUI rendering.
The migration direction is recorded in DEC-0023.
Do not introduce a fork, hidden local PTY, or second production state engine to
pretend the migration is complete. Keep oneterm-vt's external package unchanged.

## Verification Plan

Split-escape SSH parsing, replies to SSH (not PTY), OSC security policy, local ConPTY,
render/input/selection/search regressions and the full project gate.

2026-11-11 snapshot adaptation: add a native render buffer per consumer and copy
the viewport into pointer-free owned rows/cells, preserving grapheme UTF-8, wide
cell roles, style colors/flags, effective palette/default colors, row wrap and
opaque equality-only identities, and viewport cursor state. Bound copied cells
and text bytes, report oversized snapshots rather than truncate, and free native
iterators before the render state on every path. Prove snapshot lifetime across
write/resize/terminal destruction, independent consumers and Unicode/style
fixtures using only Ghostty state. Check every added FFI layout/key against the
installed C headers. This is a staged frame projection, not GPUI/history/search,
hyperlink, graphics or production-pump parity; keep those acceptance items open.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

The earlier librio-vt PTY-coupling finding is superseded by the owner's engine
choice. Official libghostty-vt headers expose terminal_new/vt_write/free,
WRITE_PTY and PROGRAM_STATUS callbacks, render state and search/selection APIs.
Source candidate: fbb6dbe7ea5c1ce6df14ef82e00779a568cdb2a2. Its build manifest
requires Zig >= 0.17.0; evaluate pinned Zig 0.17.1, not a floating nightly.
Native build, ABI safety, graphics and application cutover are not yet proven.

Added `scripts/ghostty-vt/probe.c` and its explicit-input PowerShell runner. The
probe opens no PTY and checks every two-chunk boundary plus byte-at-a-time VT
input, UTF-8 width through DSR, reply capture, ground/error state and resize.
The expanded probe also checks snapshot text, palette/reset styles and snapshot
independence after clearing the terminal; all of these checks passed on Windows.
It is not wired into production or the default gate before native linking works.
PowerShell AST parsing, `git diff --check`, the documentation-path check and
English contributor-text check passed. The native probe now compiles, links and
runs successfully against the pinned static library.

Initial archive downloads failed with EOF/timeouts. The pinned source tarball
has since downloaded and extracted successfully, with SHA-256
`2ff4f0aaf595bc07d36b4a08cf3e85a52581e3be792b295ae34a5db481197f99`.
Zig 0.17.1 downloaded, passed the pinned SHA-256 check, extracted, and reported
the expected version. The Windows MSVC native build and the C/Rust runtime probes
passed. The expanded C probe compiles with warnings as errors against the pinned
headers.
Added `scripts/ghostty-vt/build.ps1` to validate the archive and toolchain, keep
caches inside the project, build the upstream MSVC target without SIMD, and write
build provenance. Its PowerShell AST check and native execution passed, including
a subsequent cached rebuild.
The source reports library version 1.1.0-dev, not a pre-1.0 ABI. Source inspection
found no Sixel decoder in src/terminal, only a device-attributes enum: graphics
compatibility still requires a bounded adapter, not a second terminal engine.

Native proof includes a standalone Rust ownership probe compiled against the
same static library, before Cargo/application integration: a thread-confined
owner, stable boxed callback context, fixed-capacity reply capture without callback
allocation, explicit overflow failure and exactly-once cleanup. No unsafe Send or
Sync implementation is justified by the current evidence.

Windows build and runtime proof now passed: the C split-input/render probe and
all four Rust ownership tests passed against ghostty-vt-static.lib (SHA-256
`1d96fb0570b557cfc2e3ad6a419dc0c616ca4bcd8b8c7ccb0df53c6ce82eab51`).
The verified Rust owner now lives in the terminal adapter behind an explicit
staging-only ghostty-native feature. Its build script requires the project-local
GHOSTTY_VT_PREFIX, performs no downloads and does not change the default engine.
This allows Cargo tests/Clippy to exercise the real native boundary before the
shared model, snapshots and pump are cut over together. Do not run it beside the
shipping engine on production bytes.

`cargo test -p oneterm-terminal --features ghostty-native`: 187 passed, including
the four native-owner tests; native-feature all-target Clippy passed with
warnings denied. Most of those 187 tests still exercise oneterm-vt: this is not
evidence of Ghostty pump/render/security parity. The shared TerminalHandle,
TerminalContent, TerminalModel and TerminalPump have not been switched.
The wrapper reports native processing errors as well as reply overflow.

Upstream clipboard request contexts are stack-borrowed and must be answered
synchronously. The existing asynchronous SessionEvent::ClipboardRead path needs
an explicit safe bridge; retaining upstream request/reply pointers is invalid.
This and Sixel/anchor/render compatibility remain cutover blockers.

The full `pwsh -NoProfile -File scripts/ci-local.ps1` was rerun on 2026-11-10:
fmt and both workspace Clippy variants passed; workspace tests stopped at the
same two unmodified local-shell orphan tests (unexpected conhost exit `0x6d`).
No test or production behavior was weakened to bypass the failure. Previously
completed VT/API/package checks remain evidence for oneterm-vt only, not Ghostty.

After adding the real adapter owner and Cargo build script, the complete gate
was rerun without GHOSTTY_VT_PREFIX: fmt and both workspace Clippy variants passed;
workspace tests again stopped at those two local-shell orphan tests (`0x6d`,
33 passed, 2 failed, 3 ignored in that crate). Later script steps were not reached.
Log: shell sh_1c611856400162wB3idqJdPudT. Native-feature terminal tests/Clippy and
both headless probes were rerun after processing-error handling was added and
passed (187 terminal tests, four standalone owner tests). Dependency-graph,
documentation-path, English contributor-text and diff-whitespace checks passed.
No background downloads or native builds remain running.

### Owned viewport implementation (2026-11-11)

Added `ghostty/render.rs` and native fixtures in `ghostty/render_tests.rs`.
Frames own their UTF-8, styles, resolved colors, palette, cursor and opaque row
identities; native handles remain thread-confined, but owned Frame is Send/Sync.
The projection enforces 1,048,576 cells and 16 MiB text limits with explicit
errors and fallible reservations. The C probe now guards every added render key,
enum value and mirrored layout; Rust layout tests verify the matching offsets.

Testing found a native incremental-update hazard: after two persistent render
states captured a viewport, updating one consumed changes the other needed,
leaving old cells in the other consumer. The staged capture now rebuilds native
handles for a full copy each time, and the two-consumer regression passes. This
costs allocation/copying per frame and is not production damage optimization.
The ZWJ fixture requires mode 2027; no implicit terminal mode change was added.
The LLD and probe README record these limitations. No owning production-engine
contract is changed: terminal-backend and PROJECT still correctly describe the
default-off staging boundary, so no further factual change there is needed.

Full gate rerun: fmt and both workspace Clippy variants passed; workspace tests
stopped again at the same two local-shell orphan tests (unexpected conhost 0x6d;
33 passed, 2 failed, 3 ignored). Log: shell sh_1c63e076f001NMXpWAbaDnzsCo.
Later gate steps were not executed by that run. Production engine cutover,
thread ownership/pump adaptation, graphics, hyperlinks, history/anchors,
clipboard, input/selection/search and actual GUI/SSH acceptance remain open.

Final snapshot-focused verification: native-feature terminal tests 198 passed
(15 Ghostty tests, 183 existing adapter/legacy-engine tests); native-feature
all-target Clippy passed with warnings denied. The expanded C ABI/runtime probe
and standalone import of the actual Rust owner/projection passed all 15 tests.
The Unicode fixture exercises every two-chunk boundary, including a render
capture between chunks. Additional fixtures cover explicit default/cursor colors
and alternate-screen restoration. Dependency graph, doc paths, English text,
fmt, standalone rustfmt and diff-whitespace checks passed. The Harness tool was
not exposed in this session's tool catalog; durable records were updated in
the existing in-progress packet and LLD without marking acceptance complete.

## Handoff

US-0245 real desktop continuation: owner reports elevated-window output, search,
selection, copy/paste, wheel and Chinese IME pass, but supplied close logs reveal
a real five-second surface cleanup timeout. An HWND-specific reproduction confirms
the platform IME adapter retains the surface entity. Native-only weak forwarding
now permits real idle GUI close with exit 0 and prompt owner/consumer release;
GPUI-owned HWND destruction removes invalid-HWND errors on repeated closes.
Source-located `window not found` is the expected late async active-status update
after WM_ACTIVATE, logged by GPUI when close wins the race; it remains visible.
Retained-adapter regression/native view suite
465 pass (3 existing ignored), native app 33 pass, native Clippy/build pass.
See US-0245 and evidence/US-0245-real-close.md. New full gate passes all checks
in .tmp/us0245-weak-ime-ci.log and again after close-order changes in
.tmp/us0245-close-order-ci.log (optional --full not run). Logger source-location
instrumentation full gate passes in .tmp/us0245-log-origin-ci.log. Owner's rebuilt-binary
logs corroborate same-second surface/consumer/owner cleanup without timeout;
Chinese IME/search-close reacceptance, admitted-work
close, ordinary-token behavior and production parity remain open.

Latest additions: US-0215 adds state-owned native keyboard/mouse encoding and
existing-policy paste delivery through GhosttyPump. US-0216 adds bounded native
tracked history anchors, typed coordinate spaces, terminal/screen provenance,
recoverable invalid/stale token errors and serialized pump queries. Native
terminal tests pass 272, standalone native tests pass 56, C ABI/runtime probe,
native terminal/view all-target Clippy and 9 renderer frame tests pass. These
remain staging interfaces, not production input/model/local/SSH migration.
The latest complete gate passed fmt/both workspace Clippy variants and again
stopped at the unchanged PowerShell Enter-guard test (92 core passed, 1 failed);
later steps did not run. Log: sh_1c8f16038001JNAaxz20mJf1Cv.

The pinned native coordinate API omits some space bounds; US-0216 validates
native round trips, history-vs-active membership and viewport height explicitly.
Real anchor tests cover scroll, bidirectional reflow and history erasure without
fabricating RowId order. Selection/search integration is still pending.

Latest slices: US-0213 captures native modes/active screen/keyboard flags and
scrollbar atomically with viewport rows/cursor, adds serialized viewport commands
and metadata damage. US-0214 retains the exact source Arc with projected paint
rows and prepares consumer data before committing damage; rejected projection
retains the last good publication. Native-feature terminal tests pass 248 and
native terminal/view all-target Clippy passes. Final US-0214 renderer tests pass
9/9, C ABI/runtime probe passes and standalone native tests pass 37/37. Its
complete gate rerun passed fmt/both workspace Clippy variants and failed at the
unchanged PowerShell Enter-guard test (92 core tests passed, 1 failed); later
steps did not run. Doc-path, English, graph and diff-whitespace checks passed.

Neither slice switches TerminalContent/TerminalHandle/TerminalModel or the
production local/SSH pump. The next integration must replace their ordered
RowId-dependent history/selection contract using real native anchors, wire
backend-side projection/publication without blocking GPUI/tokio, and preserve
full input, OSC/security, clipboard and graphics behavior. Native SGR-pixel
mouse/synchronized-output flags are explicit state, not complete input support.
Do not coerce missing state into legacy defaults or publish an empty frame on
error. Production migration and real Windows GUI/SSH acceptance remain open.

US-0209 now supplies a tested dedicated native worker and shared Arc viewport
publication with independent reader damage behind the default-off feature.
Native handles remain !Send/!Sync; no production caller has been switched.
Native-feature terminal tests passed 209 tests (26 Ghostty), all-target native
Clippy and the C/standalone Rust probe passed. Worker failure reporting preserves
native errors and waits for panic status before returning a disconnection error.

The latest full gate passed fmt and both workspace Clippy variants but stopped
at the unchanged core PowerShell Enter-guard test (empty default-host verdict;
92 passed, 1 failed). A targeted rerun reproduced it; later steps did not run.
Earlier ConPTY/DirectWrite failures are not resolved by the new focused proof.

US-0210 extends that reply-first seam: a default-off GhosttyPump now privately owns
EngineOwner and routes replies through the existing PtyTransport trait before
explicit capture. Its 13 native transport fixtures pass; the native-feature
terminal suite now passes 222 tests and native-feature all-target Clippy passes.
The C/standalone native probe still passes 26 owner/render tests. Full gate again
passes fmt/both Clippy variants and stops at the same core PowerShell Enter-guard
test (92 passed, 1 failed); later gate checks remain unexecuted. No production
backend, OSC event routing or model/content rendering path is switched by this
seam. Replace the shared terminal adapter next, preserving the reply-first seam,
the earlier administrator/startup changes, and reporting the outstanding full-gate
ConPTY orphan and DirectWrite test failures independently of migration evidence.

US-0211 extends the staging seam with bounded native title/directory/bell events,
owned worker drain, existing router sanitization and reliable sink delivery before
capture/repaint. Native Cargo tests now pass 233 (31 native owner/render/event,
19 pump), native all-target Clippy passes, C ABI/runtime and standalone Rust pass
31 tests. The full gate still fails at the same core PowerShell Enter-guard test;
later checks did not run. Callback testing revealed that the pinned native parser
ignored a 2048-byte title; long-OSC parity must be reconciled before cutover.
Other OSC families, clipboard, graphics, shared model/content and actual local/SSH
session integration are not complete. This is not production migration acceptance.

US-0212 removes the GPUI row reader's direct SnapshotRow dependency: shared
RenderRow values borrow shipping snapshots without row copies and can lend real
Ghostty projected cells/runs/clusters. Native cache keys use exact immutable
content equality, not fabricated ordered RowId/SeqNo. No native history/image
anchor is returned. This only connects the row/cell/scanner seam, not the full
TerminalContent/model/TerminalSession contract or production local/SSH selector.
Native default colors currently project as RGB; default-vs-exact intent and
theme/contrast integration remain open alongside the existing migration blockers.

US-0217 now adds worker/pump native anchor-based set/clear/copy, recoverable
endpoint/copy-limit errors and 1 MiB plain UTF-8 output. Terminal-owned selection
survives caller anchor release; native screen switching clears destination
selection rather than restoring it. Immutable viewport selection spans share the
paint source Arc and do not change paint row keys for selection alone. Native
selection/history/reflow/bounds fixtures, exact spans, independent damage readers
and batch-only publication pass. Latest proof: 282 native-feature terminal tests,
63 standalone tests, C ABI/runtime probe, native terminal/view all-target Clippy,
and 9 renderer frame tests pass. Full gate again passes fmt/both workspace Clippy
and stops at the unchanged core PowerShell Enter guard (92 passed/1 failed);
later steps are not reached. See US-0217 for retained evidence. Selection/search
gestures, production overlays, async command/close, shared model/content and all
actual local/SSH integration remain open; production still uses oneterm-vt.

US-0218/0219/0220 add incremental native search, a bounded asynchronous owner
bridge with out-of-band close/native join, and a shared native model that reads
preprepared frames and sends interactions to the same owner. No native handles
move threads; cancelled reply delivery cannot silently lose nonempty transport
bytes/events. Model readers retain independent damage and last-good data across
close/native/projection failure. Search returns only response-scoped screen
coordinates, rejects case-sensitive/whole-word options and limits responses to
4096 matches. Its soft-wrap behavior differs from legacy matching, and internal
native search memory is not bounded by the response cap. Those are parity gaps,
not permission to silently weaken the shipping find bar.

Latest Windows proof: 295 native-feature terminal tests, 66 standalone native
tests, pinned C ABI/runtime probe, native terminal/view all-target Clippy and
9 renderer frame tests pass. Full gate again passes fmt/both workspace Clippy
then fails unchanged PowerShell Enter guard at core shell.rs:916 (92 passed,
1 failed); later checks are not reached. The new bridge/model are Cargo-only
staging, not an async GhosttyPump, clipboard bridge, shipping TerminalModel or
real TerminalSession integration. Reply/event delivery still belongs to the
backend caller before explicit capture. Next: search option/memory/highlight
parity, async reply-first transport pump, real shared model/session contract and
gesture/overlay integration, then remaining OSC/graphics/theme/input parity and
simultaneous local/SSH cutover with actual GUI/SSH acceptance.

US-0221 now supplies AsyncGhosttyPump on the owner bridge/shared model. It enqueues
replies before native event drain, awaits bounded reliable title/directory/bell
delivery before capture, and posts coalesced Output afterward. Closed reliable
queues fail-stop. Active-operation cancellation immediately closes transport/
admission and prevents later prepared frame publication; already committed events
or frames cannot be rolled back. Graceful close flushes without capture; explicit
shutdown signals before polling, discards unfinished events and awaits native
destruction off the executor. Native handles still never leave their owner.

The new pump is not selected by either real read loop. Production TerminalModel,
TerminalSession and Content/Handle remain on oneterm-vt. Next work is production
session/read-loop/gesture/overlay integration, search options/memory/highlights,
paste/clipboard and remaining OSC/graphics/theme/input parity before simultaneous
backend cutover and actual GUI/SSH acceptance. See US-0221 for current proof and
the unchanged full-gate blocker; earlier counts above are historical evidence.

US-0222 extends proof to real authenticated loopback SSH channels and the existing
SshTransport FIFO, with no legacy engine for the stream. The peer receives input
then exact DSR reply; split CSI output appears in native paint, coalesced resize
matches peer/native dimensions, and reliable-event backpressure gates capture.
Cancellation preserves last-good and closes admission; the fixture explicitly
closes the channel and checks peer receipt. Native SSH suite: 111 pass; native
terminal: 313 pass; SSH/terminal native all-target Clippy passes. This is not
shipping ssh_main_task/session/SFTP teardown or local PTY/GUI acceptance. Full
ci-local still stops at the unchanged PowerShell Enter guard (92 pass/1 fail).

US-0223 now proves real Windows ConPTY input/output/resize/child exit using the
original LocalTransport/ShellNotifier FIFO and AsyncGhosttyPump, with no legacy
state for that stream. Child ACK/native paint and child-reported 100x30/native
frame agree; child exits successfully and explicit shutdown closes admission.
Native local fixture and local/terminal all-target Clippy pass; 313 native
terminal tests pass. Local suite remains partial: native 34 pass/2 fail/3 ignored,
default 33 pass/2 fail/3 ignored, both failing the existing conhost 0x6d orphan
guards. Full ci-local still stops at the same core PowerShell Enter guard.
Shipping read loops/session/GUI and forced-close cleanup remain unproven; real
transport fixture success is not production application-engine replacement.

US-0224 now adds an opt-in NativeChannel owner task for an already authenticated
direct SSH channel. It autonomously reads/feeds/captures and services the original
reply FIFO while reliable UI delivery is pending. Independent explicit close,
endpoint Drop and cancelled wait wake teardown under full UI/command queues;
last-good is retained and queued/in-flight write budgets release without replay.
Physical close/native destruction precede direct handle release. Eight real
owner fixtures pass; native SSH suite 119 pass, native terminal 313 pass and
native SSH/terminal all-target Clippy passes. Full gate still stops at unchanged
PowerShell Enter guard after fmt/both workspace Clippy. This is not shipping
session/factory selection: resize/physical input, lifecycle UI events, hop/SFTP/
tunnel lifetimes, local production loop and remaining parity are still open.

US-0225 adds validated bounded physical key/mouse admission to NativeChannel.
Its one owner encodes at batch boundaries into the original SSH FIFO using
remote-selected Ghostty modes. Four real fixtures prove mode-derived bytes,
invalid rejection, full-queue close without replay and UI-release resumption.
Native SSH 123 and terminal 313 tests plus native all-target Clippy pass. Raw
writes may overtake queued physical input; this is not platform key mapping or
production integration. Resize/lifecycle/session/local-loop work remains open.
Full gate still stops at the existing PowerShell Enter guard after fmt/both
workspace Clippy variants pass.

US-0226 adds coordinated NativeChannel resize in the same bounded physical
control FIFO: native resize, original transport pending-size/FIFO barrier,
window-change submission, then capture. Six new fixtures prove real peer/frame
sizes and input order, invalid rejection, pressure/release, full-queue close,
physical failure retaining last-good and marker-suppression recovery. Native SSH
129 and terminal 313 tests and native all-target Clippy pass. Physical submission
is not acknowledgment; shipping factories still use the legacy engine. Full gate
still fails the unchanged PowerShell Enter guard after fmt/both workspace Clippy.
Next work remains lifecycle/session/content integration, local owner loop,
hop/SFTP/tunnel ownership, platform mapping and remaining semantic/GUI parity.

US-0227 adds native ordered Exited/Closed delivery and cached exit status. Late
data after exit status still paints. Closed follows physical/native teardown and
waits on bounded UI capacity; explicit close/Drop/cancelled wait can interrupt
delivery, recorded as LifecycleDelivery::Cancelled rather than false success.
Immediately ready Closed can still be sent on explicit close. Original errors
retain precedence over final event loss. Seven new real owner and three native
pump fixtures pass; native SSH 136/terminal 316 plus native all-target Clippy pass.
Full gate still stops at the unchanged PowerShell Enter guard after fmt/both
workspace Clippy variants. This remains a direct-channel staging endpoint, not
shipping session/content/factory integration. The direct handle is retained
until final UI delivery/result; peer close still is not independently monitored
while blocked by reliable batch/exit delivery. Local owner, jump/SFTP/tunnel and
remaining platform/semantic/GUI parity still precede simultaneous cutover.
