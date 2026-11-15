# Work: Native workspace SFTP

ID: US-0252
Intake: IN-0057
Created: 2026-11-14

## Status

- [x] Planned
- [ ] In progress
- [x] Implemented

Harness discovery returned no tool; no DB registration is claimed.

## Classification

- Change type: new capability within accepted native migration
- Risk lane: high-risk (remote filesystem effects and connection ownership)
- Spec Intake: IN-0057

## Outcome

The native SSH pane provides the existing workspace SFTP browser with a real
SFTP channel on its authenticated connection, without a legacy terminal or
second login. Native consuming completion observes SFTP task destruction.

## Scope

- In scope: optional same-connection SFTP, capabilities/cwd/counters, switching
  active panes, cancellable setup and observed shutdown, loopback proofs.
- Out of scope: theme/input styling, permissions, production cutover, jump
  hosts/forwarding/logging, remote external-server tests without owner selection.

## Acceptance

- [x] One authentication/connection supplies both shell and SFTP.
- [x] Existing browser receives backend and live cwd through active pane state.
- [x] Unsupported SFTP logs a warning and leaves a usable native terminal.
- [x] Setup cancellation does not publish a leaked SFTP task.
- [x] Close/drop/remote exit cancels lookup and operations; completion observes
  task destruction even when browser retains a backend Arc.
- [x] Focused integration, native regression, strict Clippy and ci-local pass.
- [x] Preserve predecessor/data, optimized GUI Release and owned smoke pass.

## Documentation

### Owning Docs Reviewed

- docs/PROJECT.md and docs/HARNESS.md: staging, invariants and documentation.
- docs/agents/{code-style,structure,crate-dependency-rules,error-policy,persistence,dependencies}.md:
  layering, observable errors, isolated data and pinned dependency APIs.
- high-level-design.md, low-level-design/native-ssh.md, US-0251: existing
  direct-only scope, auth/trust, completion and pending/unplaced ownership.
- docs/sftp-browser-design.md: optional same-connection backend and file safety.
- Existing sftp_task/session/native channel/native pane and active state source:
  reuse transfer/path policy without detached native lifetime.

### Documentation Action

Update required: native-sftp LLD, intake HLD, PROJECT, SSH/backend/SFTP docs
to describe implemented scope and remaining desktop UAT separately.

### Reconciliation

Updated PROJECT, terminal-backend, ssh-client-connect, sftp-browser-design and
intake HLD/native-sftp LLD. No persisted schema or dependency change. Release
and full-gate evidence are recorded below; external-server desktop UAT and
the pre-existing GPUI close race remain separate open acceptance gaps.

## Plan

- [x] Add observed native SFTP owner and optional setup.
- [x] Publish native capabilities through the existing pane/active-state seam.
- [x] Prove supported/unavailable/cancel/close paths and run full gates.
- [x] Reconcile docs and preserve/build/verify Release.

## Decisions

DEC-0023 and DEC-0019 unchanged. See low-level-design/native-sftp.md.

## Verification Plan

In-process loopback SSH/SFTP fixtures only: directory/realpath/stat and
transfer roundtrip/cancellation, same connection, unsupported subsystem,
blocked setup/lookup, retained backend close and native completion; workspace
capability publication and local-pane isolation. Native regression, strict
native all-target Clippy, bundled ci-local, git diff --check. Release PE GUI
inspection and owned no-argument WM_CLOSE smoke. Physical external-server
browser/transfer UAT is separate; never infer it from fixture success.

## Evidence and Gaps

Owner desktop feedback (2026-11-14): "SFTP ok". This confirms the owner's
reported SFTP acceptance on the delivered native build. No separate claim is
made that every transfer, cancellation or reconnect UAT case was enumerated.
The accompanying Quick Connect Host border screenshot belongs to US-0249
input-style acceptance rework; theme and GPUI close-race scope are unchanged.

Continuation review: six native_connect tests pass in
.tmp/us0252-connect-proof.log. Before further implementation, reviewed shared
download/upload/staging/pipeline and pinned russh-sftp 3.0.0 client sources.
Immediate transfer abort skips staging cleanup; failed downloads also spawn
an unobserved close. Correct native ownership with a cancellable, destruction-
observed stream and separately drained transfer tasks, preserving legacy policy.
Blocked lookup/request/read/write retained-backend tests now PASS (four tests,
.tmp/us0252-blocked-proof.log): shutdown settles below the one-second test
bound, local download staging is removed and original local/remote targets
are preserved. These are deterministic fixture bounds, not a new timeout.
Native connection proof now includes realpath alongside directory/stat and
upload/download, one authentication, unsupported subsystem and setup cancel
(.tmp/us0252-connect-proof.log; six tests). Actual native pane publication and
remote/local/empty Space switching PASS (.tmp/us0252-pane-proof.log); identity
and cwd Arc are preserved and another workspace has no leaked backend.

The first broad gate run failed on a temporary callback lifetime in a newly
added blocked-write test (E0716); the callback now has a named binding. The
corrected final run PASS: seven native SFTP tests (including real SSH remote
exit/drop with retained backend) and one blocked-write cancellation test in
.tmp/us0252-final-focused.log; native six-crate regression 1,280 passed and
seven ignored in .tmp/us0252-native-regression.log (exclude the two child
process reruns from the sum); strict native app all-target Clippy PASS in
.tmp/us0252-native-clippy.log; complete scripts/ci-local.ps1 PASS in
.tmp/us0252-ci.log; git diff --check PASS. The complete bundled gate includes
its explicit ignored-test runs. Optional -Full cargo-deny was not requested.
The five previously accepted theme glyph exceptions remain unchanged.

Optimized Release build PASS (8m33s): target/release/oneterm-native-desktop.exe,
46,371,328 bytes, modified 2026-11-14T16:16:48.4272240+08:00, SHA256
F33F8FFFA786F96449DD986ED469BC6539BE7249E8554DE92FFCDCC02E915FB3;
PE subsystem Windows GUI (2). Evidence: .tmp/us0252-release-build.log and
.tmp/us0252-release-manifest.json. Hash-verified predecessor retained at
.tmp/us0252-predecessor-20261114-160812; EXE-sibling oneterm-native-data was
not changed by build/backup.

Owned no-argument startup/close smoke PASS for process/window/exit criteria:
PID 13852, HWND 3018116, exact native workspace title, zero application-owned
console windows at enumeration; ownership rechecked before targeted WM_CLOSE,
exit code 0. Evidence: .tmp/us0252-release-smoke-20261114-161700.json and
its .stdout.log/.stderr.log siblings. Inherited token Elevated, bundled ConPTY,
isolated storage; absent elevated settings defaulted without writes. Sampled
enumeration does not establish absence of every possible transient child window.

This smoke is NOT clean-log or upstream-race acceptance: stderr reproduces
invalid HWND 0x80070578 and window not found after close. No dependency patch,
suppression, delay or quit-order workaround was applied. The existing GPUI
race remains unresolved. Native SFTP implementation and packaged build are
verified by fixtures/gates; physical external-server SFTP/SSH UAT remains open.
OSC52, physical drag/IME and ordinary-token persistence gaps remain; theme is
paused, input styling still needs the affected field, and cancelled search
repeats stay cancelled.
