# Work: Isolated native desktop launcher

ID: US-0245
Intake: IN-0057
Created: 2026-11-12

## Status

Reopened for owner Release UAT feedback (2026-11-13): the probe does not look
like OneTerm, and requiring --allow-elevated-test prevents normal double-click
startup in the owner's environment. The owner explicitly withdraws the probe's
elevation opt-in policy. This packet removes that launcher-only gate and mode
split. Full workspace/native integration is a distinct outcome tracked in
US-0249; do not pass off this simple surface as that deliverable. Harness tool
discovery still returns no tool; mechanical reopen is unavailable, not claimed.

Reopened again for failed real desktop close acceptance (2026-11-12): owner
supplied a five-second cleanup timeout with three remaining session references.
Native owner completion afterward does not erase this failure.

Reopened for acceptance rework: explicit elevated desktop opt-in authorized by the
owner after ordinary launch failed. Desktop acceptance remains pending. Harness discovery returned
no tool; this authored record does not claim database status synchronization.

## Classification

- Change type: new capability within the accepted migration staging contract
- Risk lane: high-risk (terminal-controlled effects and session lifetime)
- Spec Intake: IN-0057

## Outcome

Provide an explicitly opt-in local native desktop window for honest interaction
acceptance without changing the normal application entry or factory.

## Scope

- In scope: app-owned default-off binary, real NativeLocalSession and
  NativeTerminalSurface, bounded event observation, observable errors and teardown.
- Out of scope: SSH launcher, production selection, persistence, full parity,
  program-controlled clipboard/notification side effects and automatic commands.

## Acceptance

- [x] Default application entry and backend selectors remain unchanged.
- [x] Explicit binary opens a marked native-only window with a real local shell
  (owner confirms the elevated-test UI; ordinary-token acceptance remains open).
- [ ] Launch with no arguments regardless of elevation classification; no
  --allow-elevated-test requirement or administrator-specific probe UI.
- [ ] Preserve the real token classification for underlying security checks;
  no token spoofing, self-elevation, production permission or persistence changes.
- [x] No settings/session/vault/layout files are loaded or written by startup.
- [x] All reliable events are drained independently of GUI rendering; OSC clipboard
  access and notification effects are explicitly denied, not silently applied.
- [ ] Output invalidates the native surface; errors and exit/closed state are visible
  (owner confirms output refresh; natural exit display remains unverified).
- [x] Closing requests shutdown; owner destruction and event-consumer termination
  are observed without blocking the GUI event loop (idle real-window close; pending
  admitted-operation close remains unverified).
- [x] Focused policy/lifetime proof, native build/Clippy, gate and desktop gaps recorded.

## Documentation

### Owning Docs Reviewed

- docs/PROJECT.md and docs/agents/crate-dependency-rules.md: composition-root boundary.
- high-level-design.md and low-level-design/ghostty-engine.md: native staging ownership.
- US-0239-ghostty-native-surface.md and US-0244-ghostty-search-navigation.md:
  caller-owned events/security and outstanding desktop acceptance.
- docs/decisions/DEC-0023-libghostty-vt-for-the-application.md: no premature cutover.
- docs/agents/error-policy.md: explicit runtime failures.

### Documentation Action

Update required: HLD and native LLD establish launcher isolation and ownership;
probe README documents invocation and limitations. No persistence schema changes.

### Reconciliation

HLD/LLD now describe launcher isolation, policy, event consumer and teardown.
Probe README documents invocation/limitations; PROJECT, structure and dependency
guides identify the optional binary and existing optional futures dependency.
US-0208 and US-0244 handoffs retain production and desktop acceptance gaps.

## Context

The pinned reference directory and srcwalk are unavailable. Inspect the resolved
project-local GPUI registry sources and existing app patterns instead. No new
third-party version or backend-to-UI dependency is needed.

## Plan

