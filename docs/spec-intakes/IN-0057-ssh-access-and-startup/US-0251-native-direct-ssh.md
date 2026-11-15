# Work: Native direct SSH terminals in the workspace

ID: US-0251
Intake: IN-0057
Created: 2026-11-14

## Status

- [x] Planned
- [x] In progress
- [ ] Implemented

Harness discovery returned no tool; no database registration is claimed.

## Classification

- Change type: new capability within accepted native migration
- Risk lane: high-risk (authentication, network effects and ownership)
- Spec Intake: IN-0057

## Outcome

Manual direct SSH Connect and Quick Connect in the isolated native workspace
create a real native pane through existing authentication and host-key UX,
with no legacy fallback and observable bounded teardown.

## Scope

- [x] In scope: tagged factory/UI seam, direct connection, native lifecycle,
  destination-aware duplicate placement, loopback regression.
- [x] Out of scope: SFTP, jump/forward ownership, output logging, automatic
  connections, production cutover, GPUI patch, token/persistence changes.
- Depends on US-0224 through US-0227 and US-0249.

## Acceptance

- [x] Native selection never creates a legacy terminal or dual parser.
- [x] Existing auth, trust errors/approval, deadlines and cancellation retained.
- [x] Unsupported configurations reject before network or credential use.
- [x] Configured scrollback and remote-origin OSC security reach native owner.
- [x] Real native session opens in existing panels and duplication carries tag.
- [x] Input, exit, close/drop and consuming completion pass loopback proof.
- [x] Native regression, strict Clippy and full local gate pass.
- [ ] External-server desktop acceptance recorded separately, not inferred.

## Documentation

### Owning Docs Reviewed

- docs/PROJECT.md and docs/HARNESS.md: production and documentation gates.
- docs/agents/{code-style,structure,crate-dependency-rules,error-policy,persistence,dependencies}.md:
  layering, policy, bounded visible errors and dependency constraints.
- docs/ssh-authentication.md and docs/ssh-client-connect.md: trust and auth UX.
- IN-0057/high-level-design.md and low-level-design/native-workspace.md:
  isolated startup and explicit native lifetime seam.
- low-level-design/native-ssh.md: direct-only owning contract for this slice.

### Documentation Action

Update required: native SSH LLD and intake HLD, PROJECT/backend descriptions
after implementation; preserve historical evidence and remaining limitations.

### Reconciliation

Updated native SSH LLD, intake HLD, PROJECT, terminal-backend,
ssh-client-connect and persistence contracts for the direct-only implementation,
isolated known_hosts, pending/unplaced cleanup and disabled native OSC 52.
No schema or dependency version changed for this slice. Native regression,
strict Clippy and the full local gate pass. Optimized Release build and owned
no-argument GUI/WM_CLOSE smoke pass, with the known upstream post-close
diagnostics still present. External-server desktop acceptance remains pending;
the overall packet is not marked accepted/implemented while that gap remains.

## Context

NativeChannel already owns an authenticated direct channel but not application
connection orchestration or NativeSession lifecycle. Reuse the existing trust
and authentication helpers, not a new credential/host-key implementation.

## Plan

- [x] Add configured native owner/lifecycle and direct connection orchestration.
- [x] Wire tagged factory, shared dialogs and duplicate destinations.
- [x] Prove loopback behavior and run regression/full gate.
- [x] Reconcile contracts, preserve and build Release only after gates pass.

## Decisions

DEC-0023 and DEC-0019 remain unchanged. Unsupported options fail explicitly
rather than silently losing requested effects or weakening security.

## Verification Plan

Focused native backend loopback/preflight/lifecycle and factory/UI tests;
native SSH/session-ui/view/app regressions; strict native all-target Clippy;
scripts/ci-local.ps1. Document unavailable external-server/ordinary-token UAT.
Preserve prior EXE/settings/evidence before any optimized Release replacement.

## Evidence and Gaps

### Owner screenshot follow-up (2026-11-14)

The supplied screenshot shows successful remote shell login and ls/ll output,
but the visible Session panel is the saved-session list, not a connected-session
indicator. The owner explicitly confirms testing
D:/work/OneTerm/target/release/oneterm-native-desktop.exe. Treat the screenshot
as owner-reported native remote-shell login/command evidence, not complete SSH
UAT (duplicate, disconnect and cleanup are not established by the image).
A later process snapshot found only an installed production process; that
snapshot cannot identify the earlier screenshot process. The prior attribution
to the installed build was unsupported and is withdrawn.
The daily UI config selects Alduin; native isolated ui_config.json is absent,
so native startup defaults to Zed One Dark. This does not establish the live
theme selected in the screenshot: an in-memory theme change need not persist
under the Elevated token. The earlier definitive theme-consistency conclusion
is withdrawn pending evidence of the theme actually selected in that window.
The shared terminal ANSI16 builder uses Tango unless overridden; foreground and
background follow the UI theme. Program ANSI output need not match UI accents.
Follow-up verification: five native_colors_ regressions passed, including live
theme observation without output and preservation/reset of program OSC colors;
evidence .tmp/us0251-theme-recheck.log. Source confirms partial theme alignment,
not a fully theme-derived terminal palette: build_terminal_theme uses UI default
foreground/background/caret but fixes ANSI16 to Tango unless settings override
it. For example Alduin UI base.blue is #87afaf while default terminal ANSI blue
is #3465a4 (bright #729fcf). This can explain different-looking command output
without proving the live screenshot's selected theme. These tests verify the
publication path, not pixel-perfect screenshot parity or live remote OSC state.
No palette policy or executable was changed during this read-only diagnosis.

