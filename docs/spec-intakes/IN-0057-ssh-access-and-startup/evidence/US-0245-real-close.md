# US-0245 real Windows close rework

Date: 2026-11-12. Windows elevated opt-in test; not production or ordinary-token proof.

## Failure

After owner-reported successful visual/input UAT, closing the window reports:

```text
10:33:01 event consumer ended
10:33:06 cleanup timed out: 3 session references remain
10:33:06 owner destroyed: Report { stop: Requested, exit_code: None, lifecycle: Complete }
Error: Native desktop surface cleanup timed out before GUI exit
```

The first agent CloseMainWindow experiment selected the console window and ended
with 0xc000013a. It does not measure GUI teardown. A corrected local probe uses
EnumWindows, verifies the process ID and exact ADMIN TEST title prefix, then posts
WM_CLOSE only to that HWND. It runs no shell commands and does not forcibly kill
the process. Its 15-second process observation bound is not the production
five-second cleanup deadline.

At 10:36:56, the corrected probe reports root release and consumer end. At
10:37:01 the diagnostic reports 3 references and surface entity alive=true. No
surface release appears. Exit is 1. Thus a surface owner survives HWND/root removal.

Resolved gpui-pre 0.3.9 input.rs shows ElementInputHandler holds Entity<V>.
Windows platform/input frame state can retain that handler after root release.
The native surface installs it while focused. Headless Root release alone did
not reproduce that platform ownership.

## Change and causal check

Native-only WeakInputHandler stores WeakEntity and bounds. Each callback upgrades
temporarily and forwards to a temporary stock ElementInputHandler, preserving
live semantics. Dead-entity queries return neutral values and edits are inert.
The normal terminal and GPUI dependencies are unchanged. Native close timeout,
admitted-operation handling and timeout failure persistence are not weakened.

After replacing only that ownership edge, the same real HWND close procedure:

```text
10:38:49 root released; requesting session stop
10:38:49 surface released: references=3, search busy=false, edit busy=false
10:38:49 event consumer ended
10:38:49 owner destroyed: Report { stop: Requested, exit_code: None, lifecycle: Complete }
GUI close exit: 0
```

Windows invalid-HWND messages still appear during shutdown. They are not hidden
or claimed fixed; cleanup now completes without timeout. The observed session
reference count during release is transient (launcher/close task/surface), not
a count of surviving owners after asynchronous cleanup.

## Regression and limits

The new retained-adapter test first holds the stock strong handler beyond window
removal and proves it retains the surface; dropping it while retaining the weak
handler permits release. It checks live Unicode commit forwarding, native-clear
completion and inert stale callbacks. Its first run asserted asynchronous clear
completion too early (2 rather than 1 references); the fixture now drives the
executor until completion under a bounded deadline, rather than weakening the
expected final owner count.

The original close proof above is idle/focused terminal only; the later real
pending-search probe is recorded below. Owner must still recheck actual Chinese
IME against the rebuilt binary. An admitted endpoint operation held across real
window closure, forced OS quit and ordinary-token behavior remain open proof gaps.

## GPUI-owned HWND close rework

Resolved `gpui-pre-windows` 0.3.9 source explains the remaining native errors:
`WM_CLOSE` returns to `DefWindowProcW` when the should-close callback says yes;
Windows destroys the HWND and dispatches `WM_DESTROY`, then `WindowsWindow::drop`
later calls `ShowWindowAsync`, `RevokeDragDrop`, and `DestroyWindow` on that same
handle, logging failures. The isolated probe now intercepts should-close, marks
the GPUI window removed, and returns false to prevent default destruction. GPUI
remains sole HWND owner; production application behavior and pinned dependencies
are unchanged.