- [x] Add feature-gated composition-root launcher and bounded event observation.
- [ ] Prove policy and shutdown, compile and launch the isolated window.
- [ ] Record actual desktop evidence separately from simulated widget proof.

## Decisions

DEC-0023; launcher-specific boundary is recorded in the owning HLD/LLD.

## Verification Plan

2026-11-13 acceptance rework: remove LaunchMode and the opt-in parser, use one
title/banner for all launches, and accept only an empty command line. Preserve
the true token in the existing global and retain default OSC denial and native
cleanup contracts. Replace opt-in regression tests with strict no-argument
startup parsing tests and re-run actual Root lifecycle proof, native app tests,
all-target Clippy, full local gate and Release build. Existing privileged-message
probe scripts with exact ADMIN TEST titles are historical evidence only and must
not be reused unchanged. Do not auto-run the two owner-cancelled search repeats.
Full workspace delivery is not achieved by removing this gate.

Approved session-end failure policy: retain ordinary WM_CLOSE's five-second
foreground reference-drain path and post-loop owner wait. For app quit arriving
before root release, transfer the single launcher owner and consumer to a worker;
the quit observer awaits its result while GPUI releases windows and drives tasks.
Create a failure guard synchronously, before the observer future is polled. If
GPUI cancels/times out that future, or cleanup fails, flush an explicit diagnostic
and exit nonzero rather than allowing GPUI to report exit 0. This independent
session-end path has GPUI's 200 ms budget, not a five-second guarantee. No retry,
deadline extension, dependency changes or production entry changes. Test guard
drop/disarm and quit-vs-window-close routing; run native tests/Clippy/build, real
WM_CLOSE and targeted synthetic session-end, plus the full bundled local gate.

Session-end diagnostic continuation (no implementation change): resolved GPUI
platform.rs:1251 invokes App::shutdown and then process::exit(0), potentially
bypassing this launcher's post-GUI owner/consumer observation. Launch a new
ADMIN TEST process, retain its process handle, verify the visible title/PID and
the same PID's hidden Zed::PlatformWindow, and send WM_QUERYENDSESSION followed
by WM_ENDSESSION only to that hidden HWND. Never broadcast, log out, restart, or
target an existing user process. Observe exit and previously recorded direct
children under a bound without force-killing survivors. Retain actual teardown
logs; exit 0 without owner completion is not graceful-shutdown acceptance.
This is synthetic message-path coverage, not an actual OS session-end test.
Harness tool discovery remains empty; no database synchronization is claimed.

Real pending-search close continuation: extend only the ignored local probe to
foreground the PID/title-verified test HWND, send Ctrl+F and a plain search string
without Enter, then close within the debounce interval. Require the existing
surface-release diagnostic to report search busy=true before claiming pending
search coverage. Exit 0 alone cannot establish correct input routing. No shell
commands, forced termination, deadline relaxation or production code changes.
Retain failed/unsupported automation attempts as gaps, not passing UAT. This is
not Chinese IME or an admitted endpoint-acknowledgment proof.

Residual HWND-error acceptance rework: the resolved Windows backend lets default
WM_CLOSE reach DefWindowProcW (native destruction), while WindowsWindow::drop
later revokes drag/drop and destroys the same handle. Use only the isolated
window's public on_window_should_close callback to request GPUI remove_window
and return false, suppressing default native destruction. Keep Root release,
bounded cleanup, weak IME ownership and error logging unchanged. Compare the same
process/title-verified real WM_CLOSE procedure, including repeated runs, and keep
remaining platform errors if any. Add callback-level ownership regression, run
native app tests/Clippy/build and bundled gate. No registry/dependency edits.

Diagnostic continuation for the intermittent GPUI `window not found`: resolved
`gpui-util::ResultExt::log_err` records caller file/line in the `log::Record`, but
the probe's default env_logger formatter drops both fields. Update only this
isolated probe formatter to print source location when present, then repeat the
verified HWND-close probe. Use the resulting file/line to inspect the actual
callsite and classify whether it is expected teardown or a defect; do not change
the behavior or suppress the error in this diagnostic step. Run native app tests,
Clippy/build, full local gate, and repeated real HWND close. Keep production logger
configuration unchanged.