Production SFTP normally follows the active SSH backend automatically. Native
US-0251 intentionally has no
SFTP channel/capability, and a manual connect step cannot enable it.

Owner follow-up (2026-11-14): SFTP is now explicitly requested for the native
workspace; this is a distinct missing parity outcome, not a failed password
or an accepted working feature. Source review confirms NativeSession has no
capability method, TerminalPane::Native returns empty capabilities, and native
connect opens only the shell before moving the handle into NativeChannel.
Reuse the existing same-connection optional SFTP backend and workspace active
state, never a second login or hidden legacy terminal. Before implementation,
establish its own packet/LLD for capability publication, cwd/counter state,
setup cancellation, retained backend references and observed task teardown.
The existing open_sftp helper starts a detached task and its uid/gid lookup
precedes the shutdown-select loop: simply calling that helper would not prove
bounded native completion. Add explicit owner observation and cancellation
proofs rather than wiring a live backend with unobserved work into shutdown.
No SFTP implementation or remote file operation is claimed by this review.

Launch repair: the project-local native EXE shortcut still contains the obsolete
--allow-elevated-test argument, which the new no-argument entry rejects. Back up
the shortcut and remove only that argument; do not replace the installed EXE or
close the user's active process. Verify shortcut target/arguments after saving.
Cleanup is limited to five verified empty, non-linked random scratch directories;
no source, library/cache, settings, predecessor EXE or proof artifacts are removed.
Evidence: .tmp/us0251-cleanup-20261114.json.

Pre-code packet and high-risk LLD established; no Harness database registration
is claimed because discovery exposed no Harness tool.

- Four native_connect loopback tests passed: preflight, unknown/approved/changed
  trust, auth rejection, pre/in-flight cancellation, native surface/PTY/env,
  resize/input/replies, exit, bootstrap, close/drop and OSC 52 unavailability.
- Two native shutdown tests passed: accepted raw bytes drain before close; hard
  close interrupts blocked reliable delivery during orderly shutdown.
- Tagged SSH duplicate placement/missing-destination and pending-connect
  cancellation/cancelled-observer tests passed in the native view suite.
- Two final unplaced regressions passed: pending event drain remains in close
  snapshot; missing event receiver remains a visible cleanup failure.
- Strict native all-target Clippy passed after the final source edits.
- Baseline full local gate completed successfully: .tmp/us0251-ci-local.log
  ends with ci-local: all checks passed. This run preceded the final unplaced
  cleanup edits and does not substitute for the final gate below.
- Final native regression passed: app 43, local-shell 76 (3 ignored),
  session-ui 108 (1 ignored), SSH 166, terminal 359, terminal-view 516
  (3 ignored), zero failures; .tmp/us0251-native-regression-final.log.
  Nested app subprocess tests are not added again to these suite counts.
- Final full local gate passed after the final source edits:
  .tmp/us0251-ci-local-final.log ends with ci-local: all checks passed.
  Existing theme-glyph exceptions remain owner-accepted; no new waiver added.
- Optimized native Release build passed in 11 minutes:
  .tmp/us0251-release-build.log. Executable:
  target/release/oneterm-native-desktop.exe, 46,190,592 bytes,
  modified 2026-11-14T13:54:14.4041754+08:00, SHA-256
  06C92131367BC3D8F71676011AFB110285E6372DEF9F28663FE18EB799411E75.
- Owned no-argument GUI/WM_CLOSE smoke passed: PID 17756, HWND 923524,
  exact title OneTerm Native Workspace (test), exit code 0; evidence
  .tmp/us0251-release-smoke-20261114-135415.{json,stdout.log,stderr.log}.
  Logs confirm isolated EXE-sibling storage, actual Elevated token, no settings
  writes under that token, default layout and bundled ConPTY. Post-close
  window-not-found and invalid-HWND diagnostics persist; a successful smoke
  exit does not resolve the upstream blocker or prove ordinary-token behavior.
  This smoke does not connect externally or prove external-server SSH UAT.
- Predecessor EXE and isolated data copied to
  .tmp/us0251-predecessor-20261114-133639; manifest retains SHA-256
  24762453BA75F6FEB23E29BA4F01393450D1ACF9B4EBE5F04017A46E1A735562.
- Documentation-path and English-text checks passed after contract updates.

Remaining gaps: external SSH desktop acceptance needs an explicitly selected
target/credentials; loopback is not that acceptance. Ordinary-token persistence
remains deferred. Native OSC 52 is unavailable even with permissive settings.
GPUI post-close window-not-found/invalid-HWND diagnostics remain an upstream
blocker; no dependency patch, delay, log suppression or quit-order workaround.
Physical drag/drop and OS IME candidate UI remain unverified.

Ownership review follow-up completed: unplaced native completion is retained in
workspace snapshots for a vanished duplicate destination; initial-CWD admission
failure retains the connection guard until cleanup completes. Pending-drain and
visible-failure regressions pass in the final suite. The final gate above covers
these source edits; the earlier full-gate run remains baseline evidence only.
