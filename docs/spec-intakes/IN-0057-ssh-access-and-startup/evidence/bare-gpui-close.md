# Bare GPUI Windows close diagnostic

Date: 2026-11-14. Owner: US-0249 acceptance rework. Local evidence only;
no upstream issue has been published and no dependency has been changed.

## Reproduction boundary

`crates/app/src/gpui_close_tests.rs` opens one bare `gpui::div` through
`gpui_platform::application()` with `QuitMode::LastWindowClosed`. It calls no
OneTerm application initialization, component initialization, asset provider,
workspace, terminal, SSH, SFTP, persistence or native library API. The test is
linked in the app test executable, so this is runtime isolation, not a claim
that the executable's dependency graph contains only GPUI. It works without
the `native-desktop` feature.

Each of three rounds now runs four children: immediate and first-frame default
close, plus immediate and first-frame admitted logical close. Logical close
uses on_window_should_close to call Window::remove_window and return false,
preventing default WM_CLOSE destruction. It uses no cleanup tasks or timers.
All modes use the unique title, verify the HWND's PID immediately before
posting WM_CLOSE, and observe exactly one GPUI window-close callback. The
parent enforces a 30-second per-child watchdog, kills only its own stalled
child, and retains separate stdout/stderr files under project-local `.tmp`.
There are no arbitrary GUI sleeps, quit-order workarounds or logger filters
that suppress errors. Error logs include source files/lines; the two known
error signatures also capture backtraces. Passing the diagnostic asserts
termination and close observation, not clean logs or a repair.

Run from the repository with the project-local Cargo environment configured:

```powershell
cargo test -p oneterm-app --lib gpui_close_tests::bare_gpui_owned_window_close_diagnostic -- --exact --nocapture
```

## Observations

Focused run: `.tmp/us0249-bare-gpui-proof.log`. Six of six children exited
successfully and observed one close. Every child reported four error records,
two with `0x80070578`, and zero exact `window not found` records. Detailed logs:
`.tmp/us0249-bare-gpui-22820-1794647577889374000/`.

The four errors in the inspected immediate-close child are:

- `directx_devices.rs:82`, `0x887A002D`: missing/mismatched DXGI debug SDK
  component during initialization; the framework then disables DirectX debug
  features and creates its D3D 11.1 device. This is not a close-race record.
- `window.rs:626`, `0x80040102`: RevokeDragDrop on an invalid window handle
  during asynchronous WindowsWindow drop.
- `window.rs:627`, `0x80070578`: DestroyWindow on an invalid HWND, with the
  captured stack passing through WindowsWindow drop and close_one_window.
- `platform.rs:493`, `0x80070578`: RedrawWindow on an invalid HWND, with the
  captured stack on WindowsPlatform::begin_vsync_thread.

These are pinned `gpui-pre-windows 0.3.9` sources in the project Cargo cache.
The VSync failure is therefore reproduced without terminal or SFTP cleanup.
The observed duplicate/late destruction error is a distinct framework path;
do not describe every invalid-HWND record as VSync. Six observations are not
a claim of deterministic behavior on every machine or schedule.

The source still queues activation callbacks that update a GPUI WindowHandle;
the previous real-workspace trace captures the missing-window error there.
The initial six-child schedule did not reproduce that signature. No synthetic
WM_ACTIVATE, dependency patch, delay or native destruction change was added
to force a desired result. The continuation below closes the isolated
reproduction gap, not the framework repair.

### Logical-close comparison (2026-11-14)

Focused command remains the same. New evidence:
`.tmp/us0249-logical-gpui-proof.log` and
`.tmp/us0249-bare-gpui-25252-1794648170959082300/`.
All twelve children exited successfully and observed exactly one close.

- Default WM_CLOSE: four of six children logged one exact `window not found`;
  all six logged two `0x80070578` records.
- Admitted logical close: two of six children logged one exact
  `window not found` and one VSync `0x80070578`; the other four logged neither
  signature. All six logged the expected logical-remove marker.
- All twelve also logged the startup DXGI debug SDK error. An error count of
  one in a logical child therefore does not mean a shutdown error.

In both inspected failing modes (`0-immediate` and `0-logical-immediate`),
the missing-window record originates at pinned `gpui-pre/src/window.rs:2017`.
Its stack includes Window::new's callback and the Windows event task. Debug
symbols label that task `update_ime_enabled::closure`; this symbol label alone
does not establish an IME cause. The source-qualified line is the
on_active_status_change callback's failed WindowHandle::update. The VSync
record remains source-qualified to `platform.rs:493` and its captured
begin_vsync_thread stack. The logical child does not emit the baseline's
RevokeDragDrop/duplicate-DestroyWindow errors.

Thus both originally observed shutdown signatures now reproduce without any
workspace/terminal/SFTP cleanup. Default-close children also reproduced the
missing-window signature in this run, so adding logical modes is not proof
that logical close is its sole cause. The data shows schedule-dependent
framework lifetime failures; neither deterministic reproduction nor a fix is
claimed. This test intentionally does not assert that an error must occur.

## Verification and preserved artifacts