Source-location formatter identifies the log at resolved GPUI
`gpui-pre-0.3.9/src/window.rs:2017`: an async active-status callback's `.log_err()`.
Windows queues it from `WM_ACTIVATE`; closing before the callback runs makes the
window update return the normal closed-window error. This is an expected late
activation notification, not failed HWND/surface cleanup. The error remains
visible. Three further real close runs exit 0 and show no such log. Native tests
and build pass; full `scripts/ci-local.ps1` gate passes in
`.tmp/us0245-log-origin-ci.log`.

Real-close rework: inspect actual GPUI window/input-handler and native surface
ownership; reproduce with rendered/IME-connected surface lifetime, not just an
undrawn Root fixture. First add bounded lifetime diagnostics to distinguish an
unreleased surface from a detached search/drag task or retained frame element.
Keep the five-second deadline and failure persistence unchanged. Do not weaken
admitted-operation completion or interpret late native destruction as success.
Re-run native tests/build/Clippy and bundled gate after any implementation change;
real desktop reacceptance remains required. Owning LLD's bounded-cleanup contract
is already correct and remains unchanged pending causal evidence.
Actual HWND-specific WM_CLOSE reproduction now confirms surface alive at timeout
(.tmp/us0245-real-close-stderr.log, exit 1). GPUI input.rs ElementInputHandler owns
a strong Entity. Owning LLD now specifies native-only weak synchronous forwarding;
add retained-handler regression and re-run actual GUI close after the change.
The first CloseMainWindow experiment selected the console rather than the titled
GUI and exited with Ctrl+C status; it is not accepted as GUI close evidence.

Authorized acceptance rework (2026-11-12): add only --allow-elevated-test to the
independent binary; reject unknown/duplicate arguments before startup. Keep the
real token classification globally, never pretend an elevated token is ordinary.
Unknown tokens fail even with opt-in. Preserve no-persistence startup, OSC denial
and cleanup. Test ordinary/elevated/unknown with and without opt-in, strict parsing,
and visible mode labels; run native app tests/Clippy/build and the bundled gate.
Do not launch an elevated interactive shell automatically: provide the explicit
command after proof. This is elevated-only UAT, not ordinary-token acceptance.
Owning HLD/LLD and DEC-0023 updated before implementation. Harness rediscovery
again returned no tool; mechanical reopen is unavailable and not claimed.

Acceptance continuation: add a deterministic GPUI window fixture using the actual
Desktop root, real NativeLocalSession and independent event consumer. Remove the
window both idle and with a debounced search pending; prove the weak root expires,
native controls stop, session references release and wait/consumer complete. This
tests GPUI entity/task ownership with its test platform, not Windows GUI/OS IME.
If the current post-loop Arc::try_unwrap fails under that proof, rework the owning
launcher teardown here rather than inventing a separate shipped-defect packet.
The Root fixture confirms the timing gap: idle release leaves one session Arc,
pending-search release leaves two until the foreground executor advances. Rework
release to request stop, keep explicit-quit event processing alive while detached
surface cleanup drops references, then quit; bound that drain to five seconds and
retain the post-loop ownership check as an observable failure, never an implicit
retry of input. Re-prove Root removal and cleanup ordering with real sessions.
Next fault proof: retain an extra Arc across actual Root release, keep the test
executor running through the real five-second deadline, and verify the close task
releases its reference but persists a timeout failure. Release the injected owner
afterward and prove native wait/consumer finish without erasing that failure.
Test the same final cleanup-status check that run() uses; no shorter test-only
production deadline or permission bypass is introduced.

