# Work: Terminal New SSH context menu

ID: US-0253
Intake: IN-0057
Created: 2026-11-14

## Status

- [x] Planned
- [x] In progress
- [x] Implemented

Harness tool discovery returned no tools; no database registration claimed.

## Classification

- Change type: new entry point for existing capability
- Risk lane: high-risk intake; existing auth/network contracts unchanged
- Spec Intake: IN-0057

## Outcome

Right-click a terminal or empty split, then New SSH, to select Quick Connect
or a saved session without navigating away from the terminal workspace.

## Scope

- [x] In scope: shared submenu in native, legacy and empty-Space menus;
  fresh grouped saved-session rows; stable-id dispatch and existing dialogs.
- [x] Out of scope: new SSH backends, connection/persistence policy changes,
  theme changes, auto-connect, replacing the clicked pane, release publication.

## Acceptance

- [x] Owner rework 2026-11-15: New SSH Here opens Quick Connect or saved SSH in the clicked empty Space, not a new tab or later active Space.
- [x] Missing/occupied destinations never overwrite content; unplaced native connections receive observed cleanup and a warning.

- [x] New SSH follows New Terminal in all three context-menu surfaces.
- [x] Quick Connect... opens the existing quick-connect flow.
- [x] Saved rows retain grouping/order and dispatch exactly their stable id.
- [x] Empty lists are inert; reopening reflects changes; long lists scroll.
- [x] Existing token guards, split/edit/close behavior and data are preserved.

## Documentation

### Owning Docs Reviewed

- docs/PROJECT.md and docs/HARNESS.md: composition and pre-code gate.
- docs/templates/work.md: packet shape.
- docs/agents/code-style.md, structure.md, crate-dependency-rules.md,
  error-policy.md and dependencies.md: style, layering and reference rules.
- docs/ssh-client-connect.md: shared stable-id dialog and placement contract.
- IN-0057 HLD and low-level-design/native-ssh.md: isolated native boundaries.
- Existing terminal + menu, WorkspaceCommands, session-ui public guards and
  terminal context-menu tests: reuse rather than new UI-to-backend edges.

### Documentation Action

Update required: intake, HLD, new low-level-design/ssh-context-menu.md and
docs/ssh-client-connect.md describe the new navigation entry. Existing
authentication and placement contracts remain unchanged.

### Reconciliation

Updated intake, HLD, low-level-design/ssh-context-menu.md and
docs/ssh-client-connect.md. No authentication, storage, teardown or dependency
contract changed. The new submenu delegates to existing guarded entry points.

## Context

reference/gpui-kit and srcwalk are absent. Read pinned registry
gpui-component-0.8.0/src/menu/popup_menu.rs before using submenu APIs.
Submenus build eagerly inside the parent context-menu callback, so a snapshot
read there is fresh per menu opening. No dependency changes.

## Plan

- [x] Add shared builder and integrate the three menu surfaces.
- [x] Verify callbacks, freshness and existing menus; reconcile evidence.

## Decisions

Use the existing WorkspaceCommands registry and new-tab connection flow.

## Verification Plan

Actual menu tests with application-scoped command probes (no network), native
terminal-view and session-ui regression, strict native app all-target Clippy,
scripts/ci-local.ps1, documentation checks and git diff --check.

## Evidence and Gaps

### Split destination acceptance rework (2026-11-15)
Delivered in rebuilt target/release/oneterm-native-desktop.exe, SHA256
BCFF049563BF2FB9E9776E6922BE7D93A20A9E5700B0B1BA1C21515268E06E65.
Owned Elevated-token Release startup/WM_CLOSE smoke passed (PID 16228, exit 0);
SSH sessions, vault, trust and settings hashes unchanged, normal dock save only.
See .tmp/split-release-manifest.json and
.tmp/split-release-smoke-20261115-134451.json. Known upstream post-close
window-not-found diagnostic remains deferred. Real-host/menu UAT stays open.

Verification: actual native/legacy menus expose New SSH on occupied terminals
and New SSH Here on empty Spaces; both Here submenu callbacks are exercised.
Native authenticated-session handoff is invoked after focus changes for Quick
Connect and saved rows: exact captured Space receives the pane, original
identity remains unchanged, repeated occupied/closed-target completions never
replace or fall back, and consuming native cleanup finishes successfully.
No external host contacted. Destination routing compiles through both actual
saved/quick dialogs and shared connection completion. Owning SSH LLD and
docs/ssh-client-connect.md reconciled. Full bundled CI, 1,297 app-inclusive
native regressions (7 existing ignored), strict native all-target Clippy and
git diff --check PASS; evidence .tmp/split-menu-focused.log and
.tmp/split-rework-{ci,native-tests,native-clippy}.log. Physical menu/real-host
UAT remains separate; no auth, vault, trust, theme or engine cutover changes.