The corrected HWND-verified probe against this build reports same-second Root,
surface, consumer and native owner teardown with exit 0. The invalid-HWND messages
are gone. A source-location formatter in the isolated launcher attributed the
intermittent `window not found` to resolved GPUI `gpui-pre-0.3.9/src/window.rs:2017`,
the `.log_err()` on the async active-status callback registered around line 1999.
Windows `WM_ACTIVATE` queues that callback on the executor; if the window is
removed before it runs, `WindowHandle::update` returns the closed-window error.
This is a late activation notification, not a failed HWND destroy or native surface
cleanup. It remains logged and is not filtered. No dependency was modified.

Three further consecutive real HWND close runs each exit 0 with same-second
surface, consumer and owner completion. None emitted `window not found` in this
sample; the earlier 2/3 occurrence is not claimed impossible. No invalid-HWND
errors or cleanup timeout occur. Repeated output is retained in
`.tmp/us0245-log-origin-repeat.log`. Source-location instrumentation tests pass
(33) in `.tmp/us0245-log-origin-tests.log`; build succeeds in
`.tmp/us0245-log-origin-build.log`. Full `scripts/ci-local.ps1` gate passes after
instrumentation in `.tmp/us0245-log-origin-ci.log`.

Logs: .tmp/us0245-real-close-stderr.log (latest actual close),
.tmp/us0245-weak-ime-build.log, .tmp/us0245-weak-ime-view-tests-final.log,
.tmp/us0245-weak-ime-app-tests.log, .tmp/us0245-weak-ime-clippy.log,
.tmp/us0245-weak-ime-ci.log, .tmp/us0245-close-order-ci.log,
.tmp/us0245-log-origin-repeat.log, .tmp/us0245-log-origin-ci.log. Full results are
recorded in the owning packet.

## Real Windows pending-search close

The ignored local probe `.tmp/us0245-search-close-probe.ps1` finds only its own
PID/title-verified ADMIN TEST HWND, verifies foreground ownership, sends Ctrl+F
and a plain search string without Enter, then posts WM_CLOSE during debounce.
It sends no shell command and does not forcibly kill the process. At 11:39:20Z,
surface release logs `session references=4, search busy=true, edit busy=false`;
consumer and owner complete in the same second, lifecycle Complete, exit 0.
The extra session reference is drained without weakening the five-second bound.
The known GPUI active-status callback error remains visible at window.rs:2017.
This proves a pending native search driver at real Windows close, not that a
native endpoint acknowledgment was already admitted. Chinese IME and ordinary
token acceptance are not inferred from this automation.

The two assertion-enabled repeats were cancelled by the owner; the retained
repeat log has only the first run marker and no completed proof. They must not
be counted as passes or automatically restarted on the cancellation notice.

Ordinary-token discovery continuation: Explorer-mediated launch is being tried
without elevated opt-in and with direct TokenElevation verification. The first
helper failed C# compilation before launch; the second failed on PowerShell's
read-only PID variable after requesting launch. Neither is ordinary-token proof.
The corrected attempt remains unverified until a matching newly launched HWND,
token query and process completion are observed. No token or launcher gate is
changed. Read-only keyboard registry values report Chinese locale 00000804;
installation/configuration alone does not prove real Chinese composition.

Explorer-mediated launch found no ordinary-title window. A direct Explorer-token
broker then created a child but it exited with code 1 before window verification;
this is a failed startup attempt, not ordinary-token acceptance. The broker now
holds the child process handle while observing exit so PID reuse cannot be
mistaken for a remaining test process. Console-sharing retry also exits 1.
Direct TokenElevation query of source Explorer PID 6956 returns true: this is
not an ordinary token. The broker now rejects that source before spawning.
Earlier no-opt-in child exit 1 is consistent with the elevated-launch guard,
not a native ordinary-token initialization defect. No startup restriction was
relaxed; an actual non-elevated desktop launch remains required.

Follow-up probe queried the elevated caller's `TokenLinkedToken` directly.
`GetTokenInformation(TokenLinkedToken)` failed with Windows error 1312
(`The specified logon session does not exist`), before process creation. No
probe process was launched by this attempt. This environment therefore cannot
provide the proposed linked limited token; ordinary-token acceptance remains
blocked pending an actual non-elevated interactive session. No fallback to
Explorer or changed security policy is justified.