Run launcher policy/lifecycle tests, app native all-target Clippy, explicit binary
build, fmt/doc/English/dependency checks and scripts/ci-local.ps1. Launch only the
isolated binary for native UAT; do not infer IME/mouse/clipboard success from build
or window existence. Retain existing PowerShell/Fira Code failures as blockers.

## Evidence and Gaps

Continuation: the two asserted pending-search repeats were owner-cancelled and
are not counted as passes. The remaining verified probe was closed normally by
PID/title/HWND and left no direct children; its late-attached exit code was not
available. Ordinary-token startup remains blocked: source Explorer PID 6956 has
TokenElevation=true, so Explorer-token launches without opt-in exit 1; no ordinary
window was proved. Chinese locale 00000804 is configured but does not prove IME
composition. Owner must observe real composition, candidate commit and close on
the latest binary; Unicode paste/simulated commits cannot substitute.

Forced termination sample at 12:57:27Z: only newly created title/PID-verified
launcher 29320 was force-terminated, exit -1. Recorded conhost 33368, OpenConsole
33612 and cmd 29560 all exited within 15 seconds, without forced child cleanup.
No Rust teardown completion is claimed. WM_ENDSESSION and arbitrary descendants
remain unverified; see evidence/US-0245-real-close.md. Real close while an endpoint
acknowledgment is already admitted remains distinct from busy/debounced search.

- .tmp/us0245-tests-final.log: four launcher tests pass. Policy denies program
  clipboard/notifications, repeated closure cannot grow status, and elevated and
  unknown tokens fail closed. A capacity-one reliable queue drains 1000 requests
  without a GUI. The real ConPTY/native test waits for a published viewport before
  requesting close, then observes the consuming owner result and consumer join
  within a 25-second proof deadline. This is not a GUI release test.
- .tmp/us0245-app-tests.log: all 30 native-feature app tests pass; doc tests pass.
- .tmp/us0245-clippy-final.log: app native all-target Clippy passes warning-free.
- .tmp/us0245-build-final.log: explicit native desktop binary builds.
- .tmp/us0245-launch-final.log: actual launch exits 1 with token Elevated;
  ordinary-only guard stops before shell/GUI creation. No window, screenshot or
  desktop interaction success is claimed. Do not bypass that guard for UAT.
- .tmp/us0245-ci.log: bundled gate passes fmt and both workspace Clippy variants,
  then core PowerShell Enter guard fails (92 pass/1 fail, empty output versus
  INSTALLED:AcceptLine). Later gate steps did not run. Existing Fira Code native
  view, conhost/OSC110 and parity blockers remain as previously recorded.
- Dependency policy, doc paths, English contributor text and git diff whitespace
  checks pass. No production selector or persisted schema changed.

Window-lifecycle acceptance continuation:

- .tmp/us0245-window-root-release.log reproduced the reference timing gap with
  actual Root/Desktop/native-surface entities and real local endpoints: idle
  release leaves one reference, pending debounce leaves two. Waiting only after
  stopping GPUI cannot safely drive that detached foreground cleanup.
- Release now requests stop and schedules bounded reference draining while
  explicit-quit GPUI processing remains alive. Only the launcher and close task
  may remain before quit. Five-second timeout logs a failure and is remembered
  independently so post-loop teardown cannot silently turn it into success.
- .tmp/us0245-window-close-fixed.log: five launcher tests pass. The new Root
  fixture confirms Ctrl+F actually moves focus into search, types through the
  simulated window, closes before debounce finishes, observes the root's weak
  reference expire, advances the executor until only the launcher owns the session,
  then observes native wait and consumer termination without timeout. Idle close
  uses the same fixture. This does not hold an admitted native acknowledgment;
  US-0244's endpoint fixture covers that different boundary. Forced OS quit
  still needs direct desktop acceptance; the timeout branch is covered
  by the subsequent fault-injection proof below.
- .tmp/us0245-close-clippy.log: final native app all-target Clippy passes.
- .tmp/us0245-close-app-tests.log: all 31 native app tests and doc tests pass;
  .tmp/us0245-close-build.log: the updated explicit binary builds.