The diagnostic is Windows/test-only and does not change shipping behavior.
Strict native app all-target Clippy passed. Native app library regression
passed all 44 tests with zero failures. The complete bundled
`scripts/ci-local.ps1` gate passed, including documentation/English checks.
Evidence: `.tmp/us0249-bare-gpui-clippy.log`,
`.tmp/us0249-bare-gpui-native-app.log` and `.tmp/us0249-bare-gpui-ci.log`;
background shell `sh_1d9551cc2001D82YgojcRn1tlv` completed successfully.
The five existing owner-accepted theme glyph contrast exceptions are unchanged.
Optional `-Full` cargo-deny was not run. Separate documentation-path, English
and diff checks also passed. These results validate the diagnostic increment,
not clean-log acceptance or a framework repair.

The logical-mode extension's strict native app all-target Clippy passed,
native app library regression passed all 44 tests with zero failures, and
the full bundled CI gate passed. Background shell
`sh_1d95e4502001qyIihpZ9SnmWFG` completed successfully. The five existing
owner-accepted glyph contrast exceptions remain unchanged; optional -Full
cargo-deny was not run. Documentation-path, English and diff checks passed.
These verify the extension, not a framework repair or clean-log acceptance.
Logs: `.tmp/us0249-logical-gpui-{clippy,native-app,ci}.log`.

The existing Release executable was not rebuilt. Its observed SHA256 remains
`F33F8FFFA786F96449DD986ED469BC6539BE7249E8554DE92FFCDCC02E915FB3`.
EXE-sibling data, predecessor EXEs, dependency caches and native libraries are
untouched. Production engine selection, theme, ordinary-token persistence
deferral and the five-second/200 ms cleanup budgets remain unchanged.

## Next boundary

### Investigation continuation (2026-11-14, after shared-focus Release)

Owner chose further investigation only: no dependency upgrade/patch, log
suppression, quit delay or publication. Re-ran the existing diagnostic with
the native-desktop feature enabled:

```powershell
cargo test -p oneterm-app --features native-desktop --lib gpui_close_tests::bare_gpui_owned_window_close_diagnostic -- --exact --nocapture
```

PASS: all twelve children exited and observed exactly one close. Evidence:
`.tmp/us0249-gpui-close-recheck.log` and
`.tmp/us0249-bare-gpui-23724-1794654467941065000/`.
Default-close children: five of six emitted missing-window, all six emitted
two invalid-HWND records. Logical-close children: five of six emitted
missing-window and one invalid-HWND record; the other child emitted neither.
All twelve also emitted the independent startup DXGI debug-SDK error.
Termination success is not clean-log acceptance.

Source-qualified records again point to three distinct lifetime paths:

- `gpui-pre/src/window.rs:1999-2017`: the platform active-status callback
  updates a WindowHandle after GPUI has removed its lookup.
- `gpui-pre-windows/src/events.rs:824-831`: activation is dispatched as a
  detached foreground task retaining WindowsWindowInner. The platform drop
  clears visibility callbacks but not active-status callbacks. Thus a later
  task can still invoke the removed GPUI window's callback.
- `gpui-pre-windows/src/events.rs:327-350` posts registry removal after
  WM_DESTROY. `platform.rs:1269-1282` removes the tracked HWND only when the
  posted message runs; the VSync reader at `platform.rs:477-495` can still
  reach that HWND and call RedrawWindow in the intervening lifetime gap.
- Default-close children additionally reach `window.rs:626-627`: default
  native destruction is followed by the platform wrapper's asynchronous
  RevokeDragDrop/DestroyWindow. Logical close avoids those duplicate-destroy
  records but does not repair activation/VSync lifetime ownership.

Repair requirements, not an applied patch: queued callbacks must be cancelled
or validated against a live window generation before updating GPUI; native
destruction and the VSync registry must share a lifetime protocol; default and
logical close must converge on one idempotent native-destruction owner. A lone
IsWindow check is not a proof against check/use races or HWND reuse. Any
synchronous registry removal must also account for existing VSync read locks
and Windows reentrant message dispatch rather than introducing lock inversion.

The inspected public OneTerm/GPUI seams do not expose ownership of these
private callbacks or the platform registry. No supported application-side
root-cause fix was identified in this pass. Do not change the cleanup budgets,
delay destruction, manipulate private HWND data, or hide logs to claim a fix.
An actual repair needs separately authorized dependency/upstream work and
regression coverage for both close paths, late callbacks, multiple windows
and active dialogs; publication remains separately authorized.

This continuation changes documentation only; no implementation or dependency
file was edited and no new full implementation gate is claimed. The earlier
shared-focus full CI result remains recorded in US-0249. Current Release is
unchanged at SHA256
`9508A08B266DC3C02B8CF40732CC404DFEF81BD5A40088F473A4D3D9C45AA65A`.
This supersedes the historical artifact identity above, not its test results.


This is an unpublished upstream-report starting point, not an independent
Cargo-package artifact or an upstream fix. Publishing a report or changing
dependency policy requires separate authorization. Both signatures now have
bare-window evidence. The next task is an upstream-compatible repair route,
not further attribution to terminal cleanup; retain the real-workspace trace
and keep native-workspace clean-log acceptance open until a repair is proved.
