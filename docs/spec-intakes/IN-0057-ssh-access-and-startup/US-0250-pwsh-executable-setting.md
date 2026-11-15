# Work: PowerShell 7 executable setting

ID: US-0250
Intake: IN-0057
Created: 2026-11-14

## Status

- [x] Planned
- [x] In progress
- [x] Implemented

## Classification

- Change type: existing-contract change requested by the owner.
- Risk lane: high-risk boundary reviewed; elevation policy unchanged.
- Harness discovery returned no tools; no database update is claimed.

## Outcome

Settings > General > Shell exposes a startup executable input for PowerShell 7,
using the existing Custom Program input mechanics and shell.program storage.

## Scope

- In scope: contextual input label/help, settings round trip, ordinary Pwsh
  resolution and retaining the configured path when opening that same preset.
- Out of scope: privilege-policy changes, new schema, executable auto-selection,
  daily configuration import, unrelated native-theme acceptance or Release build.

## Acceptance

- [x] Selecting PowerShell 7 builds Startup Executable; empty means auto-detect.
- [x] Custom Shell retains Custom Program and its existing semantics.
- [x] A configured path survives settings serialization and new same-kind tabs.
- [x] Pwsh keeps its normal integration arguments, not Custom Shell behavior.
- [x] Elevated/unknown-token resolution still ignores user program paths.

## Documentation

### Owning Docs Reviewed

- docs/PROJECT.md and docs/agents/{code-style,structure,crate-dependency-rules,error-policy,dependencies,persistence}.md.
- docs/gui-layout.md: General Shell settings placement.
- docs/terminal-backend.md: shared LocalShellConfig and resolver.
- docs/decisions/DEC-0019-elevated-shells-open-in-an-elevated-window.md: M3/M4.
- IN-0057.md, high-level-design.md and low-level-design/native-workspace.md.

### Documentation Action

Update HLD, detail contract and GUI/persistence documentation. Reuse shell.program;
no migration or new field. Pinned reference checkout/srcwalk are absent; use
existing SettingField::input patterns and pinned local registry sources as needed.

### Reconciliation

HLD, low-level-design/pwsh-executable-setting.md, docs/gui-layout.md and
docs/agents/persistence.md describe the existing field's UI exposure and retained
privilege boundary. DEC-0019 needs no change: M3/M4 are not relaxed.

## Plan

- [x] Expose contextual executable input without duplicating state.
- [x] Prove persistence, preset spawn and unchanged privilege boundary.
- [x] Run focused tests and complete scripts/ci-local.ps1.

## Decisions

Reuse the existing optional program override; switching shell kinds still clears
it, avoiding execution of another shell's binary. Same-kind creation retains it.
DEC-0019 remains unchanged: this is not an administrator executable override.

## Verification Plan

Settings label/mutator/JSON mapping tests, ordinary core resolver argument test,
elevated Pwsh override rejection test, panel preset regression, settings/UI/core
tests, and the full scripts/ci-local.ps1 gate. No live configuration writes.

## Evidence and Gaps

Focused core/settings/settings-ui Pwsh tests pass (5/1/1), including explicit
path and integration args, JSON round trip, same-kind reselection, empty reset,
different-kind reset, field wording and elevated trusted-path replacement.
Evidence: `.tmp/us0250-pwsh-focused.log`. The actual native panel factory test
passes for default startup, matching Pwsh preset and new empty-Space terminal
retaining the path, with Cmd clearing it (`.tmp/us0250-panel-focused.log`).

Native app strict all-target Clippy passed (`.tmp/us0250-native-clippy.log`);
the complete native view suite passed 509 tests with 3 existing ignored tests
(`.tmp/us0250-native-view.log`); scripts/ci-local.ps1 completed all checks
successfully (`.tmp/us0250-ci.log`). Physical Settings UI/ordinary-token
persistence acceptance is not automated here; no daily or isolated live
settings were written.
Release rebuilt on 2026-11-14 at 12:15:56 +08:00, including this setting and
the preceding theme repair. The first build could not replace the running old
EXE (access denied); after the owner closed it, the retry passed in 9m 29s.
Evidence: `.tmp/us0250-release-build{,-retry}.log`; predecessor retained under
`.tmp/us0250-pre-rebuild-*`. New executable:
`target/release/oneterm-native-desktop.exe`, 45,741,056 bytes; SHA256
`EB8A164C095E10422F3D7B133909C2782EA0B3434E89EFEFF104761A10C7F653`.

No-argument real GUI smoke passed: PID 7428, HWND 200896, title
`OneTerm Native Workspace (test)`; PID-checked WM_CLOSE exited with code 0.
Evidence: `.tmp/us0250-release-smoke-20261114-121610.{json,stdout.log,stderr.log}`.
The process used EXE-sibling `oneterm-native-data`, bundled ConPTY and an
Elevated token; ordinary-token persistence and physical Settings acceptance
remain unverified. Post-close GPUI `window not found` and invalid-HWND errors
remain visible and unresolved despite exit 0. This is a startup/close smoke,
not full native migration or owner theme acceptance.