- .tmp/us0245-close-ci.log: fmt and both workspace Clippy variants pass; the same
  PowerShell guard still fails (92/1), so subsequent gate commands remain unrun.

Timeout continuation evidence:

- .tmp/us0245-timeout-tests.log: six launcher tests pass. Retaining one extra
  session Arc across actual Root release exhausts the real five-second deadline;
  the close task releases its Arc and check_cleanup() retains the failure. After
  releasing the injected Arc, native wait and consumer join finish, but the same
  final check used by run() still fails. TestPlatform quit is a no-op; this proves
  task ownership and failure persistence, not real Windows process exit.
- .tmp/us0245-timeout-clippy.log: native app all-target Clippy passes.
- .tmp/us0245-timeout-app-tests.log: all 32 app tests and doc tests pass.
- .tmp/us0245-timeout-build.log: latest isolated binary builds.
- US-0246 independently corrects the core guard probe route/module discovery;
  core now passes 93 tests. .tmp/us0246-ci.log passes fmt/both Clippy variants,
  then workspace tests stop at two existing ConPTY orphan checks (conhost.exe
  exit 0x6d); later gate commands remain unrun.
- Subsequent US-0247 corrects only the verified system-host exit-reason check;
  both orphan regressions now pass in default and native local-shell suites.
  Latest .tmp/us0247-ci.log passes fmt/both Clippy variants and advances workspace
  tests to the pre-existing Fira Code DirectWrite comparison failure (446/1/3
  ignored in terminal-view). Later gate steps remain unrun; this does not change
  the launcher's missing real desktop acceptance.
- US-0248 subsequently clears the font-version test blocker; .tmp/us0248-ci.log
  completes scripts/ci-local.ps1 with all checks passed (without optional --full).
  Production shaping is unchanged. Native-feature parity and real ordinary-token
  desktop acceptance remain separate and unproven by that default-feature gate.

Owner interactive UAT feedback (2026-11-12): reports all requested checks passed
in the ADMIN TEST window: generated 100 searchable output lines, Ctrl+F search,
mouse selection, wheel scrolling, copy/paste, Chinese IME entry and submission,
then closing with search open. This is owner-reported elevated desktop evidence;
no independent screen recording or OS-level trace was captured. Ordinary-token
behavior, explicit program-originated OSC denial observation, and close while a
native endpoint acknowledgment is admitted remain unverified.

Subsequent owner close logs invalidate cleanup acceptance: window-not-found and
invalid HWND errors at 10:33:01, event consumer ended, then at 10:33:06 cleanup
timed out with 3 session references. Native owner finally reports Stop::Requested,
lifecycle Complete, but run() exits with surface-cleanup timeout. The output proves
native termination eventually completed, not timely foreground surface cleanup.
Visible interaction feedback remains valid; real close acceptance is failed.

Remaining: ordinary-token desktop acceptance if an ordinary token can be made
available; verify admitted-operation window close and natural exit display.
Post-GUI teardown explicitly errors if entities/tasks retain session references;
idle/debounced-search Root release now has GPUI test-platform proof, but real
Windows GUI teardown after the owner-reported search-pending close and
admitted-operation window close remain unverified.
Native theme/default-color parity, SSH launcher and full production parity are not
claimed. Production migration remains open.

## Handoff