Reopen this owning packet for New SSH Here. Capture a weak panel and stable
SpaceId through existing tagged connection completion, retaining authentication,
trust, vault, cancellation and engine selection. No fallback tab or replacement
when the target closes/fills; use existing observed unplaced cleanup. Occupied
terminal New SSH stays new-tab; local New Terminal Here stays. Reviewed commands,
common connection flow, saved/quick dialogs, duplicate placement and menu tests.
Update owning LLD and SSH docs; prove real menu destination dispatch/completion
and native lifecycle, run full gate/native suites/strict Clippy and rebuild.
No schema/theme/dependency changes. Harness unavailable; manual record only.

### Owner-requested local Release rebuild (2026-11-14)

Owner asks to check remaining issues and build a fresh Release if unblocked.
No new failure is outstanding for this menu slice. This is a local optimized
native acceptance build, not publication, production cutover or declaration
of full feature parity. Retain the previously deferred GPUI close-log issue,
unimplemented OSC52/jump/forwarding/logging requirements and incomplete
external-server/physical UAT. Existing data and credentials must be preserved.

Plan before replacing the artifact:
- [x] Rerun full ci-local and native menu/regression checks.
- [x] Preserve/hash-check predecessor EXE and record sibling data hashes.
- [x] Build existing native-desktop Release profile and verify GUI subsystem.
- [x] Launch one owned no-argument process, recheck PID/title/HWND before
  WM_CLOSE, record bounded exit and unsuppressed shutdown logs.
- [x] Reconcile artifact manifest, data preservation and remaining gaps.

No source, dependencies, persistence schema or shutdown policy changes are
part of this build. Never close an existing user process.

Fresh full ci-local PASS (.tmp/us0253-release-ci.log); native view 522 PASS/
3 existing ignored and session-ui 109 PASS/1 existing ignored
(.tmp/us0253-release-native-tests.log); strict native app Clippy PASS
(.tmp/us0253-release-native-clippy.log). Optimized build PASS in 672.8 seconds
(.tmp/us0253-release-build.log). Artifact:
target/release/oneterm-native-desktop.exe, 46,381,568 bytes, modified
2026-11-14T23:41:20.8514339+08:00, GUI subsystem 2, SHA256
63BD7F2A2A53C736666308FEA7A5B044E96592FF13CD468D833BED9917723888.
Manifest: .tmp/us0253-release-manifest.json. Hash-verified predecessor:
.tmp/us0253-predecessor-20261114-232417, previous SHA256
9508A08B266DC3C02B8CF40732CC404DFEF81BD5A40088F473A4D3D9C45AA65A.

Owned no-argument startup/WM_CLOSE smoke PASS: PID 13632, HWND 1902746,
exact native workspace title and executable path rechecked before close, exit 0.
Evidence: .tmp/us0253-release-smoke-20261114-234140.json and stdout/stderr
siblings. Inherited token Elevated, bundled ConPTY, isolated sibling storage.
All four existing data-file hashes unchanged after build and smoke
(.tmp/us0253-release-data-before.json). No saved host or credential used.
Neither known GPUI shutdown error appeared in this one smoke; the intermittent
defect remains deferred/open, not fixed by a non-reproduction.
Physical New SSH appearance, external-server duplication/disconnect and active
operation shutdown, drag/drop, OS IME and ordinary-token persistence remain
separate acceptance gaps. Four requested future native capabilities remain
requirements only; this build does not implement them. No public release,
version bump, commit or push performed.

Actual menu tests pass for native and legacy terminals and empty Spaces.
They verify New SSH placement, Quick Connect callback, ungrouped/grouped order,
stable ids 902/1234, refreshed lists, inert empty-list clicks and keyboard
navigation/scrolling to Host 60. Opening menus invokes no connect callback.
The probes intercept WorkspaceCommands without using a network or credentials.
Snapshots are unordered; tests sort rendered coordinates before order checks.

2026-11-14 proof:
- Focused actual-menu tests: 2 PASS (.tmp/us0253-focused.log).
- Native terminal-view: 522 PASS, 3 existing ignored; session-ui: 109 PASS,
  1 existing ignored (.tmp/us0253-native-tests.log).
- Strict native app all-target Clippy PASS (.tmp/us0253-native-clippy.log).
- Full scripts/ci-local.ps1 PASS (.tmp/us0253-ci-local.log), including default
  regression, policy/docs checks and unchanged known contrast exceptions.
- Initial CI stopped on added test-module ordering; corrected and reran the
  entire gate successfully. No failing verification remains.

Standalone terminal fixtures without AppServices omit the application submenu;
real composition installs commands before windows. Unknown-token gating uses
the existing action policy; public dialog entry points keep their own guards.
No process-global token mutation was introduced to these tests.
Physical owner visual acceptance is separate from headless tests. The initial
source-only delivery did not rebuild Release; the owner-requested rebuild above
supersedes that artifact status. Isolated data was not touched.
Optional -Full cargo-deny was not run. Deferred GPUI close issue is unchanged.

## Handoff

Not required; same-session implementation.