Cancellation cleanup: probe PID 11512 was verified by executable path, creation
time, ADMIN TEST title and HWND ownership as the cancelled pending-search run.
It was sent normal WM_CLOSE and exited within 15 seconds; no direct children
remained afterward. Its exit code was unavailable from the late-attached process
wrapper and is not recorded as 0 or counted as a successful repeat.

Remaining acceptance boundaries: registry locale detection is not Chinese IME
composition evidence. A search driver busy during close is not proof of an
already-admitted endpoint acknowledgment. Resolved Windows GPUI handles
WM_ENDSESSION on its separate hidden platform HWND, not the visible terminal
HWND; sending that message to the terminal HWND would be invalid evidence.
TerminateProcess bypasses Rust Drop entirely and must be assessed separately
from graceful WM_ENDSESSION, including native shell/conhost orphan observation.
No system-wide logout, restart or message broadcast is authorized or required.

## Forced parent process termination

At 12:57:27Z the ignored `.tmp/us0245-force-exit-probe.ps1` launched and verified
the PID/title-specific ADMIN TEST HWND for new PID 29320. It recorded direct
children with creation times: conhost PID 33368, OpenConsole PID 33612 and cmd
PID 29560. It force-terminated only the new launcher, which exited -1. All three
recorded direct children disappeared within the 15-second observation bound;
PID plus creation time was used to avoid confusing reuse with a survivor. No
child was force-killed by the probe. Retained stderr:
`.tmp/us0245-force-exit-stderr.log`.

There are no graceful surface/owner completion logs, as expected when Windows
terminates a process without Rust destructors. This is limited evidence of no
surviving recorded direct children for this idle forced-exit sample; it proves
neither native acknowledged-operation completion, arbitrary descendant cleanup,
nor Windows logout/WM_ENDSESSION behavior. The latter remains unverified.

## Owner interactive IME reacceptance

The rebuilt isolated ADMIN TEST window PID 15128 started at 13:06:05Z. The
owner was asked to use the actual Windows Chinese input method, observe
composition/candidates, commit Chinese without Enter, and close normally.
After clarification of the candidate location, the owner reported completion.
This is owner-reported interactive IME acceptance, not an independently captured
composition trace; no character injection or clipboard paste was performed by
the agent.

At 13:26:58Z the retained `.tmp/us0245-ime-manual.stderr.log` records root and
surface release (`search busy=false, edit busy=false`), consumer completion and
native owner lifecycle Complete in the same second. PID 15128 is no longer
running and no process with ParentProcessId 15128 was observed afterward. The
process handle was not retained across turns, so no OS process exit code is
claimed. There is no cleanup timeout or invalid-HWND diagnostic in this run.
The known GPUI queued active-status error at window.rs:2017 remains visible.
This does not prove close during active composition or an admitted endpoint
acknowledgment, and remains elevated-only acceptance.

## Targeted synthetic session-end message path

At 14:44:05Z a new ADMIN TEST launcher PID 31952 was started with its process
handle retained. EnumWindows verified both its exact visible ADMIN TEST title
(HWND 4002026) and same-PID hidden Zed::PlatformWindow (HWND 328656). Only that
hidden HWND received WM_QUERYENDSESSION (response 1) and, after identity
reverification, WM_ENDSESSION with wParam=1. No broadcast, logout, restart,
TerminateProcess, or message to another process was used.

The process exited 0. Previously recorded direct children conhost 8824,
OpenConsole 4940 and cmd 13772 all disappeared within the 15-second observation
bound (checked by PID plus creation time); none was force-killed. Retained log:
`.tmp/us0245-session-end.stderr.log`. At 14:44:07Z it records root release and
surface release with three session references, search busy=false, edit busy=false.
There is no event-consumer completion or native-owner completion record.