2026-11-13 owner UAT rework: removed launcher-only elevation admission gate,
LaunchMode and administrator-specific title/banner. The explicit test entry
requires no arguments; its old opt-in flag is no longer accepted. Actual token
recording, underlying production policies, default OSC denial and cleanup remain.
Native app tests 35/35 and all-target Clippy pass in
`.tmp/us0245-no-opt-in-tests.log` and `.tmp/us0245-no-opt-in-clippy.log`.
Full bundled gate passed in `.tmp/us0245-no-opt-in-ci.log` (optional -Full
cargo-deny not run). The subsequent optimized Release rebuild succeeded in
5m 40s, recorded in `.tmp/us0245-no-opt-in-release.log`:
`target/release/oneterm-native-desktop.exe`, 25,139,712 bytes,
last write 2026-11-13 00:38:55 local. This proves compilation, not real
no-argument startup or full workspace acceptance. The prior optimized build succeeded
in `.tmp/us0245-release-build.log`, but is the rejected standalone probe, not
the full workspace deliverable. No real no-argument launch has been observed yet.
US-0249 owns that distinct workspace integration; no workspace source is changed
in this gate-removal rework. Owner feedback does not erase earlier cleanup gaps.

Approved explicit session-end failure policy is implemented only in the isolated
launcher: synchronous cancellation guard, single-transfer owner/consumer worker,
and nonzero failure instead of GPUI's misleading exit 0. Focused native app tests
35/35, native all-target Clippy and explicit build pass. Rebuilt targeted
WM_ENDSESSION PID 32488 exits 1 with explicit incomplete-cleanup diagnostic;
consumer ends but owner completion is absent. Its three recorded direct children
disappear. This proves failure visibility, NOT successful graceful session end.
Normal real WM_CLOSE on that binary exits 0 with owner/consumer completion and
no timeout/invalid-HWND diagnostic. Full bundled scripts/ci-local.ps1 gate exits
0 with all checks passed in .tmp/us0245-quit-barrier-ci.log (optional -Full
cargo-deny not run; existing owner-accepted theme glyph exceptions remain).
See evidence/US-0245-real-close.md for logs and
limits. Admitted-operation HWND proof has not yet been implemented.

Session-end continuation at 14:44:07Z: targeted synthetic WM_QUERYENDSESSION and
WM_ENDSESSION to new PID 31952's verified hidden platform HWND exits 0, with no
surviving recorded direct children. Only root/surface release is logged; native
owner and consumer completion are absent. Resolved GPUI calls process::exit(0)
after App::shutdown, bypassing this launcher's post-GUI wait/join. This is not
graceful native cleanup acceptance or actual Windows logout evidence. An app-owned
quit observer/barrier would need to integrate cleanup before GPUI's immediate
exit; do not patch dependencies, relax the deadline, or count exit 0 as proof.
See evidence/US-0245-real-close.md. No implementation was changed in this
continuation. Real HWND close during admitted endpoint acknowledgment remains
unverified; current busy diagnostics cannot distinguish admission from debounce.

Owner selected quit-cleanup repair before admitted-operation instrumentation.
Feasibility review found GPUI's fixed SHUTDOWN_TIMEOUT is 200 ms, so a plain
on_app_quit future cannot inherit the five-second ordinary WM_CLOSE guarantee.
Implementation is paused for an explicit session-end failure policy rather than
silently changing deadlines or allowing process::exit(0) to mask missing cleanup.
Dependency source remains untouched. See the evidence's quit-barrier review.

Latest owner IME reacceptance (2026-11-12): owner reports completing real Chinese
composition/candidate/commit interaction and normal close in rebuilt ADMIN TEST
window PID 15128. This is owner-reported, not independently captured IME evidence.
At 13:26:58Z root, surface, consumer and native owner complete in the same second;
no cleanup timeout or invalid-HWND error appears. The known GPUI active-status
callback error remains logged at window.rs:2017. PID 15128 and its direct children
are absent afterward; OS exit code is unavailable. See
evidence/US-0245-real-close.md and .tmp/us0245-ime-manual.stderr.log. This supersedes
the request below for post-rebuild owner IME reacceptance only; ordinary-token,
active-composition close, admitted-operation close and production parity remain
unverified. UAC linked-token discovery failed before spawn with error 1312.

Owner rebuilt-binary close logs (2026-11-12 10:43:07Z): root and native surface
release, event consumer end and native owner Report { stop: Requested,
exit_code: None, lifecycle: Complete } all occur in the same second. No cleanup
timeout/final error is present in the supplied output. Surface reports search
busy=false and edit busy=false; this is not an admitted-operation close proof.
The logs do not independently confirm Chinese IME or search interactions on the
rebuilt binary. At that point window-not-found/invalid-HWND errors remained, as did the optional
DXGI debug warning and an accessibility-node advisory. Do not hide or declare
these fixed. Owner-supplied output corroborates timely cleanup after weak IME
forwarding; explicit process exit code was not supplied.

Real-close rework evidence: evidence/US-0245-real-close.md preserves the failed
HWND-specific reproduction and causal change. Native-only weak IME forwarding
removes the platform-to-surface strong ownership edge. The same real Windows
WM_CLOSE procedure now releases surface/consumer/native owner in the same second,
exits 0 and does not time out. A further GPUI close-order correction removes the
invalid-HWND errors: the isolated should-close callback removes the GPUI root and
prevents default WM_CLOSE destruction, leaving HWND destruction to GPUI. Three
consecutive real HWND closes exit 0; one has an intermittent `window not found`
log, two do not. That log remains unattributed. New retained-adapter regression proves stock strong retention,
weak release, Unicode commit forwarding and inert stale callbacks. Native view
suite: 465 passed, 3 existing ignored (.tmp/us0245-weak-ime-view-tests-final.log).
Native app tests and all-target Clippy pass (.tmp/us0245-weak-ime-app-tests.log
and clippy.log); binary build passes. Full gate completes with all checks passed
in .tmp/us0245-weak-ime-ci.log (without optional --full cargo-deny). The later
close-order callback change also passes the full gate in
.tmp/us0245-close-order-ci.log; native app suite has 33 passing tests and native
all-target Clippy passes. Three consecutive real HWND close probes exit 0 and
have no invalid-HWND errors. The intermittent `window not found` is attributed to
GPUI's queued active-status callback and remains unsuppressed; see
evidence/US-0245-real-close.md.
Owner must reaccept Chinese IME and search-pending
close on this rebuilt binary. Idle real GUI close is verified, not admitted-work
close or ordinary-token behavior. Deadline/failure persistence remain unchanged.

Owner desktop feedback (2026-11-12): launched the rebuilt executable from
C:\Users\mocius with --allow-elevated-test. Supplied logs confirm ADMIN TEST
warning, bundled ConPTY, AMD Radeon 780M and successful Direct3D 11.1 device
creation after the optional DXGI debug-interface warning (0x887A002D). The owner
reports the interface is fine. This is actual elevated-window/rendering feedback,
owner's subsequent interactive UAT feedback below covers visible interaction
paths. Ordinary-token behavior and independent owner/consumer teardown observation
remain unverified.

Elevated opt-in acceptance rework: .tmp/us0245-admin-app-tests.log passes all 33
native app tests and doc tests, including strict CLI parsing and the full token
matrix. Production entry, persistence and OSC denial are unchanged. Native
all-target Clippy (.tmp/us0245-admin-clippy.log) and explicit binary build
(.tmp/us0245-admin-build.log) pass. The rebuilt executable actually refuses default
elevated launch and opt-in with an invalid argument, both exit 1, before shell/GUI
creation (.tmp/us0245-admin-default-refusal.log and invalid-refusal.log).
Bundled gate (.tmp/us0245-admin-ci.log) exits 0: all checks passed, without
optional --full cargo-deny. Existing owner-accepted theme glyph exceptions remain.
No elevated GUI was launched by the agent. Owner reports the UI and listed
interactive paths pass; labels are not independently visually inspected. Actual
launch diagnostic before rework is
.tmp/native-desktop-launch-diagnostic.log (Elevated, exit 1).

Next: ordinary-token desktop launch and GUI teardown/UAT; current harness process
is elevated, so this session cannot claim that proof. Continue this packet for
launcher ownership and verification; US-0244 retains
search desktop acceptance, and US-0208 retains production parity/cutover.