Resolved gpui-pre-windows 0.3.9 platform.rs:1251-1266 invokes the quit callback,
flushes logs and calls process::exit(0) when synchronous App::shutdown succeeds.
Resolved gpui-pre app.rs:1087-1105 supplies that callback. This bypasses the
launcher's post-GUI-loop session.wait and consumer.join in native_desktop.rs.
Thus this sample proves only targeted synthetic message handling, process exit
and no observed recorded-direct-child survivors. Exit 0 does not establish
completed native/Rust teardown. Actual Windows session end and graceful native
completion remain unverified; this path has an application cleanup-observation
gap, unlike the real WM_CLOSE path.

Admitted-operation investigation: native_surface_search.rs:257-273 waits out the
150 ms debounce before calling session.search; its busy flag spans both debounce
and endpoint acknowledgment. NativeLocal admissions precede reply-channel waits.
Existing native_surface_tests.rs:519-651 holds an endpoint acknowledgment and
proves ordered cleanup in the GPUI test executor, not a real HWND/ConPTY close.
A deterministic real-window proof needs explicit admission/acknowledgment evidence
and bounded orchestration; arbitrary sleeps or search busy=true cannot supply it.

Quit-barrier feasibility review: resolved gpui-pre app.rs:79 sets SHUTDOWN_TIMEOUT
to 200 ms. App::shutdown (1157-1202) creates quit-observer futures, clears windows
and flushes effects, then waits for those futures only within that budget. Merely
registering an asynchronous on_app_quit observer cannot provide this launcher's
existing five-second WM_CLOSE cleanup guarantee. A timed-out observer can be
dropped before the platform's process::exit(0). Do not ship an observer-only
change and claim the immediate-exit gap fixed. The owner selected fixing quit
cleanup first, but this newly discovered deadline constraint requires an explicit
failure policy before implementation. No source or dependency edits were made.

## Explicit quit-failure policy implementation and first proof

After owner approval, only the isolated launcher gained a synchronous quit
failure guard and a single-transfer ShutdownOwner (session plus consumer). App
quit before root release transfers that owner to a cleanup worker and awaits its
result in GPUI's observer. Dropping an unpolled or pending observer logs/flushes
the failure and exits 1. Normal WM_CLOSE retains the five-second root-release
task and post-loop wait/join. Production startup and dependencies are untouched.

Native app suite: 35 passed, including guard unpolled/pending cancellation,
disarming and close-vs-quit routing (`.tmp/us0245-quit-barrier-tests.log`). Native
all-target Clippy and explicit binary build pass (`.tmp/us0245-quit-barrier-clippy.log`,
`.tmp/us0245-quit-barrier-build.log`). Full bundled `scripts/ci-local.ps1` gate
subsequently exited 0 with all checks passed in `.tmp/us0245-quit-barrier-ci.log`.
Optional -Full cargo-deny was not run; existing owner-accepted theme glyph
exceptions remain. This gate does not turn the explicit session-end failure
below into successful graceful-shutdown acceptance.

At 15:24:37Z, rebuilt new PID 32488 received targeted messages on its verified
hidden platform HWND 722724 (visible ADMIN TEST HWND 396784). It exited 1 with
`app quit cleanup incomplete or cancelled within GPUI's quit budget; refusing
successful exit`. Consumer completion is present but native owner completion and
quit-cleanup success are absent. Recorded conhost 3272, OpenConsole 38760 and cmd
29252 all disappeared within the bound; no child was killed by the probe. This
is real message-path evidence of explicit failure rather than false success,
not graceful session-end acceptance. Diagnostic assertion intentionally failed
on exit 1; no timeout/deadline was relaxed. Logs:
`.tmp/us0245-session-end-fixed.result.log`,
`.tmp/us0245-session-end-fixed.stderr.log`.

At 15:25:03Z the same rebuilt binary's real HWND-specific normal WM_CLOSE exits
0 with same-second root/surface release, consumer completion and native owner
lifecycle Complete. No invalid-HWND or cleanup timeout appears in that sample.
Log: `.tmp/us0245-quit-barrier-window-close.log`. Session-end graceful completion
and admitted-operation HWND close remain open, independently of this normal-close
regression proof.
