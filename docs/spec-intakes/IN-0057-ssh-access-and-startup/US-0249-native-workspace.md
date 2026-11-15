# Work: Native engine in the OneTerm workspace

ID: US-0249
Intake: IN-0057
Created: 2026-11-13

## Status

- [x] Planned
- [x] In progress
- [ ] Implemented

Harness tool discovery is empty; no database registration is claimed.

Acceptance rework reopened (2026-11-14): ANSI defaults still use Tango, so
theme acceptance remains unchecked. Harness discovery was retried and returned
no tool; this authored packet records the rework without claiming a DB update.

### ANSI theme rework

Outcome: both shared terminal render paths derive ANSI 0-15 defaults from the
active UI theme, including idle live changes. Use base red/green/yellow/blue/
magenta/cyan in ANSI order; background and foreground supply black and white,
muted foreground supplies bright black. Bright chromatic colors and bright
white raise HSL lightness by 20% of the remaining distance to white. UI
base.*.light tokens are translucent/dim surface variants, not ANSI brights,
and must not be interpreted as brighter terminal ink. Composite translucent
base colors against the theme background before converting to opaque RGB.
User ANSI overrides win over defaults; program OSC overrides win until reset.
Indices 16-255 and truecolor behavior remain unchanged. No theme-file, schema,
storage, permission, SSH, lifecycle budget or production-engine change.

Reviewed: docs/PROJECT.md, docs/HARNESS.md, docs/templates/work.md,
low-level-design/native-workspace.md, theme builder/palette/tests, native
surface color publication and pinned gpui-component 0.8.0 theme schema.
Update the owning native workspace LLD with this mapping and precedence.
The reference checkout/srcwalk remain absent; registry sources prove the API.

Acceptance and verification plan:
- [x] Embedded Alduin base colors and all normal/bright ANSI slots are proved.
- [x] Live palette-only theme changes publish without terminal output.
- [x] Settings and OSC priority/reset, extended indices and truecolor are proved.
- [x] Focused tests, full native regression, strict native Clippy and ci-local pass.
- [x] Preserve predecessor EXE/data, build optimized Release and run owned GUI smoke.
Physical theme-selection UAT and existing upstream post-close errors remain
separate gaps; automated palette proof is not owner visual acceptance.

ANSI verification (2026-11-14): three new regressions failed against the old
Tango builder, then passed after the shared builder correction. Theme suite
13 PASS and native color publication suite 6 PASS. Full native regression:
app 43, local 76, session-ui 108, SSH 166, terminal 359, view 519: 1,271 PASS,
7 existing ignored, zero failures. Strict native app all-target Clippy, bundled
scripts/ci-local.ps1 and git diff --check PASS. Optional -Full cargo-deny not
run; five existing owner-accepted glyph contrast exceptions remain unchanged.
Evidence: .tmp/us0249-ansi-{red-tests,theme-tests,native-colors,
native-regression,native-clippy,ci-local}.log. Owning native-workspace LLD
now records the exact palette derivation and precedence; no persisted schema,
dependency or terminal-engine selection changes. Predecessor EXE preserved
with matching SHA256 06C92131367BC3D8F71676011AFB110285E6372DEF9F28663FE18EB799411E75
under .tmp/us0249-ansi-predecessor-20261114-141641; isolated data untouched.
Optimized Release rebuild PASS (6m55s): target/release/oneterm-native-desktop.exe,
46,191,616 bytes, modified 2026-11-14T14:32:03.5237750+08:00, SHA256
EF210B3A45440510EC5C37C897EC18108819DF5E80C521C456533D4F39978D5D.
Evidence: .tmp/us0249-ansi-release-{build.log,manifest.json}. No-argument owned
GUI smoke PASS: PID 19792, HWND 2297518, exact native workspace title; HWND
ownership rechecked immediately before targeted WM_CLOSE, exit 0. Logs confirm
EXE-sibling isolated storage, actual Elevated token, bundled ConPTY and absent
settings defaulted without writes. Evidence:
.tmp/us0249-ansi-release-smoke-20261114-143214.{json,stderr.log,stdout.log}.
Invalid HWND 0x80070578 still appears after close; it is not fixed or hidden.
Passing smoke proves startup/exit, not physical theme-selection visual UAT,
ordinary-token persistence or complete SSH desktop acceptance. Those gaps,
native SFTP/OSC52 limitations and unchanged close budgets remain in force.

## Classification

### Bare GPUI close diagnostic acceptance rework (2026-11-14)

Outcome: retain a process-isolated, GPUI-only Windows close reproduction that
distinguishes framework callback/redraw failures from workspace and terminal
cleanup. This serves the existing unaccepted close criterion, not a new product
capability. Harness tool discovery again returned no tool; no DB reopen is
claimed. The existing owning acceptance remains open.

Scope: a test-only bare GPUI view, PID/title-verified HWND WM_CLOSE, child-process
watchdog, source-qualified error logging and diagnostic backtraces. Exclude
OneTerm initialization, persistence, workspace, terminals and SFTP. Do not alter
dependencies, framework code, production behavior, lifecycle budgets, theme or
the verified Release executable. Do not publish an upstream issue.

Owning docs reviewed: docs/PROJECT.md (entry/production boundary),
docs/HARNESS.md and docs/templates/work.md (acceptance rework),
low-level-design/native-workspace.md (unchanged close budgets),
docs/agents/dependencies.md (published GPUI family/no UI patch), and
crates/app/src/native_workspace_live_tests.rs (owned HWND/process precedent).
Documentation action: no runtime contract change; append measured diagnostic
results and gaps here. The native cleanup contract already states the required
behavior and is not repaired by an isolated framework reproducer.

Acceptance and verification plan:
- [x] Bare view runs on the real Windows platform in its own bounded process.
- [x] Close targets only the exact child-owned HWND; logs preserve provenance.
- [x] Record reproduction/non-reproduction without claiming a race is fixed.
- [x] Focused diagnostic, strict native all-target Clippy, full ci-local,
  documentation and diff checks pass; record unavailable or failed checks.
- [x] Preserve Release/data and leave production dependencies unchanged.

Focused result: the GPUI-only test passed; six children closed and exited,
all six emitted two invalid-HWND 0x80070578 records and no exact missing-window
record. The detailed stack proves both WindowsWindow drop/DestroyWindow and
VSync RedrawWindow paths; startup DXGI debug SDK and RevokeDragDrop errors
are separately identified, not counted as missing-window failures. See
[bare GPUI close evidence](evidence/bare-gpui-close.md). Background shell
sh_1d9551cc2001D82YgojcRn1tlv completed successfully: strict native app
all-target Clippy PASS, native app library regression 44 PASS/zero failures,
and the full bundled scripts/ci-local.ps1 gate PASS. Evidence:
.tmp/us0249-bare-gpui-{clippy,native-app,ci}.log. The gate retains five
existing owner-accepted glyph contrast exceptions; optional -Full cargo-deny
was not run. English, documentation paths and git diff --check also passed.
The Release hash remains F33F8FFFA786F96449DD986ED469BC6539BE7249E8554DE92FFCDCC02E915FB3.
Reconciliation: only this packet and the evidence report require diagnostic
documentation updates; the runtime cleanup contract remains unchanged. This
completes the diagnostic increment, not the owning native-workspace acceptance.
The missing-window isolated reproduction and actual framework repair remain
open. No repair or deterministic race claim is made.

Logical-close diagnostic continuation (2026-11-14), pre-code gate: the bare
baseline lets default WM_CLOSE destroy the HWND before GPUI logical removal,
whereas the native workspace admits close then uses Window::remove_window.
Extend the same isolated test with immediate and first-frame admitted logical
close modes using that public GPUI API, without cleanup tasks, delays, synthetic
activation messages or a replacement quit policy. Keep the default-close
baseline for comparison. Reviewed the existing evidence, native workspace close
hook and pinned GPUI app/window/events teardown implementation. The absent
reference checkout still requires registry-source fallback. No runtime contract
change; update this packet and evidence report only.

Acceptance/verification:
- [x] Compare default and admitted logical close in isolated owned children.
- [x] Retain source/stack evidence and distinguish observed from inferred paths.
- [x] Focused diagnostic, native app tests, strict native Clippy and full bundled
  CI pass; record gaps and preserve the existing Release/data/dependencies.

Focused continuation PASS: twelve children exit successfully with one close
each. Exact window-not-found reproduces in four/six default-close children
and two/six logical-close children; the latter two also reproduce VSync invalid
HWND. All six default-close children reproduce both invalid-HWND records.
Source-qualified window.rs:2017 identifies the active-status callback; debug
stack symbol update_ime_enabled alone is not an IME-cause attribution. Detailed
comparison is retained in evidence/bare-gpui-close.md and
.tmp/us0249-logical-gpui-proof.log. Both original signatures are now isolated
from terminal/SFTP/workspace initialization; actual repair remains open.
Verification completed successfully in background shell
sh_1d95e4502001qyIihpZ9SnmWFG: strict native app all-target Clippy PASS,
native app library regression 44 PASS/zero failures, full bundled
scripts/ci-local.ps1 PASS. Logs:
.tmp/us0249-logical-gpui-{clippy,native-app,ci}.log. Optional -Full
cargo-deny was not run; the five existing owner-accepted glyph contrast
exceptions remain unchanged. Documentation paths, English and diff checks
also passed. Reconciled this packet and evidence/bare-gpui-close.md; the
runtime contract is unchanged. The isolated reproduction gap is closed,
but actual framework repair and native-workspace clean-log acceptance remain
open. No dependency, data, theme, close-budget or Release change is made.


- Change type: new capability within the accepted engine migration
- Risk lane: high-risk (native ownership, sessions, terminal effects and public seams)
- Spec Intake: IN-0057

## Outcome

The explicit native test executable presents the real OneTerm workspace and
drives its terminal panes with native sessions, rather than a standalone probe
or a hidden fallback to the production engine. No elevation opt-in argument is
required to open it.

## Scope

- In scope: existing workspace chrome, tabs/Spaces, themes/settings, native local
  and SSH view/service seams, independent reliable-event ownership and teardown.
- Out of scope: production-default cutover, rewriting global elevation policy,
  automatic elevation, dual parsing, external oneterm-vt API removal.
- Depends on US-0208 native adapter, US-0239 through US-0244 native surface, and
  US-0245 launch/cleanup proof. Their unverified gaps remain visible.

## Acceptance

### Split-close entry acceptance review (2026-11-14)

Owner asks how to close one split and requests a right-click menu if missing.
Outcome: prove a discoverable Close Space entry closes the right-clicked Space
only, preserves its siblings and collapses to a borderless single Space.
Review found existing entries in native_pane_menu.rs, input/menu.rs and
space/render.rs, gated to more than one leaf; no duplicate menu is needed.

Pre-code documentation review: docs/PROJECT.md, docs/HARNESS.md,
docs/templates/work.md, docs/agents/code-style.md, docs/agents/dependencies.md,
docs/terminal-split.md, IN-0018 high-level design, Space renderer, native menu,
panel close/shutdown mechanics and existing actual-menu split tests.
Documentation action: record existing entry usage and interaction proof here;
amend IN-0057/HLD/native-ssh requirements for the four independently scoped
future capabilities without implementing them in this acceptance repair.
No destructive-close policy, theme, dependency or persistence change.

Verification plan:
- [x] Actual native context menu closes an empty sibling via Close Space.
- [x] Actual occupied-pane context menu closes that pane, preserving sibling
  identity and observing its own completion; one leaf offers no Close Space.
- [x] Native focused/full view tests, strict native Clippy, full bundled CI
  and documentation/diff checks pass for any added regression coverage.

Verification (2026-11-14): actual-menu regression passes for empty Spaces in
all four split directions and an occupied native pane that was not previously
active. The target completes once; the surviving original session remains
alive and keeps its Space id. The single-Space menu omits Close Space.
The test uses viewport quarter-points for equal split hit-testing because the
test snapshot does not expose the empty Pane node; actual menu items are
located by role/label and clicked, not replaced with direct close calls.
Native terminal-view suite: 520 PASS, 3 existing ignored, zero failures.
Strict native app all-target Clippy and full scripts/ci-local.ps1 PASS.
Evidence: .tmp/us0249-close-space-menu-focused.log,
.tmp/us0249-close-space-native-tests.log,
.tmp/us0249-close-space-native-clippy.log and
.tmp/us0249-close-space-ci-local.log. git diff --check PASS.
Harness discovery again exposes no harness tool; no database update claimed.
No runtime menu change or Release rebuild is needed: Close Space already
exists. Usage: right-click inside the split to remove, then Close Space;
Close Terminal Tab is the separate whole-tab action. If context menus were
disabled in terminal settings, enable them to use this entry.
Four native capabilities are requirements only, not implemented by this work.
Existing owner visual acceptance and deferred GPUI shutdown gaps remain open.
Optional -Full cargo-deny was not run.


### Shared control focus acceptance rework (2026-11-14)

Owner reports Combobox still has a doubled edge and asks to inspect other
controls. Reopen the same input-style acceptance: the previous three-field
repair was too narrow. Outcome: use the pinned component library's supported
edge-only focus policy across application controls, preserving keyboard focus
indicators and all existing colors, selection styles, radius and persistence.

Pre-code review: docs/PROJECT.md, docs/HARNESS.md, docs/templates/work.md,
docs/agents/code-style.md, docs/agents/dependencies.md, docs/gui-layout.md,
theme initialization, Quick Connect and pinned component styled/input/combobox/
select/button/checkbox/radio/switch/number-input/input-group source. The reference
checkout and srcwalk remain absent; registry-source fallback is explicit.
Theme.focus_ring=false removes only the additional outer band: bordered
controls keep a tinted edge and borderless controls retain a single focus cue.
Apply this as application control chrome beside the existing fixed radius,
not a theme-file/color change or per-control disabling of focus. Remove the
Quick Connect-only override in favor of the shared policy.

Documentation action: update docs/gui-layout.md and this owning packet.
No auth, dependency, persistence schema or shutdown behavior changes.
Harness discovery was previously empty; no DB reopen is claimed.

Acceptance/verification plan:
- [x] Audit all shared focus decoration callers and record exceptions.
- [x] Edge-only policy survives startup, mode/config changes and theme updates.
- [x] Controls retain focus capability; focused tests, native strict Clippy and
  full bundled CI pass.
- [x] Preserve predecessor/data, rebuild optimized Release and pass owned GUI smoke.
- [ ] Owner confirms physical Combobox and other control focus appearance.

Implementation: set Theme.focus_ring=false beside fixed startup control chrome.
Pinned Theme::apply_config never assigns this field and Theme::change edits the
existing theme, so repeated resets/observers are unnecessary. Removed the local
quick_connect_input bypass and restored standard Input construction; the input
regression now keeps component focus decoration enabled under the shared policy.
No palette file, dependency, credentials or persistence path was edited.

Audit: Input, Combobox, Select, NumberInput, Checkbox, Radio and Switch call
focus_ring_style; Button uses the same focus_style path with variant-appropriate
edge/inside/outside placement for borderless buttons. InputGroup calls the outer
ring helper directly but gates it with theme.focus_ring and preserves its focused
border. ColorPicker, DatePicker, TimeField, OTP and DataTable also consume the
shared focus style. No OneTerm per-control focus_ring/focus_bordered opt-outs
remain. Slider has no matching shared outer-ring caller; no speculative slider
change is made. Dialog shadows, popup surface borders, selection cues and
non-focus outlines are intentional separate layers, not removed by this repair.

Focused proof PASS: cargo test -p oneterm-theme --lib focus -- --nocapture,
two tests. Real startup initialization, both mode switches, every registered
theme config and later Theme::update preserve edge-only policy; unrelated
font updates preserve the palette. Shared bordered decoration retains the ring
color with nonzero alpha. Actual rendered Input, Combobox, Select, Button,
Checkbox, Radio, Switch and NumberInput each accept keyboard focus. These are
policy/style/focus proofs, not pixel comparison or physical owner acceptance.
The first focused invocation timed out compiling dependencies at 120 seconds;
rerun completed successfully with a longer limit.

Verification completed in shell sh_1d9b2ef31001tU4RdpblOuBHOp: theme suite
12 PASS, native session-ui 109 PASS/one existing ignored, strict native app
all-target Clippy PASS, and full scripts/ci-local.ps1 PASS. The bundled gate
includes explicit ignored-test runs. Five existing owner-accepted glyph contrast
exceptions are unchanged; optional cargo-deny (-Full) was not run. Evidence:
.tmp/us0249-shared-focus-{tests,theme,session,clippy,ci}.log.

Predecessor preserved and hash-verified at
.tmp/us0249-shared-focus-predecessor-20261114-185618, SHA256
71694DC0D77F77A91A9C44739AE85998050C8D6ACED344E32166738E5D12C92E;
EXE-sibling data remains untouched. Optimized Release build PASS in 7m27s,
shell sh_1d9b39928001Q5XVAxbKP24fLj. Executable:
target/release/oneterm-native-desktop.exe, 46,371,328 bytes, modified
2026-11-14T19:03:55.9988146+08:00, GUI subsystem 2, SHA256
9508A08B266DC3C02B8CF40732CC404DFEF81BD5A40088F473A4D3D9C45AA65A.
Identity/build evidence: .tmp/us0249-shared-focus-release-{manifest.json,
backup.json,build.log}.

Owned no-argument startup/WM_CLOSE smoke PASS: PID 28724, HWND 4854176, exact
native workspace title; PID/title/HWND rechecked immediately before targeted
WM_CLOSE, exit 0. Evidence:
.tmp/us0249-shared-focus-release-smoke-20261114-190410.{json,stdout.log,stderr.log}.
Inherited token Elevated; isolated EXE-sibling storage; bundled ConPTY. Missing
settings defaulted without writes. Both known GPUI shutdown signatures appeared:
window not found and invalid HWND 0x80070578. This proves startup and process exit,
not clean-log acceptance or framework repair. Shutdown budgets and logging are
unchanged. Physical owner confirmation of Combobox and other controls remains
open; this smoke does not manipulate their UI.


### Quick Connect input border acceptance rework (2026-11-14)

Owner supplied a screenshot identifying the Host field in SSH Quick Connect:
the focused border looks doubled. Outcome: Host, Port and Username retain
one theme-colored edge on focus, without the component's additional outer
ring. Preserve unfocused edges, keyboard focus, typing, layout and theme.
No global theme, dependency, authentication or close-lifecycle change.

Reviewed docs/PROJECT.md, docs/HARNESS.md, docs/templates/work.md,
docs/agents/code-style.md, docs/agents/dependencies.md,
docs/ssh-client-connect.md, shared labelled_field and quick-connect source.
The reference checkout/srcwalk are absent; pinned component registry source
shows focus_bordered gates both the tinted border and extra outer ring.
Disable that combined decoration only on these fields and explicitly retain
a focused edge from theme.ring using the actual input focus handle.
Documentation action: update this owning acceptance record; no connection
contract change. Credential fields and other dialogs are not changed by this
targeted screenshot repair.

Acceptance and verification plan:
- [x] Focused and unfocused field configuration has a single edge with theme
  tokens; owner visual acceptance remains separate.
- [x] Actual focus/typing and field construction regression tests pass.
- [x] Native strict Clippy, full CI and focused session tests pass.
- [x] Preserve predecessor/data, build optimized Release and pass owned GUI smoke.
- [ ] Owner confirms the rebuilt Quick Connect single-edge appearance.

Implemented a local quick_connect_input builder for Host, Port and Username:
the component's combined focus decoration is disabled, and the existing edge
uses theme.ring only when that input owns focus, otherwise theme.input.
No outer container, fixed color or global theme override is added. The shared
duplicate dialog uses these same three fields and inherits the correction.
Added a real GPUI focus/typing regression with style/ring assertions. Initial
test compilation exposed two mutable style-inspection bindings; corrected
before rerunning. Verification completed successfully in background shell
sh_1d98f5f7a0016MvxMttyEqwlnV: focused regression PASS, native session-ui
109 PASS/one existing ignored, strict native app all-target Clippy PASS and
full scripts/ci-local.ps1 PASS. Logs:
.tmp/us0249-input-border-{focused,session,clippy,ci}.log. Optional -Full
cargo-deny was not run; five existing owner-accepted glyph contrast exceptions
remain unchanged. The full gate includes its explicit ignored-test runs.
Optimized Release build PASS (7m18s), background shell
sh_1d9941614001iZobLUNiAsSyhm: target/release/oneterm-native-desktop.exe,
46,372,864 bytes, modified 2026-11-14T18:29:21.7864177+08:00, GUI subsystem 2,
SHA256 71694DC0D77F77A91A9C44739AE85998050C8D6ACED344E32166738E5D12C92E.
Hash-verified predecessor retained at
.tmp/us0249-input-border-predecessor-20261114-182203; EXE-sibling data preserved.
Build/identity evidence: .tmp/us0249-input-border-release-{build.log,
manifest.json,backup.json}. Owned no-argument GUI smoke PASS: PID 30536,
HWND 1249998, exact native workspace title, ownership/title rechecked directly
before targeted WM_CLOSE, exit 0. Evidence:
.tmp/us0249-input-border-release-smoke-20261114-182933.{json,stdout.log,stderr.log}.
Inherited token Elevated, isolated EXE-sibling storage, bundled ConPTY; absent
settings defaulted without writes. Existing window-not-found error appeared
after close, so this is startup/exit proof, not clean-log acceptance. No GPUI
repair is claimed. Owner visual acceptance of the rebuilt fields remains open;
this smoke does not interact with the Quick Connect dialog. Theme, dependency,
permissions and cleanup budgets are unchanged.


- [x] Startup console acceptance rework (2026-11-14): the optimized native
  workspace entry uses the same Windows GUI subsystem gating as production;
  no application-owned console window appears. Debug console and ConPTY
  child shell remain unchanged.

Console rework pre-code review: crates/app/src/bin/{oneterm,oneterm-native-desktop}.rs,
native_workspace.rs and docs/PROJECT.md confirm the native entry lacks the
production release-only Windows subsystem declaration. Apply that exact gate,
without FreeConsole, child-shell hiding hacks, permission or lifecycle changes.
Verification: full ci-local and strict native Clippy, optimized Release PE
subsystem inspection, owned GUI startup/close with process-scoped window
enumeration. Preserve predecessor EXE, isolated data and diagnostics. No new
architecture/schema contract: existing GUI entry contract already applies.
Input style feedback lacks an identified field/screenshot; inspect shared
form controls but do not claim pixel-level diagnosis from source alone.
Console entry verification (2026-11-14): cargo fmt --all -- --check, strict
native app all-target Clippy, bundled scripts/ci-local.ps1 and git diff --check
PASS. Logs: .tmp/us0249-gui-entry-{fmt,clippy,ci}.log. Existing five accepted
glyph exceptions unchanged; optional -Full cargo-deny not run. Optimized
GUI-subsystem Release is building with a hash-checked predecessor backup.
Optimized Release and platform verification PASS (2026-11-14 14:50 +08:00).
PE subsystem is Windows GUI (2); owned no-argument GUI startup PID 22004,
HWND 1055222, exact native workspace title, zero application-owned console
windows at enumeration, targeted ownership-rechecked WM_CLOSE, exit 0.
This sampled window enumeration plus PE inspection proves the entry repair;
it does not claim frame-by-frame absence of all possible child-process windows.
Bundled ConPTY and isolated EXE-sibling data remain selected; inherited token
is Elevated, settings absent/defaulted without writes. No upstream teardown
error appeared in this smoke, but that does not resolve the prior GPUI race.
Release: target/release/oneterm-native-desktop.exe, 46,191,616 bytes, modified
2026-11-14T14:50:43.9353625+08:00, SHA256
578B374C5B6D76968D758CCD313BDCF4DF49EC3451239B9B1CEF894520CCB975.
Hash-checked predecessor preserved under
.tmp/us0249-gui-entry-predecessor-20261114-144312; isolated data unchanged.
Evidence: .tmp/us0249-gui-entry-release-{build.log,manifest.json} and
.tmp/us0249-gui-entry-release-smoke-20261114-145057.{json,stderr.log,stdout.log}.
SFTP remains unimplemented and input-field styling awaits an identified
surface; this executable includes only the console repair over the prior build.

- [x] Screenshot color parity (2026-11-14): owner confirms the rebuilt native
  Release's actual visual appearance is OK after semantic foreground repair.

- [ ] Theme acceptance rework: native terminal foreground/background follow
  the selected UI theme across real shell startup and theme changes, with
  legitimate program OSC colors preserved until reset.
- [ ] Owner acceptance rework (2026-11-14): native terminal right-click and
  keyboard actions expose all four shared Space splits; actual menu actions
  and tab-to-empty-Space moves are exercised, not only direct tree calls.
- [ ] PowerShell 7 actual startup cwd and OSC 7 displayed cwd are checked
  independently against resolved policy; elevated cwd restrictions remain.
- [ ] Remaining local interactions are retested with evidence and gaps explicit.
- [ ] Existing OneTermWorkspace is used, not copied or imitated.
- [ ] Terminal tabs/Spaces host native local and SSH sessions; no legacy engine
  is created to satisfy a missing native interface.
- [ ] Settings/theme and status metrics use native publications without invented
  legacy handles/modes. Unsupported features fail visibly, not silently.
- [ ] No-argument startup works without an elevation-specific test flag.
- [ ] Program effects, credential approval and event draining retain owning policy.
- [ ] Pane/tab/window closure observes native owner and consumer completion;
  cleanup timeout/cancellation cannot report successful teardown.
- [ ] Normal production startup/factory is unchanged.
- [ ] Native focused proofs, full local gate, Release build and real Windows UI
  acceptance are recorded independently.

## Documentation

### Owning Docs Reviewed

- docs/spec-intakes/IN-0022-broadcast-input-channels/low-level-design/registry-and-fan-out.md:
  origin-encoded bytes, committed text, peer-mode paste, interrupt and membership.
- docs/architecture.md: current Space ownership and unchanged factory selection.
- docs/spec-intakes/IN-0018-rebuild-terminal-render-engine/high-level-design.md:
  Space behavior parity and shared rendering contract; updated staging mechanism.

- docs/PROJECT.md: entry points and migration boundary.
- docs/agents/crate-dependency-rules.md: R3, R4, R7-R12 service ownership.
- docs/agents/persistence.md: configuration and privileged-write boundaries.
- high-level-design.md: owner-selected native workspace requirement.
- low-level-design/ghostty-engine.md: current surface/lifecycle/native contract.
- US-0245-ghostty-desktop-launcher.md: UAT mismatch and gate removal.
- docs/decisions/DEC-0023-libghostty-vt-for-the-application.md: no dual engine/fallback.

### Documentation Action

Update required: accept a native workspace LLD before source changes, update the
HLD, service/architecture docs and operator instructions with implemented scope.
No persisted engine selector or schema change is assumed.

### Reconciliation

Service ownership and shared tree/rendering increments are implemented and
documented in the owning renderer design and architecture index. Tagged pane
lifecycle, live typography, acknowledged native color binding and bounded
program-effect dispatch are implemented as described in the workspace LLD.
Factory/startup selection, remaining configuration/input capabilities and
SSH orchestration remain in progress; no desktop acceptance or complete
workspace is claimed.

Native input-channel continuation updates the IN-0022 registry LLD and native
workspace LLD with mixed peer targets, weak pane callbacks, bounded origin/peer
queues, acknowledgment-gated fan-out and enrollment tokens. Production factory,
persisted schemas and native startup selection are unchanged.

## Context

OneTermWorkspace builds terminal panels through PanelRegistry and obtains
commands/metrics from AppServices. AppSessionFactory currently returns only
TerminalSession. TerminalPanel/SpaceTree hold TerminalView entities which assume
the old render handle. NativeLocalSession deliberately implements lifecycle and
SurfaceSession, not that legacy rendering API. Registering a new window or calling
normal run() would not integrate the native engine. The reference checkout and
srcwalk are unavailable; use existing patterns and pinned project-local registry
sources for GPUI APIs, without changing versions.

## Plan

- [ ] Design native session factory/service seam and pane ownership, including SSH.
- [ ] Design shared terminal-pane abstraction for tab/Space operations and metrics.
- [ ] Implement native route in the existing workspace, keeping defaults untouched.
- [ ] Prove teardown/policy/commands; run gate/build and owner desktop acceptance.

## Decisions

DEC-0023, owner UAT amendment 2026-11-13.

## Verification Plan

Screenshot acceptance rework (2026-11-14, owner: old version on right):
PowerShell directory dates/times/sizes and permission punctuation are colored
in legacy but plain in native. Reopen this same unaccepted outcome; Harness
tool discovery remains empty, so no DB transition is claimed. Reviewed PROJECT,
agent style/structure/dependency/error rules, native workspace LLD, IN-0044 HLD,
semantic design, shared overlay/planner and native projection/surface fixtures.
Native RenderInputs leaves semantic scanning disabled; effective RGB projection
also discards default-foreground eligibility used by the shared style merge.
Acceptance: bound surfaces follow live Off/Auto settings and shell profile;
default-colored directory tokens use identical shared class colors; ANSI,
explicit RGB, extended palette and native OSC effective colors remain owned
by their existing rules. Preserve unbound surface policy, isolated storage,
privilege guards, lifecycle deadlines and production selection. No schema or
dependency change. Add failing real-native painted-row and settings regressions
before fixing, then native suites/strict Clippy and full ci-local. Rebuild and
smoke Release independently; owner screenshot parity remains manual acceptance.

Theme acceptance rework (2026-11-14): owner clarified terminal colors differ
from the UI, not a request to import daily configuration. Keep isolated
storage and permission guards unchanged. Reviewed PROJECT, style, dependency,
error/persistence rules, native workspace LLD default-color contract, shared
terminal theme, native palette driver, render projection/cache, and existing
color tests. Add real-shell startup/publication and painted-row assertions
to distinguish theme binding from program/ConPTY overrides or stale defaults.
Fix the demonstrated root cause only; no hardcoded theme or blanket OSC
suppression. Update the LLD if mechanics change; otherwise record no contract
change. Verify native color/paint and PTY suites, native Clippy, full ci-local;
preserve old Release before any rebuild and record exact GUI smoke separately.
This is owning US-0249 acceptance rework, not a new shipped BUG.

Paint investigation isolates a policy mismatch: native projection resolves all
default/ANSI foregrounds to RGB, so the shared planner treats them as explicit
truecolor and skips configured minimum contrast. Add a failing actual-prepaint
regression with identical low-contrast default, ANSI, extended and truecolor
values before repair. Retain native effective RGB and OSC/packed erase colors,
but carry non-exact default/ANSI intent into view-owned paint colors. No public
engine, persistence or security contract changes; reconcile the native workspace
and engine LLDs with the narrowed color-intent gap.

Owner acceptance rework (2026-11-14): reopen this unaccepted workspace
outcome, not a new BUG. Harness discovery again returns no tools, so no DB
reopen is claimed. Reviewed docs/HARNESS.md, docs/templates/work.md, PROJECT,
agent style/dependency/layering rules, native workspace LLD, terminal-split
index and its IN-0018 owner, shared menu/render/action tests, native surface,
pane/factory and core shell trust boundary. Native pane stores SplitContext
but never exposes a context menu; native surface also omits Terminal key
context and unconditionally sends right-click to the terminal. Restore native
UI entry points using existing shared split/duplicate/close mechanics and
native edit controls, without a legacy session. Reference checkout and
srcwalk are unavailable: inspect exact resolved dependency APIs locally.
PowerShell cwd is still under investigation: resolve_shell deliberately
discards configured cwd in elevated windows; do not bypass that boundary.
Documentation action: update native workspace LLD for restored input/menu
wiring and record actual cwd evidence here. No schema or security contract
change is planned. Verify actual right-click/keyboard/menu click paths, four
directions, native move identity, clipboard/Find/IME/resize/focus and close
regressions; native app/view/local/core suites, native Clippy and full local
CI. Preserve Release before rebuild and validate rebuilt exact GUI PID/HWND.
Ordinary-token UAT stays deferred. Cancelled pending-search repeats stay off.

Owner-requested ordinary-token persistence acceptance (2026-11-14): launch
the unchanged Release without arguments using the current account's genuine
UAC linked limited token, only if available. Verify child TokenElevation=0
and exact GUI title/PID before interaction. Never spoof the app elevation
global, change ACLs, import daily configuration or disable persistence guards.
Review DEC-0019, isolated root, UiConfig/theme startup and workspace persistence
owners plus existing token broker. Initially empty EXE-sibling native data
must gain valid settings/layout through the real app. Exercise a harmless
existing view setting if safely targetable, close the exact GUI, relaunch
under the same ordinary token and verify retained settings and normal exit.
Keep immutable snapshots/evidence under .tmp; distinguish file survival from
observed runtime restoration and report any unproved part. No production
implementation or schema change is planned; record evidence in this packet.

Post-close diagnostic continuation (2026-11-14): retain successful Release
smoke and investigate the remaining 'window not found' separately. Review
PROJECT, agent code/dependency/error/persistence rules, native workspace LLD,
window composition and workspace exit hooks. Reference checkout/srcwalk and
Harness tool discovery remain unavailable; inspect exact resolved GPUI 0.3.9
sources without changing them. Add test-only logger provenance/backtrace for
this exact error in the existing real-window child proof before choosing a
repair. Do not suppress errors, alter shutdown budgets, persistence guards or
production startup. Re-run native live proof; if implementation changes are
needed, run native regression/Clippy and full ci-local before completion.
Documentation action: update this packet with attribution and evidence; update
owning LLD only if the shutdown contract or implementation changes. Ordinary
token/full owner UAT remains unverified; no de-elevation or token spoofing.

Follow-up phase evidence: background palette publication reduces close-time
references from three to two but direct quit still fails. Surface release
unconditionally spawns an idle search-clear foreground task owning the remaining
session Arc, even when search was never opened. Move this ordered release clear
to the background as well, with an explicitly Send search future contract; do
not drop/skip an admitted clear or restart cancelled pending-search repeats.
Evidence: .tmp/us0249-live-background-colors.log and cleanup_search_on_release.

Measured direct-quit root cause (2026-11-13): phase logs show PTY/native teardown
finishing in roughly 4-6 ms while pane completion waits on session references.
Palette foreground tasks are scheduled but cannot resume their acknowledgments
under GPUI shutdown; ordinary WM_CLOSE does resume them and completes. Evidence:
.tmp/us0249-live-phase.log and .tmp/us0249-live-palette-phase.log. Make only
the color-publication future explicitly Send and run its admitted wait on the
background executor. Release its session Arc before sending the result to a
foreground, weak-recipient UI observer. Keep coalescing/error reporting and
never cancel admitted work because its surface disappears. No PTY grace or
quit budget changes. Validate real quit without a palette-ready pre-drain.

Direct-quit diagnosis continuation (2026-11-13): inspect pane reference release,
consuming completion, reliable-event completion and owner PTY/native teardown
with phase-specific debug diagnostics. Reproduce the retained live Windows
failure with those diagnostics enabled; no change to deadlines, cancellation,
child-exit grace or accepted-work semantics. Only change a demonstrated root
cause, then re-run live proof, focused regressions, Clippy and the bundled gate.
Reviewed PROJECT, error-policy, code-style, dependency rules/dependencies,
native pane/workspace shutdown, local owner teardown and Windows child drop.
No external VT transport policy change is authorized by this diagnosis.

Real-window close/quit continuation (2026-11-13): add Windows child-process
proofs using the actual GPUI platform application, real workspace/docked native
panels and ConPTY. Use a unique window title and verify the HWND belongs to the
child PID before posting WM_CLOSE to that window only. Exercise admitted native
controls and retain completion observations without retaining session owners.
The multi-panel forced-quit proof must invoke the actual GPUI app-quit observer
within its unchanged 200 ms budget, not a custom extended timeout. Share the
runtime quit-observer installation with the proof to prevent drift. Isolate all
configuration under project .tmp, keep test-only launch logic out of the binary,
and keep failure/cancellation fail-stop. No logout/restart/session-end broadcast.
Review native_workspace, NativeWorkspaceSessions, real panel construction and
resolved GPUI shutdown/window APIs. Re-run focused proofs, native Clippy and
full local gate; preserve existing Release until these acceptance gaps close.

Native immediate-close acceptance rework (2026-11-13): the real workspace
shutdown proof exposed cancellation of an active native output/publication
operation by ordinary close, reported as incomplete lifecycle delivery. Add a
separate orderly native-pane shutdown request, not a reinterpretation of hard
close. The configured local endpoint atomically seals new input admission,
finishes already queued controls/current bounded output publication, reads no
more output once the seal is observed and then performs native/PTY teardown.
Keep hard close and admitted-future cancellation fail-stop unchanged. Retain
independent event draining and the same five-second pane/window budget; forced
app quit still has GPUI's existing budget and cancellation remains failure.
Review local admission/drive/batches/teardown and session completion, pane close
and actual workspace proof. Extend native-workspace LLD and prove sealed
admission, queued-control ordering, blocked publication with explicit draining,
hard-close/cancellation preservation and real immediate workspace close without
a ready-output/quiet delay. Run native regressions/Clippy and full local gate.
This reworks US-0249, not a shipped-behavior BUG or production cutover.

Native startup continuation (2026-11-13): owner approved a persistent
`oneterm-native-data` directory beside the test executable, without reading or
importing production settings/vault/sessions/layout and without automatic SSH
or update checks. Apply DEC-0023's amendment and native-workspace LLD. First
prove once-only absolute configuration-root selection before any reads, with
explicit preparation/late-selection failures and unchanged default paths.
Route actual default-shell/new-tab/empty-Space construction through a tagged
factory result; default factories remain legacy, native-only factory explicitly
rejects legacy local and SSH operations. Reuse registered workspace/feature init
with injected services; preserve token classification and all write guards.
Prove real native factory selection and actual panel dispatch/configuration,
including failed spawn/no fallback. Review app init/window/session_factory,
core config root, panel construction and native pane completion. Reconcile
DEC-0023, persistence and native-workspace docs. Run focused tests, native
regressions/Clippy and full local gate. Native entry replacement and Release
launch remain gated on multi-pane window/app-quit completion proof; do not
publish an unsafe partial route. Harness tool discovery returned no available
tool; Markdown remains the durable pre-code record, no DB state is claimed.

Native cursor-settings continuation: bind live shape/color and blink enable to
the shared CursorConfig only for workspace-bound native surfaces. Reuse the
legacy shape mapping and 500 ms shared timer grid; retain published native
visibility, position and steady-cursor blink policy. Configured shape/color
override native appearance as on legacy panes, but never unhide a program-hidden
cursor. Reset phase on focus/blur or disabled blinking; avoid tick invalidation
for unbound/unfocused, absent/hidden or native-steady cursors. Timer captures a
weak surface, owns no session reference and cancels on release. Review cursor.rs,
Frame::cursor, legacy render/blink and existing native prepaint proofs; update
native-workspace and renderer LLDs. Prove live setting updates, focus/phase,
native steady/hidden behavior, configured color precedence and release cleanup.
Run native regressions/Clippy and full bundled local gate. Gutter/semantic,
startup/SSH/storage/app-quit and desktop acceptance remain separate open gaps.
Pinned reference checkout/srcwalk remain unavailable; reuse compiled project
patterns and local pinned GPUI scheduler sources without dependency changes.

Continuation proof scope: compile the origin queue before extending it; then
exercise actual surface key/IME/clipboard routes with a native-owner encoder
fixture. Cover acknowledgment gating, byte identity, ordered text/key/paste,
interrupt semantics, empty encodings, failed origin/no peers, release/blur,
membership leave/rejoin/move while acknowledgment is pending, bounded origin
admission and surface-release cancellation. Add actual panel menu/tab-wide
native membership proof, reconcile the IN-0022 target contract, and rerun the
native regressions, all-target Clippy and full bundled local gate. Existing
startup/SSH/storage/app-quit gaps remain open; no owner-cancelled UAT is resumed.

Native channel integration increment: generalize registry targets to legacy
sessions or engine-neutral callbacks with weak native pane capture. Keep pane
identity, join ordering, channels A..E and existing legacy write behavior. Bind
native membership explicitly from panel dependencies; leave on exit/closed,
shutdown and release, reject dead joins. Route native peers through a bounded
ordered inbox, never recursively through origin input. Raw encoded bytes,
committed UTF-8 and interrupt bytes use a new acknowledged raw-visible FIFO
operation; paste retains peer-native filtering/bracketing. Share existing
payload budgets and cancellation guards. Peer failures remain visible without
preventing other peers from receiving input. Update actual panel menu/actions
to tagged panes, preserving active-Space and tab-wide semantics. Native origin
input will use ordered surface dispatch, fan out only after origin success,
and keep releases/blur origin-only. Review IN-0022 registry LLD, native
workspace LLD, pane/surface/IME/edit and backend FIFOs; update these contracts.
Prove mixed membership, order/isolation, bounded rejection, teardown without
registry-retained native owners, exact raw bytes and paste mode differences.
Run focused proofs, native regressions/Clippy and bundled full local gate.
No persistence/schema or production selector change; no desktop UAT claimed.

Native input-channel prerequisite increment: preserve IN-0022's origin-encoded
Bytes contract rather than re-encode native keys independently on each peer.
Add an explicit acknowledged native key operation returning the bytes only
after the original transport FIFO accepts them. Run native encoding once,
never write a second time to obtain a receipt, and retain bounded control
queues and fail-stop cancellation. Existing synchronous key admission remains
unchanged for unbound surfaces and releases. The new surface contract rejects
unsupported implementations explicitly, without a legacy fallback. This
increment does not join native panes or broadcast any input yet. Reviewed
IN-0022/low-level-design/registry-and-fan-out.md, state registry, legacy input,
native surface/IME/edit/pane, local and SSH control FIFOs and async pump.
Update native-workspace LLD. Prove actual returned bytes match one accepted
origin write, application/normal cursor modes, empty encodings, transport
failure, queue rejection and cancellation; run native regressions, native
Clippy, full bundled local gate and documentation checks. Channel membership,
mixed-engine peer routing, ordered fan-out and release/lifecycle proofs remain
required before claiming native input channels work. No persistence change.

Native local duplication increment: add an explicit feature-gated
SessionFactory::spawn_native_local service operation whose default rejects
unsupported factories, never invokes the legacy factory. The composition root
implements it with configured NativeLocalSession; normal spawn_local/startup
stay unchanged. Duplicate from the actual source pane with its recorded local
configuration and live cwd, current scrollback/security/logging settings, and
preserve CreatedSession identity through tab/Space/split placement. Reuse native
pane construction and its completion observer for failed placement, including
missing tab containers; never drop an unplaced live native owner as success.
Native SSH duplication remains explicitly rejected before the legacy dialog
until native SSH orchestration exists. Reviewed factory.rs, app session_factory,
panel duplicate/pane/construction, native owner and existing duplicate tests.
Update native-workspace LLD; no persistence change. Prove native factory-only
dispatch, exact config/current policy, identity/placement, source preservation,
failure cleanup and unsupported-factory rejection; run native/default regression,
native Clippy and the full bundled local gate.

Native clear increment: preserve PtySession::clear shell-command semantics by
sending exactly b"clear\r" through the originating local/SSH control FIFO,
then clearing and publishing native selection. This is not paste, RIS, local
history erasure or channel fan-out. Acknowledge transport admission and native
publication, not shell execution. Retain transport errors without retry and
fail-stop cancellation; bound surface edit overlap and detach admitted waits
across recipient release. Reviewed session.rs, ghostty/session.rs, the async
pump, local/SSH control owners and native surface/panel edits. Update the
workspace LLD; no persistence or production factory change. Prove exact bytes
with bracketed paste enabled, selection/history preservation, originating
transport isolation, closed/error/busy behavior and released-recipient waits.
Run focused native tests, native regressions/Clippy and scripts/ci-local.ps1.

Native select-all increment: use the pinned ghostty_terminal_select_all API
inside the confined owner, install its temporary snapshot before any mutation,
and publish through the existing acknowledged selection FIFO. Route the real
panel action to the actual native surface; no synthetic row endpoints, raw
PTY bytes or legacy session. Retain bounded edit admission and detach the
admitted operation across surface release. Prove history/alternate-screen
selection, real-panel dispatch, busy rejection and release completion. Clear,
duplication and input-channel integration remain explicit gaps. Reviewed the
pinned selection.h, owner/bridge/pump and existing edit contracts. Run native
terminal/local/SSH/view/app tests, native Clippy and the bundled full gate.

Program-effect increment: preserve backend policy admission; bound pending
clipboard operations and notifications; apply clipboard effects independently
of paint with live read opt-in and originating-session-only replies. Reuse
existing notification/progress/bell UI. Suppress effects after exit/shutdown,
retain explicit overflow/reply errors and unchanged close budgets. Prove bounded
queues, read denial/live opt-in, reply origin, hidden-pane dispatch and shutdown;
run native terminal/local/SSH/view/app regressions, Clippy and full local gate.
Reviewed security_policy.rs, backend/osc_router.rs and legacy view/render
contracts; no backend admission or persistence policy change is intended.
Harness discovery remains empty; srcwalk and the pinned reference checkout are
unavailable, so reuse the existing compiled notification APIs.

Native color increment: expose the existing acknowledged default-color operation
through SurfaceSession for local and authenticated-channel endpoints. Bound
workspace surfaces derive all 256 default colors from the shared static theme
and settings, never from effective OSC publications. One detached driver waits
for each admitted acknowledgment before sending the latest coalesced intent.
No automatic retry after error; show the error. Surface release cannot cancel
an admitted color wait, and pane completion still waits for its session reference
under the unchanged close deadline. Prove theme/settings updates, full palette,
OSC override/reset, coalescing and close during a pending acknowledgment. Run
native terminal/local/SSH/view/app checks, native Clippy and the full gate.

Live typography increment: explicitly bind the workspace settings entity to
the native surface. Reuse the existing terminal font builder for weight,
ligatures, features and fallbacks; refresh family/size, line height, cell width
and padding from that entity, including hidden panes before their next paint.
An observer invalidates the surface on settings changes. Standalone surfaces
without a binding retain caller-provided font/size. Prove initial and changed
inputs and real panel binding; run native view/app tests, native Clippy and
the bundled full gate. This does not implement backend color synchronization,
cursor blinking, gutters, semantic overlays, program effects or factory selection.

Tagged panel increment: place the native owner in the existing TerminalPanel
through an explicit spec and tagged pane handle, sharing Space layout/mutation.
Verify native identity/focus/title/cwd/close and legacy panel regressions. Migrate
Find/metrics/close consumers without fabricating legacy sessions; unsupported
native actions remain visible. Factories/startup and SSH selection stay unchanged.

Native-pane owner increment: wrap the actual NativeTerminalSurface in an owning
native pane entity. Keep reliable events draining independently of paint and
retain exactly one consuming completion owner after releasing pane/surface
references. Record completion, close errors, cancellation and the unchanged
five-second ordinary-close deadline; never equate a close request with success.
Prove bounded event state, event delivery after pane release, retained-reference
failure and native destruction using a real native engine fixture. This alone
does not select the workspace factory, solve GPUI's app-quit budget, or accept
program effects not yet supported by the owning session contract.

Configured-local increment: propagate scrollback settings into native creation
before output, retaining upstream defaults for existing probes. Match pinned C
option types/values with ABI guards and native readback tests; verify a real
configured local session and explicit pre-spawn rejection of enabled logging.
Do not enable an incomplete factory or legacy logging parser. Native focused
tests/Clippy and the full bundled gate remain required after this increment.

Second increment: remove TerminalView-specific ownership from the existing
Space tree by parameterizing its terminal handle. Keep the legacy default and
panel-supplied focus resolver, preserving normal behavior. Add tagged-handle
proofs for split/fill/move/close/collapse and failed-placement ownership; run
existing Space and panel tests, native/default Clippy and the bundled gate.
This prepares the shared pane boundary, not a second workspace or engine route.

First service increment: implement CreatedSession's tagged ownership and the
object-safe NativeSession completion contract described in native-workspace.md,
plus NativeLocalSession's implementation. Leave the production factory and
existing UI unchanged. Add real local erased-service tests for event ownership,
successful close/completion and retained-reference failure. Run focused native
local tests, native all-target Clippy and the full bundled gate. This increment
alone does not meet workspace UI acceptance or constitute an engine cutover.

Before implementing, enumerate the affected pane/Space/command tests in the LLD.
Prove native selection without legacy construction; theme/resize/input/search,
tab/Space focus and close; local and SSH event/security/lifetime paths. Run
native all-target Clippy/tests and scripts/ci-local.ps1, then build Release.
Real Windows UI/IME acceptance is separate from the test executor. Never restart
the two owner-cancelled search-close repeats automatically or broadcast session
end messages. Retain the fixed shutdown budgets and explicit nonzero failures.

## Evidence and Gaps

### Direct tab-drop split acceptance rework (2026-11-16)

Superseding owner clarification: hover another tab to reveal it, then use the
pointer's position in its terminal area to choose Left/Right/Up/Down. Draw a
translucent half-Space preview during the drag; only release creates the split
and moves the pane. Do not implement fixed-right title drops. Title release
retains reorder. Occupied Space drops now split directionally, superseding the
older inert-content rule for valid cross-tab terminal drags only. Empty Space
drops still fill directly; same-tab occupied and foreign/invalid drags stay inert.
Acceptance adds all four directions, preview-before-release without tree
mutation, preview/drop agreement, retained identity/channel/focus/lifetime,
and actual expanded/collapsed workspace gestures. The initial fixed-title
attempt failed focused tests and is being replaced, not delivered.

Preserve identity/channel/focus/lifetime and remove only an emptied different
source. No fixed-direction title-drop implementation is retained.
Classification: acceptance rework of US-0249, not a new shipped defect.
Reviewed PROJECT, HARNESS, work template, code-style, dependency/error policy,
crate rules, terminal-split, gui-layout, native-workspace LLD, shared title,
Space move logic and strip/actual workspace native gesture tests. No schema,
auth, theme, dependency, session creation or production cutover changes.
Documentation action: amend owning split/UI and native-workspace design.
Plan/acceptance: reproduce unprepared directional drop; reuse guarded
Space move, split only on release; prove strip and expanded/collapsed
actual workspace paths, unchanged hover and invalid-drop guards; run bundled
CI, native suite, strict native Clippy and diff check. Preserve running user
processes and data; rebuild Release only when no user EXE is running.
Harness tool previously unavailable; no database reopen is claimed.

Focused proof: `.tmp/directional-split-focused.log` passes nine native strip
gestures, including all four directions, sole-terminal fallback, prior empty
placement and same-tab empty move. Preview state matches direction and the
tree stays unchanged before release; pane identity/channel/focus/lifetime
are retained. `.tmp/directional-split-rail.log` passes actual expanded and
collapsed OneTermWorkspace native-shell moves; preview keeps rendered
geometry unchanged and release creates a real split and removes the empty
source. Geometry unit proof checks normalized edge choice and half bounds.
Initial full gate caught argument-count lint and a pixel-unit test error;
both corrected, full gate/native/lint rerun pending. No passing full result
is claimed yet. User application was no longer running at Release preparation;
predecessor EXE and full isolated data copied to
`.tmp/directional-predecessor-20261116-013434`, hashes recorded in
`.tmp/directional-release-baseline.json`. No user process was closed.

Release build PASS (`.tmp/directional-release-build.log`):
`target/release/oneterm-native-desktop.exe`, 46,488,064 bytes, SHA256
`45B15423B49BA5C0309CDCCDE81792DD0875A27784211BEFA22A620DE145178C`.
Owned startup/WM_CLOSE smoke PASS, PID 6256, actual Elevated token, exit 0,
ownership rechecked (`.tmp/directional-release-smoke-20261116-014145.json`).
All baseline files remain, no new files; only docks.json/docks.bak changed
through normal layout saving. SSH, vault, trust and settings hashes unchanged.
Known post-close GPUI `window not found` still appears and is not fixed.
Final full gate PASS after replacing an obsolete blocked-cross-tab assertion
with the still-required same-tab occupied no-op guard: bundled ci-local
(`.tmp/directional-split-ci.log`), native suite 1,305 passed / 7 existing
ignored (`.tmp/directional-split-native-tests.log`), strict native all-target
Clippy (`.tmp/directional-split-native-clippy.log`) and git diff --check.
No background verification remains. Physical pointer/preview UAT is not
claimed by the startup smoke; the known post-close diagnostic remains open.

Owner manual acceptance (2026-11-16): the owner reports the drag scenarios
and the suggested associated SSH, input/clipboard, destination-aware SSH,
SFTP and normal exit/reopen checks are basically working. Record this as
user-reported acceptance of the directional drag/split rework and associated
regression scope, not independently instrumented proof of every scenario or
acceptance of the entire native migration. No further behavior change is
requested. The known post-close `window not found` diagnostic remains a
separate open follow-up; no fix or broader cutover is claimed.

### Drag-hover destination reveal acceptance rework (2026-11-15)
Owner clarifies the missing end-to-end gesture: start on selected tab 1,
keep the button down over tab 2 to reveal its content, then continue into
tab 2's empty split and release. Reopen this owning workspace acceptance;
prior tests preselected the destination and did not prove this path.
Outcome/scope: select the hovered terminal tab during an existing terminal
dock drag without ending or replacing that drag. Ordinary hover must not
select; hovering the source is a no-op; retain row-drop reorder, identity,
channel/focus/lifetime and occupied-drop guards. No new drag payload, session
creation, schema/theme/dependency or auth-policy changes.
Reviewed docs/PROJECT.md, docs/HARNESS.md, docs/templates/work.md,
docs/terminal-split.md, docs/gui-layout.md, native-workspace.md LLD,
shared tab title, rail/drop tests and pinned gpui-pre 0.3.9 on_drag_move plus
gpui-base/component 0.8.0 selection/drag APIs. reference/gpui-kit and srcwalk
remain absent. Update owning split/UI/native-workspace docs.
Acceptance/verification plan: actual selected-source native strip gesture
fails before fix, switches to destination with the original drag still live,
then moves into its split without shutdown; normal hover and source hover
do not select another tab. Cover expanded/collapsed rail with actual workspace
skin; run full bundled CI, native suites, strict native Clippy, diff check.
Preserve predecessor/data before any Release rebuild and report manual UAT
honestly. Harness unavailable in previous discovery; manual rework only.

Focused evidence: `.tmp/drag-hover-red.log` reproduces the selected-source
failure before the handler; `.tmp/drag-hover-focused.log` passes all four
native strip gesture cases after it. `.tmp/drag-hover-rail.log` passes the
actual OneTermWorkspace expanded/collapsed rail gesture with real native
local sessions, ordinary/source-hover guards, unchanged order, live drag and
observed shutdown. The added fixture must use `panel_handle`, as production
does: a bare base PanelView has no component title and is not a valid rail
presentation proof. Row prepaint also restricts hover to its content mask so
offscreen/clipped rows cannot select through the rail's frame. Reviewed the
pinned GPUI hitbox/drag dispatch and ElementExt prepaint implementations.
Owning gui-layout.md, terminal-split.md and native-workspace LLD reconciled.
Full gate/native checks pending. Release replacement is blocked by the user's
running native desktop (PID 28824 at inspection); do not close or terminate it.
The running/delivered Release does not yet contain this hover rework.

The full bundled CI, app-inclusive native suite and strict native all-target
Clippy passed (`.tmp/drag-hover-ci-final.log`,
`.tmp/drag-hover-native-tests-final.log`,
`.tmp/drag-hover-native-clippy-final.log`), as did diff whitespace checks.
The native suite reports 528 terminal-view tests and the actual workspace rail
proof passes. PID 28824 remains running; no Release replacement was attempted.

### Split-tab acceptance rework (2026-11-15)
Native strip gesture reproduces a silent no-op when the source's active Space
is empty after Split, although it still owns one live terminal. Same-tab drops
into the freshly created placeholder have the same cause. Preserve active
terminal selection; when it is empty move only an unambiguous sole terminal.
Multiple inactive terminals remain a no-op rather than choosing a session.
Baseline plain-source native gesture transferred identity/channel/focus correctly.
Red evidence: .tmp/split-drag-red.log.

Repair proof: native gesture regressions pass for a plain source, an empty
active source after splitting, and dragging the selected tab into its own
new empty Space. They assert actual dock drag dispatch, occupied/missing
target no-op, exact pane identity, channel membership, focus, source removal
without premature shutdown, and final consuming completion. Tree proof keeps
multiple-inactive-session selection a no-op. Reviewed/updated owning
docs/terminal-split.md; no persistence or dependency change.
Full scripts/ci-local.ps1 PASS; native app-inclusive regression 1,297 PASS
with 7 existing ignored, strict native all-target Clippy and git diff --check
PASS. Evidence: .tmp/split-{drag-focused,menu-focused,rework-ci,
rework-native-tests,rework-native-clippy}.log. Five existing owner-approved
theme glyph exceptions remain; optional cargo-deny -Full not run. Physical
drag UAT is separate from automated GPUI gesture proof. Release rebuild and
owned startup/close smoke are recorded separately below.

Release delivery (2026-11-15): optimized native Release rebuilt successfully
in 5m42s; target/release/oneterm-native-desktop.exe is 46,448,128 bytes,
SHA256 BCFF049563BF2FB9E9776E6922BE7D93A20A9E5700B0B1BA1C21515268E06E65.
User normally closed the previous application; no user process was terminated.
Predecessor preserved under .tmp/split-predecessor-20261115-133859, verified
SHA256 217688E1BABC848CCEC08A06B5549B25F99022D0F1FB25C840ACC151799A79FE.
No-argument Release smoke passed with real Elevated token, PID 16228, HWND
3478966, ownership rechecked before WM_CLOSE, exit 0. Existing SSH entries,
vault, known_hosts, terminal/UI/update settings and their backups are byte-for-
byte unchanged. Only docks.json/docks.bak changed through normal layout save;
no files added, deleted, restored or migrated. Evidence:
.tmp/split-release-{build.log,baseline.json,manifest.json},
.tmp/split-release-smoke-20261115-134451.{json,stderr.log,stdout.log}.
Known upstream ERROR window not found appears again despite exit 0 and is
not fixed here. Physical drag/menu and external-host UAT remain explicit;
automated gestures and startup smoke do not claim that acceptance. No commit,
push, publication or production-engine cutover.
Owner reports tabs cannot move into split Spaces in the native Release. Reopen
this owning workspace acceptance. Inspect actual dock drag dispatch; preserve
live native pane/session, input membership, focus and lifetime when moving into
an empty Space. Occupied/missing destinations must be no-ops; source-tab removal
must never shut down the moved pane. Reviewed terminal-split.md, Space tree,
transfer code and native workspace contract. Reference checkout/srcwalk absent;
inspect pinned registry gpui-base/component 0.8.0. No schema/theme/dependency
changes. Add native drag regression, full local CI/native suites/strict Clippy
and rebuild Release; report physical drag acceptance honestly. Harness unavailable;
no database reopen/proof claimed.

Owner prioritization (2026-11-14): defer the known GPUI shutdown-log bug
because its observed impact is low. Stop active repair/investigation for now;
keep it open and distinguish successful process exit from clean-log acceptance.
This is not a fix or authorization for dependency changes, suppression or
shutdown delays. Continue with remaining native-workspace parity and acceptance
items; ordinary-token persistence remains separately deferred.


GPUI investigation continuation (2026-11-14, after shared-focus delivery):
owner selected read-only investigation with no dependency changes or log
suppression. Existing twelve-child bare-window diagnostic re-run PASS for
termination/one-close observation; missing-window appeared in 5/6 default and
5/6 logical children, invalid-HWND in 6/6 default and 5/6 logical children.
Source review distinguishes queued activation, late VSync registry removal
and default-close duplicate destruction. No supported application-side
root-cause repair was identified. Detailed evidence and repair requirements:
evidence/bare-gpui-close.md, investigation continuation. Actual repair remains
open pending separate dependency/upstream authorization. Current shared-focus
Release/data unchanged; SHA256
9508A08B266DC3C02B8CF40732CC404DFEF81BD5A40088F473A4D3D9C45AA65A.


Published-framework follow-up (2026-11-14, after US-0252 Release): reviewed
the retained close backtrace and resolved 0.3.9 source before considering an
application change. The configured reference/gpui-kit tree is absent and
srcwalk is unavailable in this session; registry sources and published crate
archives are the explicit fallback, not a restored or fabricated reference.
Downloaded gpui-pre and gpui-pre-windows 0.3.12 into the project-local
.tmp/gpui-close-review-0312 research directory only. No Cargo declaration,
lockfile, registry source, running instance or Release was changed.

Source comparison is NOT an executed upgrade proof. In published Windows
0.3.12, window.rs:613 still drops visibility callbacks but leaves the active
status callback installed; events.rs:798 still schedules that callback through
a retained WindowsWindowInner. events.rs:325 still posts close-list removal
after WM_DESTROY, while platform.rs:478 redraws tracked HWNDs and :1292 removes
them only in close_one_window. These paths do not establish a repair for the
two previously attributed races. The current gpui-component 0.8.0 registry
manifest pins gpui-pre exactly =0.3.9, so independently updating only the
platform family is not a compatible integration plan.

The owner continuation does not override the no-UI-patch contract. Do not
substitute app-quit ordering, delays, native destruction calls or log filters.
Next dependency-owned action is an upstream minimal reproduction/repair and
reviewed compatible family integration; publishing an issue or changing the
dependency policy needs separate authorization. Existing US-0252 Release
F33F8FFFA786F96449DD986ED469BC6539BE7249E8554DE92FFCDCC02E915FB3 remains
the verified artifact; the GPUI error is unresolved. This increment changes
documentation/research only, so no new implementation gate or Release is
claimed. External-server UAT, ordinary-token persistence and input styling
remain separately open; theme and shutdown budgets are unchanged.

Post-close ownership follow-up (2026-11-14, after owner visual acceptance):
bounded read-only inspection of the exact resolved GPUI 0.3.9 sources confirms
two framework-owned lifetime gaps consistent with the retained diagnostic
stack; this is source attribution, not a newly executed runtime proof.
`gpui-pre/src/app.rs` update_window_erased removes the GPUI window lookup
before the Windows platform window is dropped. `gpui-pre-windows/src/window.rs`
WindowsWindow::drop clears visibility callbacks, hides the HWND and schedules
DestroyWindow after dialog-owner idle, but leaves active-status callbacks
installed. `src/events.rs` handle_activate_msg schedules a foreground task
retaining WindowsWindowInner and later invokes active_status_change; the
GPUI callback still attempts WindowHandle::update and logs a missing window.
Separately, handle_destroy_msg posts WM_GPUI_CLOSE_ONE_WINDOW asynchronously;
only its later platform handler removes the HWND from raw_window_handles.
The VSync thread can therefore still call RedrawWindow on that destroyed HWND.
Its stop flag is checked before the iteration, not synchronized with native
window destruction. WM_NCDESTROY clears userdata/dialog ownership, not either
of these queues.

The platform_window field is crate-private; no supported application seam
was found to detach these callbacks or synchronize the private VSync list.
Changing Root-release quit ordering does not itself repair either lifetime.
No speculative quit substitution, sleep, native DestroyWindow, synthetic
platform message, error filtering, registry-source edit or dependency change
is made. The dependency policy explicitly disallows UI-layer patches; an
upstream repair and separately reviewed compatible release integration are
the next dependency-owned actions, not authorized by this investigation.
Native shutdown deadlines, persistence/elevation guards and accepted visuals
remain unchanged. Reviewed PROJECT, HARNESS, dependencies, owning native LLD,
shared window/workspace release and native close hooks; no owning-contract
change is needed for this documented boundary. No implementation/tests changed
and no new test/CI or Release build is claimed. Existing Release and its
earlier verification remain unchanged; this diagnostic is still unresolved.

Screenshot semantic acceptance rework: two regressions first failed against
the actual native engine: live settings left scanning disabled, and painted
directory dates/times/numbers differed from the legacy render path. The initial
paint test had an invalid global span offset; after correcting per-text-run
sampling, it failed on the intended color comparison. Evidence:
`.tmp/us0249-semantic-before{,-corrected}.log`.
Native projection now retains default foreground provenance as well as
exactness, both in row equality; view-owned DefaultRgb preserves effective RGB
and shared semantic eligibility. Bound surfaces apply live mode and the shared
shell-profile mapping; unbound surfaces remain disabled. The two regressions
and equal-RGB provenance/cache test pass in
`.tmp/us0249-semantic-{after,projection}.log`. Additional regression assertions
cover Off/on plan invalidation and preservation of explicit ANSI/RGB/extended
palette foregrounds. Full native regression, native app strict all-target
Clippy and the entire scripts/ci-local.ps1 gate passed (background run
sh_1d85161860011tN0G6UgMk3q0E). Evidence:
`.tmp/us0249-semantic-{native,clippy,ci}.log`. Existing ignored tests and the
five owner-accepted glyph contrast exceptions remain unchanged; optional
cargo-deny was not run. Optimized Release rebuilt successfully in 7m 34s
on 2026-11-14 at 12:46:03 +08:00, 45,740,544 bytes. SHA256:
`24762453BA75F6FEB23E29BA4F01393450D1ACF9B4EBE5F04017A46E1A735562`.
Build evidence: `.tmp/us0249-semantic-release-build.log`; previous EXE
preserved and hash-verified in `.tmp/us0249-pre-semantic-release-*`.
No-argument real GUI smoke passed: PID 30564, HWND 1184216, exact title
`OneTerm Native Workspace (test)`, PID-owned WM_CLOSE and exit 0. Evidence:
`.tmp/us0249-semantic-release-smoke-20261114-124621.{json,stdout.log,stderr.log}`.
Storage remained EXE-sibling and the inherited token Elevated; bundled ConPTY
was used. An invalid-HWND post-close platform diagnostic persists and is not
claimed fixed. Owner confirmed actual visual appearance is OK on 2026-11-14
after trying this Release; the reported screenshot/theme color mismatch is
accepted. This confirmation does not independently prove every theme switch
or program OSC scenario. Ordinary-token acceptance remains unproved.
Native OSC cell roles/exit-code anchors remain unavailable;
the shared scanner uses its textual fallback, without a second terminal parser.

Latest Release refresh (2026-11-14): optimized native desktop build passed
after the owner closed the old EXE that blocked replacement. It includes the
theme contrast-intent repair and US-0250 executable setting. Build evidence:
`.tmp/us0250-release-build{,-retry}.log`. Executable:
`target/release/oneterm-native-desktop.exe`, 45,741,056 bytes, SHA256
`EB8A164C095E10422F3D7B133909C2782EA0B3434E89EFEFF104761A10C7F653`.
The predecessor remains in `.tmp/us0250-pre-rebuild-*`.
No-argument real workspace startup and PID-owned WM_CLOSE passed: PID 7428,
HWND 200896, title `OneTerm Native Workspace (test)`, exit 0. Evidence:
`.tmp/us0250-release-smoke-20261114-121610.{json,stdout.log,stderr.log}`.
Storage remained EXE-sibling `oneterm-native-data`; bundled ConPTY was used.
The inherited token was Elevated, not ordinary. Post-close GPUI
`window not found` and invalid-HWND errors persist; they are not claimed fixed.
Physical theme/drag/drop/IME acceptance, ordinary-token persistence, native SSH
orchestration and remaining parity gaps are not satisfied by this smoke.
Earlier no-Release statements below describe prior increments.

Theme verification continuation: native terminal/view/local suites, native app
all-target Clippy and scripts/ci-local.ps1 all passed; logs are
`.tmp/us0249-theme-{native-tests,native-clippy,ci}.log`. Theme-switch/OSC-reset
paint and equal-RGB intent cache regressions are included. The owner then
requested US-0250's PowerShell 7 executable setting; Release rebuild and physical
theme UAT remain pending and are not implied by the automated gate.

Theme acceptance rework (2026-11-14, verification in progress): the actual GPUI
prepaint regression fails before repair with native default foreground lightness
0.145098 instead of shared contrast-adjusted 0.535098; evidence:
`.tmp/us0249-theme-paint-before.log`. The surgical repair keeps effective RGB
authoritative and retains default/ANSI-vs-exact foreground intent in native
projected rows and exact cache keys. The view uses a non-exact RGB tag only for
shared contrast; truecolor and extended palette remain unchanged. The focused
paint regression and real Cmd/PowerShell/Pwsh startup/result-row assertions pass
after repair (`.tmp/us0249-theme-paint-after.log`,
`.tmp/us0249-theme-startup.log`). Temporary diagnostic printing was removed.
Extended theme-switch/OSC-reset/cache tests, native suites/Clippy and the full
local gate are running; no full-gate or desktop visual pass is claimed yet.
The current Release is preserved in `.tmp/us0249-pre-theme-release-*` with
SHA256 `69C19182B1F69D5C996C362C55994614548FB7E90F86BC6019C7CE27D5B4F7EC`.
It does not contain this repair until rebuilt. Owner visual confirmation remains
required; this proven contrast-policy defect is not proof of every perceived
theme mismatch. Owning workspace and native engine LLDs now describe the policy;
production defaults, isolated configuration and OSC controls remain unchanged.

Native default-color binding is implemented for explicit workspace surfaces.
SurfaceSession forwards acknowledged full-palette updates through local and
authenticated-channel FIFOs. Static theme/settings defaults are separate from
native effective OSC colors. The single detached driver coalesces unadmitted
intent, awaits admitted publication through surface release, and reports failure
once without automatic retry. Renderer selection/contrast settings follow the
shared static theme; cell/background/cursor colors retain native authority.

Five real-engine surface proofs pass: settings coalescing/full 256-color palette
with OSC override/reset, active theme updates without output, acknowledgment
after surface release, visible failure without retry, and pane close waiting
for a pending acknowledgment. Real local and SSH color regression proofs now
exercise the erased SurfaceSession interface, not only inherent methods.

Post-color verification passed: native terminal 350; local 67 with 3 existing
ignored; SSH 155; terminal-view 485 with 3 existing ignored; no failures. Native
app tests, native all-target Clippy and scripts/ci-local.ps1 passed. Evidence:
.tmp/us0249-native-colors-proof.log and
.tmp/us0249-native-colors-{terminal,local,ssh,view,app,clippy,ci}.log.
git diff --check passed. Optional -Full cargo-deny was not run.
No Release rebuild, actual-workspace startup or Windows owner UAT is claimed.
Cursor blink/gutter/semantic settings, program effects, duplication/channels,
factory/startup and SSH orchestration, storage scope and app-quit coordination
remain incomplete. Production defaults and shutdown budgets are unchanged.

Live typography binding is implemented for explicit native workspace panes.
The actual surface observes the existing settings entity and refreshes font
family/size/weight/features/fallbacks/ligatures, line height, cell width and
padding before rendering. Standalone unbound surfaces keep caller inputs.
Three added typography proofs cover initial/live inputs without paint/output, unbound
inputs and pane binding with successful close. No persistence or production
factory/startup change. Backend colors were subsequently implemented above;
cursor blink, gutters, semantic settings, program effects and other workspace
acceptance gaps remain open.

Post-typography verification passed: native terminal-view 480 passed, 0 failed,
3 existing ignored; native app tests and native app all-target Clippy passed;
scripts/ci-local.ps1 passed. Evidence:
.tmp/us0249-native-typography-proof.log,
.tmp/us0249-native-typography-view.log,
.tmp/us0249-native-typography-app.log,
.tmp/us0249-native-typography-clippy.log and
.tmp/us0249-native-typography-ci.log. Optional -Full cargo-deny was not run.
No new Release build or Windows owner UAT is claimed.

Latest configured regression completed successfully: native local 67 passed,
3 existing ignored; native app tests passed; scripts/ci-local.ps1 passed.
Evidence: .tmp/us0249-configured-full-local.log, configured-full-app.log and
configured-full-ci.log. That run predates final tagged-panel changes.

NativeTerminalPane now owns the actual native surface, independent bounded
event reduction and detached reliable draining through close. A surviving
NativePaneCompletion distinguishes pending/success/error; unpolled observer
cancellation fails and the ordinary close deadline remains five seconds.
TerminalPanel now uses tagged actual pane entities in its existing Space tree;
explicit native PanelSpec selection does not construct a legacy TerminalSession.
Focus, subscriptions, title/cwd/status/local metrics, movement and close dispatch
through the branch. Workspace Find reaches native search. Unsupported native
duplication/input-channel/select-all/clear and program effects stay explicit.
Production factories/startup remain unchanged; normal default-shell commands
still create legacy sessions and must not be used as a native test entry yet.

Seven focused native owner/panel proofs pass, including a real native engine
with a bounded event channel and the actual TerminalPanel split/move/cwd/close
path. All 77 prior panel regressions passed after tagged migration. Native app
all-target Clippy passes. Evidence: .tmp/us0249-native-panel-proof.log,
.tmp/us0249-tagged-panel-tests.log and .tmp/us0249-native-panel-clippy.log.
Final post-panel verification completed successfully in
sh_1d2d639cb001mPZhVAru7vxE6K: native terminal-view 477 passed, 0 failed,
3 existing ignored; native app tests passed; scripts/ci-local.ps1 passed.
Evidence: .tmp/us0249-native-panel-full-view.log,
.tmp/us0249-native-panel-full-app.log and .tmp/us0249-native-panel-full-ci.log.
Optional -Full cargo-deny was not run. This verifies the implemented panel
increment, not factory/startup selection, complete workspace or desktop UAT.
No Release rebuild or owner UAT has been performed for these changes.

The shared-tree background run completed: 470 native terminal-view tests passed,
3 existing tests ignored, and scripts/ci-local.ps1 passed. Evidence:
.tmp/us0249-shared-tree-native-view.log and .tmp/us0249-shared-tree-ci.log.
It began before later configured-local/probe edits and is not final proof of
those edits. The repaired pinned C ABI probe plus 350 actual native terminal
tests passed in .tmp/us0249-configured-abi-probe.log; the configured terminal
and local focused pairs and native app all-target Clippy passed in the matching
.tmp/us0249-configured-*-tests.log and configured-native-clippy.log files.
Fresh native local/app regressions plus the full gate completed successfully
under sh_1d2cb0ee7001cjcA3fb81RSg3z as recorded above.

Configured-local verification found the historical rustc-only ownership probe
stale: selection now depends on the real published viewport and Gesture variant,
neither present in its isolated module root. The C ABI probe passes including new
scrollback guards. Repair this verification path by running the actual terminal
crate's native ownership/boundary tests after the C probe, not a fake viewport
stub. Remove the obsolete standalone ownership.rs fixture after moving its
selection case into the actual crate tests. Preserve the C ABI gate and explicit
test failures. This is verification rework within this native configuration
increment, not production behavior or a claim of full workspace acceptance.

2026-11-13 continuation: removed only regenerated target/debug/incremental
(about 0.18 GiB) after checking no build/app process was running and rejecting
reparse points. Preserved sources/uncommitted changes, evidence, native library,
Release executable and the dependency artifacts needed for continued builds.
No broad target/dependency or application-data deletion was performed.

The typed CreatedSession / NativeSession ownership seam is implemented; two
real local erased-service tests pass, native local tests pass (65, 3 existing
ignored) and native app all-target Clippy passes. Evidence is retained in
.tmp/us0249-session-seam-tests.log, .tmp/us0249-native-local-tests.log and
.tmp/us0249-native-clippy.log. The previously started terminal/app/full gate
verification completed successfully: native terminal 347 passed, native app
tests passed and the bundled scripts/ci-local.ps1 gate passed in
.tmp/us0249-native-terminal-tests.log, .tmp/us0249-native-app-tests.log and
.tmp/us0249-session-seam-ci.log. Optional -Full cargo-deny was not run.

The shared Space tree now accepts a cloneable pane handle; its renderer accepts
render entities without a TerminalView dependency. Existing default type and
panel behavior remain unchanged. Tagged-handle split/move/close/fill/focus
proofs pass (18 Space tests); all 77 panel regressions pass. A real native
surface moves through this same tree without a legacy session, and the shared
renderer accepts its entity type (2 focused proofs). Native app all-target
Clippy passes after these changes. Evidence: .tmp/us0249-space-tree-tests.log,
.tmp/us0249-panel-tree-tests.log, .tmp/us0249-shared-render-tests.log and
.tmp/us0249-shared-render-clippy.log. This proves ownership/layout seams, not
an end-to-end native workspace or visual acceptance. Full post-tree regression
completed in the shared-tree run recorded above, but does not cover later edits.
Tagged panel consumers are now connected; factory/startup selection remains
unconnected. The prior standalone Release binary
is not this deliverable. Ordinary-token and graceful session-end gaps remain.

## Handoff

Rework verification (2026-11-14): full scripts/ci-local.ps1 PASS, log
.tmp/us0249-uat-ci.log; optional -Full cargo-deny not run. Existing five
owner-accepted contrast exceptions unchanged. Optimized native workspace
Release rebuilt successfully (.tmp/us0249-uat-release-build.log). Prior
verified workspace EXE preserved byte-for-byte in
.tmp/us0249-pre-menu-release-20261114-020112/oneterm-native-desktop.exe.
New target/release/oneterm-native-desktop.exe SHA256:
69C19182B1F69D5C996C362C55994614548FB7E90F86BC6019C7CE27D5B4F7EC.
No-argument actual GUI smoke PASS: PID 17988, HWND 3148976, exact title
OneTerm Native Workspace (test), bundled ConPTY, PID-checked WM_CLOSE,
exit 0. Evidence .tmp/us0249-uat-release-20261114-112115.{json,stdout.log,stderr.log}.
Inherited token Elevated; isolated storage remains EXE-sibling. Logs contain
cursor-position access-denied platform diagnostics; these are not repaired
or hidden, and the earlier post-close GPUI race remains open independently.
Do not interpret smoke or automated interaction regressions as complete
physical desktop UAT. Nonstandard PowerShell 7 executable trust resolution
remains blocked by DEC-0019, not fixed by weakening PATH/cwd guards. Precise
owner-expected executable/startup directory needs confirmation before any
consequential policy change. Ordinary-token UAT remains explicitly deferred.

Owner acceptance rework progress (2026-11-14): restored native pane context
menu (four shared splits, duplicate destinations, native edit controls, close
Space/tab), Terminal key context, and settings-controlled right-button menu
ownership. Native menu row clicks and all four shortcut actions PASS; the
same proof exercises the actual move_tab_into routine and checks native
identity and emptied source. This is not yet physical drag/drop desktop UAT.
PowerShell investigation distinguishes two issues: the installed executable
is D:/Program/PowerShell/7/pwsh.exe; C:/Program Files/PowerShell/7/pwsh.exe
does not exist. The existing DEC-0019 elevated resolver intentionally trusts
only Program Files and rejects the nonstandard installation, rather than
falling back to PATH. No security-policy change is authorized or made.
Separately, native real-PTY tests for PowerShell and available pwsh PASS:
configured initial cwd, emitted OSC 7 cwd and Get-Location agree. This uses
normal resolver state in the test process, not a genuine ordinary-token
desktop claim. An elevated desktop intentionally ignores configured cwd and
uses home. Owner's precise expected path remains unspecified.
Evidence: .tmp/us0249-uat-{menu-test,cwd-test,native-tests,native-clippy}.log.
Native view 508 PASS / 3 existing ignored, local 75 PASS / 3 existing ignored,
app 42 PASS; strict native all-target Clippy PASS. Suites include existing
clipboard, ordinary Find, IME Unicode/commit, pointer/focus, resize, tabs and
close proofs; they do not prove physical IME candidate UI or mouse drag on
the owner's desktop. No dedicated pending-search repetition command added.
Full gate and rebuilt Release verification follow; no full UAT acceptance.

Owner deferral (2026-11-14): ordinary-token startup/persistence acceptance is
explicitly skipped for now, not passed. Read-only environment inspection found
EnableLUA=0, current process elevated and no usable UAC linked limited token;
the owner reports no non-administrator account. No system settings, account
or reboot changes are authorized. Retain the earlier blocked evidence.
Recommended next independent slice: real-workspace local desktop UAT for
new tabs, split Spaces, focus/input, Find, clipboard, resize and orderly
pane/tab/window closure; record only observed behavior. Do not restart the
owner-cancelled pending-search repeats. Then scope native SSH orchestration
separately through existing host-key/auth/jump/tunnel policy, with no legacy
fallback. Gutter/semantic parity and active-operation shutdown remain open;
no production cutover or full parity acceptance follows from this deferral.

Ordinary-token acceptance attempt (2026-11-14 01:30 local): BLOCKED before
application startup. Existing broker's genuine current-account linked-token
CreateProcessWithTokenW launch of the unchanged Release, with no arguments,
failed: 'A specified logon session does not exist. It may already have been
terminated.' No successful child PID or window was returned, so no ordinary
startup or persistence success is claimed. Both existing Explorer processes
in session 1 (PIDs 6772 and 19064) queried TokenElevation=true, so neither is
an ordinary desktop launch route. EXE-sibling oneterm-native-data remains empty.
No ACL changes, alternate credentials, token/global spoofing, configuration
seeding, daily configuration import, guard bypass or runtime changes were
performed. The empty evidence directory from the failed attempt is retained
as .tmp/us0249-ordinary-persistence-20261114-013053; failure/environment
details are recorded there in blocked.json. Next requires a genuinely
non-administrator Windows session or terminal supplied by the owner. From
that terminal launch target/release/oneterm-native-desktop.exe with no
arguments, change an innocuous UI setting, close normally, relaunch and
check restoration. App-created JSON alone proves writing, not runtime
restoration; both remain unverified until that route is available.

Post-close diagnostic attributed (2026-11-14): the existing real-platform
three-pane WM_CLOSE/direct-quit proof still passes with test-only logger
file/line and exact-error backtrace capture. Retained evidence:
.tmp/us0249-close-diagnostic-stack.log. The 'window not found' emission is
resolved gpui-pre 0.3.9 src/window.rs:2017: the platform active-status
callback updates a WindowHandle after removal. The captured stack enters
Windows platform event/task dispatch; optimized dependency symbols are not
treated as precise frame-by-frame source attribution. Additional VSync
RedrawWindow invalid-HWND diagnostics appear at gpui-pre-windows 0.3.9
src/platform.rs:493 in this run. These are framework teardown diagnostics,
not evidence of native owner/event failure, but remain unresolved.
No runtime workaround, log suppression, dependency patch/version change,
token change or timeout adjustment is made. Only the existing Windows live
test's logger gains provenance for this exact error. The Release remains
byte-for-byte unchanged; ordinary-token persistence/full UAT remains open.
Reviewed owning native workspace LLD and error/persistence contracts: no
contract change is needed for test-only attribution. Re-run native app suite,
native all-target Clippy and bundled ci-local for this test edit.
Verification completed: native app suite PASS (42 tests), native all-target
Clippy PASS and bundled ci-local PASS. Logs:
.tmp/us0249-close-diagnostic-{app,clippy,ci}.log. Optional -Full cargo-deny
not run; the five owner-accepted glyph exceptions remain unchanged. This
diagnostic-only change does not require replacing the verified Release EXE
and does not claim the framework callback race has been repaired.

No-argument Release smoke (2026-11-14 00:57 local): PASS for actual GUI
startup and targeted ordinary window close, not full owner UAT. PID 18180
exposed both ConsoleWindowClass (EXE-path title) and Zed::Window (exact title
OneTerm Native Workspace (test)). Enumerated only this process, verified GUI
HWND 1247902 ownership immediately before posting WM_CLOSE to that HWND only.
Exit code 0; native owner completion 10.54 ms and reliable-event completion
10.70 ms, lifecycle_delivered true. Packaged Release used bundled ConPTY,
unlike earlier live tests using system ConPTY. Evidence:
.tmp/us0249-release-hwnd-smoke-20261114-005703.evidence.log and matching
.stderr.log/.stdout.log. No source or binary changes were needed.
The preceding smoke remains FAILED evidence:
.tmp/us0249-release-smoke-20261114-005552.stderr.log, PID 22736, EXE-path
MainWindowHandle title, exit -1073741510 with no cleanup observation. The new
enumeration confirms why MainWindowHandle is not a safe GUI selector: this
Release also owns an EXE-path-titled console. Do not reclassify that run PASS.
Both launches inherited Elevated. Isolated EXE-sibling storage was selected;
logs explicitly show missing settings defaulted with no writes and layout
not read. Ordinary-token persistence is still unproved; do not bypass guards
or import daily data. A save_layout_on_exit intent log and a post-close
'window not found' error remain visible; successful exit is not evidence
that either diagnostic is resolved. Native SSH, gutter/semantic, active
operation shutdown and full Windows owner UAT remain open.

Actual-workspace Release built (2026-11-14 00:00:32 local):
`cargo build -p oneterm-app --bin oneterm-native-desktop --features native-desktop --release`
passed using the existing optimized Release profile (8m58s). Artifact:
target/release/oneterm-native-desktop.exe, 45,706,240 bytes, SHA256
50B6FAA4D6E50998DDC8F074F08EA3891D226860C0EE11A0377BE457A095B949.
The prior standalone EXE remains preserved byte-for-byte under
.tmp/us0249-pre-workspace-release-20261113-235133/oneterm-native-desktop.exe
(25,139,712 bytes, SHA256
50BD6EDC91BC2336EBEABF069EEFB2983E5088C88884F638ADC8AE098345389B).
Evidence: .tmp/us0249-workspace-release-build.log and
.tmp/us0249-release-preserved.log; git diff --check passed after build.
No build-cache/source/native-library/settings cleanup performed. This is the
actual workspace composition root, no-argument native-only local startup with
isolated persistent EXE-sibling oneterm-native-data, not production cutover.
The new Release has not been launched or owner-accepted. Ordinary-token/full
Windows UAT, native SSH, gutter/semantic and active-operation shutdown coverage
remain open. Earlier historical no-Release statements below are superseded.

Direct-quit repair verified (2026-11-13; supersedes earlier failure below):
foreground-owned palette publication and idle release search-clear prevented
session release under GPUI shutdown. Send color/search future contracts now
allow those waits on the background executor; weak foreground palette result
delivery owns no session. No deadline/child grace/cancellation policy changes.
Actual targeted WM_CLOSE and three-pane direct quit pass, with slowest observed
pane owner/event completion about 32 ms in .tmp/us0249-live-background-release.log,
inside GPUI's unchanged 200 ms budget. This is measured proof on this machine,
not a universal guarantee or active search/edit/channel-wait acceptance.
Full native regressions PASS terminal358/local74 (3 existing ignored)/SSH160/
view507 (3 existing ignored)/app42 including live platform proof; native
all-target Clippy and bundled ci-local PASS. Evidence:
.tmp/us0249-background-release-{terminal,local,ssh,view,app,clippy,ci}.log.
Optional -Full cargo-deny not run; five accepted glyph exceptions unchanged.
Next: build actual-workspace native-desktop using the existing Release profile.
Preserve old standalone EXE/hash under project .tmp before Cargo replaces its
output; preserve caches, source, native library, settings and all evidence.
Record new hash/size/build log. Ordinary-token/full Windows UAT, native SSH,
gutter/semantic remain open. No production cutover/distribution acceptance.
Cancelled pending-search repeats remain cancelled.

Real-window continuation (2026-11-13): added Windows-only process-isolated
actual-platform/actual-workspace tests with three real docked native panes and
accepted key controls. The exact HWND title and child PID are checked before
posting WM_CLOSE; no system broadcasts. This exposed an AsyncApp re-entry panic
during GPUI shutdown (RefCell already borrowed). The shared runtime quit observer
now captures only completion handles/sticky failure before awaiting, never
re-enters App under shutdown's borrow. Targeted WM_CLOSE now passes, exit 0.
Three-pane direct app quit still fails within GPUI's unchanged 200 ms budget,
exit 1 via QuitGuard; it also fails after explicit shell-prompt readiness, not
only during startup. This is an unresolved acceptance failure, not successful
quit coverage. Current failing regression is deliberately retained in
native_workspace_live_tests.rs; evidence .tmp/us0249-live-close.log. DXGI debug
interface unavailable and post-close 'window not found' logs also appear in
the passing WM_CLOSE child; these are not claimed resolved. No Release build
or trial launch. Next: locate pending completion/remaining references versus
PTY destruction latency in the direct-quit path, without raising the budget,
pre-draining away the scenario or suppressing failure. Native all-target Clippy
and the bundled ci-local gate passed for these edits in
sh_1d583f4f8001Mc97217kNGTQyn (.tmp/us0249-live-clippy.log,
.tmp/us0249-live-ci.log). The default-feature CI gate does not run the
native-desktop live proof: its direct three-pane quit failure remains open.
Optional -Full cargo-deny was not run; five accepted glyph exceptions unchanged.

Orderly-close rework update (2026-11-13; supersedes the quiet-only proof below):
NativeLocal now seals raw/control admission independently of hard stop. Active
publication and accepted controls finish before the existing teardown path;
the native pane requests this orderly shutdown. Hard close and abandoned
acknowledgment guards still use the unchanged fail-stop flag. The actual
workspace child-process proof now closes immediately after initial layout,
without ready-output or quiet waits, and passes. A real ConPTY focused proof
holds reliable delivery at capacity one, queues resize/selection controls,
seals admission, verifies raw/control rejection and pending completion, then
drains events and observes every acknowledgment, final FIFO resize and Closed
with LifecycleDelivery::Complete. Focused evidence:
.tmp/us0249-orderly-local.log (1 passed), .tmp/us0249-orderly-app.log (4 passed).
Full native regressions, native all-target Clippy and ci-local passed in
sh_1d47071630015bicwoX2UefZHe: terminal 358, local 74 (3 existing ignored),
SSH 160, view 507 (3 existing ignored), app 41. Evidence:
.tmp/us0249-orderly-*-regression.log and .tmp/us0249-orderly-ci.log.
Optional -Full cargo-deny was not run; five existing accepted glyph contrast
exceptions remain unchanged. No claim of real-HWND admitted-operation close or
successful 200 ms multi-pane application quit follows from these tests.
No Release rebuilt/launched. Existing deadlines and visible errors are unchanged.

Native startup continuation (2026-11-13): owner chose persistent isolated
`oneterm-native-data` beside the test EXE. DEC-0023 and native-workspace LLD
record the selection. Implemented a default-off once-only configuration root,
tagged create_local service, native-only app factory, injected feature init and
the actual workspace entry with automatic SSH/update/crash-dialog suppression.
Production startup remains unchanged; native legacy-local/SSH requests reject.
A weak pane registry collects completion observations across tabs/Spaces,
prevents new creation during shutdown and preserves earlier cleanup failures.
Ordinary window close defers HWND/root removal while observing the five-second
budget; forced app quit retains GPUI's budget and fails on cancelled/unpolled
observation. Old standalone sources/fixture tests remain test-only, not a second
runtime entry. Pinned reference/srcwalk still unavailable; reused project
patterns and inspected the resolved GPUI scheduler/window sources locally.

Initial focused app proof passed real workspace/native factory/isolation, but
the revised immediate-close path exposed a genuine failure:
`Native pane: lifecycle delivery incomplete` while initial native work was
still pending (.tmp/us0249-startup-app-proof.log, intermediate run). Do not
weaken backend cancellation guards or reinterpret this as successful cleanup.
Separate quiet-window startup/close proof from admitted-work close acceptance.
Native startup is not yet a Release/UAT deliverable; complete regression/gate
and shutdown-race validation are still required. No Release rebuild or desktop
launch is claimed. Cancelled search repeats remain cancelled.

Latest focused proofs pass: core isolated-root tests 2; actual panel startup
and three-pane/two-tab shutdown 1; app startup proofs 4 (real workspace/native
factory/isolated storage and quiet-window close, no legacy fallback, sticky
errors and child-process forced-quit cancellation). Native all-target Clippy
passes. Logs: .tmp/us0249-startup-{storage-proof,view-proof,app-proof,clippy}.log.
The quiet-window proof explicitly waits for ready output/initial activity; it
does not resolve the immediate/admitted-work close failure above. Full native
regressions and the bundled local gate completed successfully under
sh_1d45d2001001p0F5iINAMT3ZGX: core 95, terminal 358, local 73 (3 existing
ignored), SSH 160, terminal-view 507 (3 existing ignored), app 41 passed.
Native all-target Clippy with warnings denied and scripts/ci-local.ps1 passed.
Evidence: .tmp/us0249-native-startup-{core,terminal,local,ssh,view,app,clippy,ci}.log.
Optional -Full cargo-deny was not run; five existing owner-accepted theme glyph
exceptions remain unchanged. This verifies the implemented startup increment,
not immediate/admitted-work close, real-HWND or 200 ms multi-pane quit acceptance.
No Release rebuild or Windows UAT is claimed.

Native cursor continuation (2026-11-13): workspace-bound surfaces now apply
live shape and explicit color overrides through shared CursorConfig, using
the existing setting-to-shape mapping. A surface-owned weak task uses the
shared 500 ms timer grid; running/visible/blinking native cursor, focus and
live blink-enable gate phase changes. Focus/blur and disabled/non-blinking
states reset visible phase. Native hidden cursor remains hidden; native steady
cursor never disappears during blink-off. Unbound standalone surfaces retain
native appearance without a timer. No backend setting mutation or session
reference is owned by the timer. Release cancellation proof passes.
Four focused tests pass (.tmp/us0249-native-cursor-proof.log), including actual
paint color/shape, live changes, native hidden/steady, explicit focus and
scheduler-driven phase plus weak-release cleanup. Headless windows do not
provide real OS focus; the scheduler proof uses an unhosted surface with
explicit focus state and does not claim real-window focus UAT. Verification
completed successfully under sh_1d41d9506001CT6XW6yYNcNciW: native terminal
358 passed; local 73 passed, 3 existing ignored; SSH 160 passed; terminal-view
506 passed, 3 existing ignored; app 36 passed. Native app all-target Clippy
with warnings denied and full scripts/ci-local.ps1 passed. Evidence:
.tmp/us0249-native-cursor-{terminal,local,ssh,view,app,clippy,ci}.log.
Optional -Full cargo-deny was not run; five owner-accepted theme glyph
contrast exceptions remain unchanged. No Release rebuild or Windows UAT
is claimed. Reviewed renderer cursor contract and
legacy blink policy; updated native workspace and renderer LLDs. No schema,
dependency or production-startup change.

Next gutter work requires an honest native row/timestamp contract: current
native row identities are opaque equality-only values, not ordered absolute
RowIds; legacy GutterTimestamps requires absolute_line_count and clear_epoch.
Do not invent those counters or construct TerminalInfo to make the old gutter
render. Gutter/semantic settings, native startup/SSH/storage, multi-pane quit,
optimized workspace build and Windows acceptance remain open. Existing
close budgets and cancelled search repeats stay unchanged.

Native channel continuation (2026-11-13): actual panel/menu/tab-wide native
membership and mixed legacy/native targets are connected. Native peer callbacks
capture weak panes, use ordered bounded inboxes, and leave on exit/close,
shutdown and release. Raw-visible local/SSH operations preserve exact bytes,
share paste budgets and retain fail-stop cancellation. Bound surfaces serialize
key receipts, committed text, paste and origin-only releases/blur. Only successful
origin acknowledgment fans out; leave/rejoin/move/close invalidates delayed
fan-out through the enqueue-time enrollment token. Empty key encodings do not
broadcast, but still permit original release; queue-rejected releases remain
held for explicit key-up/blur cleanup.

Native regressions passed: terminal 358; local 73 with 3 existing ignored; SSH
160; view 502 with 3 existing ignored; app 36. Native app all-target Clippy
passed. Evidence: .tmp/us0249-native-channel-{terminal,local,ssh,view,app,clippy}.log.
Focused proofs cover raw bytes vs paste, shared budget/cancellation, mixed
membership/order/isolation, origin success/failure/empty receipts, IME/key/paste
ordering, releases/blur, enrollment changes, bounded queue/payload admission,
surface release and actual panel menu/actions. Peer processing failure isolation
and pending-inbox shutdown cancellation also pass in the latest native view
regression, with all-target native Clippy rerun successfully. Documentation path
and English checks and git diff --check pass. The bundled full local gate
scripts/ci-local.ps1 passed in .tmp/us0249-native-channel-ci.log (background
run sh_1d3b270f3001PIuAnciUp5q1sF). Its five existing owner-accepted theme glyph
contrast exceptions remain unchanged; optional -Full cargo-deny was not run.
No optimized deliverable or desktop
acceptance is claimed. Remaining: cursor/gutter/semantic settings, native
startup/factory selection, SSH orchestration/duplication, storage isolation,
multi-pane 200 ms app-quit coordination, admitted real-HWND close and ordinary
token/full Windows UAT. Existing five-second close and cancelled search repeats
are unchanged; the old standalone Release executable remains preserved.

Native input-channel prerequisite continuation: SurfaceSession now exposes an
explicit key_receipt future; unsupported implementations reject without a
fallback. Local and SSH native endpoints route it through their bounded
control FIFOs with existing admitted-wait cancellation guards. The confined
pump encodes once, admits the bytes once to the original transport and returns
those exact bytes only on success. Empty encoding performs no write; failures
return no receipt and never retry. Existing key/release admission is unchanged.
Six focused proofs passed: pump mode-sensitive bytes/empty encoding/transport
failure and cancellation; actual local shell exact input and queue cancellation;
SSH mode-sensitive bytes/origin isolation and queue cancellation. Fresh native
regression passed: terminal 356; local 71 with 3 existing ignored; SSH 159; view
492 with 3 existing ignored; app 36. Native app all-target Clippy, full
scripts/ci-local.ps1 and git diff --check passed. Evidence:
.tmp/us0249-native-key-receipt-{terminal-proof,local-proof,ssh-proof,terminal,local,ssh,view,app,clippy,ci}.log.
No new dependency or persistence change. Optional -Full cargo-deny was not run.
This is prerequisite proof, not completed native input channels: membership,
weak peer ownership, ordered mixed-engine fan-out, origin-success gating and
release/lifecycle integration remain unimplemented. No Release rebuild or
Windows owner UAT is claimed; startup/SSH orchestration/storage and app-quit
gaps remain. Preserve cancelled search repeats and the existing close budget.

Native local duplication continuation: the real panel now duplicates a native
local source into the existing dock tab, empty Space or split without creating
a legacy session. The explicit SessionFactory::spawn_native_local operation
rejects unsupported factories by default; the app implementation constructs a
configured NativeLocalSession. Normal spawn_local/startup remain legacy.
Recorded shell arguments, environment and UTF-8 policy are preserved; live cwd
replaces configured cwd and current scrollback/security/logging settings pass
through the service. CreatedSession retains engine identity through placement.
Occupied destinations and missing tab containers close through native pane
event draining and consuming completion with the unchanged close budget. Source
identity is preserved after success and factory failure. Native SSH duplication
still rejects before entering the legacy dialog; orchestration is not ready.
Three actual native panel duplication proofs and one real app factory proof
passed, including pre-spawn logging rejection. Fresh full native regression:
terminal 354; local 69 with 3 existing ignored; SSH 157; view 492 with 3 existing
ignored; app 36. Native app all-target Clippy, scripts/ci-local.ps1 and
git diff --check passed. Evidence: .tmp/us0249-native-duplicate-{view-proof,app-proof,terminal,local,ssh,view,app,clippy,ci}.log.
Optional -Full cargo-deny was not run. No Release rebuild or Windows owner UAT
is claimed. Remaining: input channels, cursor/gutter/semantic settings, native
startup/factory selection and SSH orchestration/duplication, storage isolation,
multi-pane app-quit coordination and admitted HWND-close proof. The two
owner-cancelled search repeats remain cancelled.

Native clear continuation: the actual panel action now uses bounded native
surface edit admission and an acknowledged original local/SSH control FIFO.
It sends exactly b"clear\r", including with bracketed paste enabled, then
clears/publishes native selection without resetting the terminal or locally
erasing history. Transport failures remain visible, clear selection when
possible and fail-stop without retry. Closed admission, full queue rejection,
post-admission cancellation and reliable-event failure/cancellation are proved.
The SSH peer proof verifies origin isolation and preserved selectable content;
the real local shell receives clear as a line. Actual panel dispatch and pending
acknowledgment after window/surface release are proved. Close budgets and
production factories are unchanged.
Fresh verification passed: native terminal 354; local 69 with 3 existing ignored;
SSH 157; view 489 with 3 existing ignored; native app tests/all-target Clippy
and the full scripts/ci-local.ps1 gate. Focused native view/panel tests: 39.
Evidence: .tmp/us0249-native-clear-{terminal-proof,local-proof,ssh-proof,view-proof,local-cancel,ssh-cancel,terminal,local,ssh,view,app,clippy,ci}.log.
git diff --check passed. Optional -Full cargo-deny was not run. No Release
rebuild or Windows owner UAT is claimed. Remaining: duplication/input channels,
cursor/gutter/semantic settings, native factory/startup and SSH orchestration,
storage isolation, multi-pane app-quit coordination and admitted HWND-close
proof. The two owner-cancelled search repeats were not restarted.

Native select-all continuation: the real panel action now reaches the native
surface and the pinned worker-owned select-all API through the existing
acknowledged selection FIFO. History and alternate-screen selection use native
semantics, not fabricated row endpoints. Bounded edit admission rejects overlap;
the detached operation survives actual window/surface release and retains its
completion obligation. The real panel proof exercises select-all then copy.
Local and authenticated SSH surface proofs exercise the same update.
Fresh verification passed: native terminal 352; local 68 with 3 existing ignored;
SSH 156; view 488 with 3 existing ignored; native app tests and native app
all-target Clippy. All 38 focused native view/panel tests and the bundled
scripts/ci-local.ps1 gate passed. Evidence:
.tmp/us0249-native-select-all-{terminal-proof,view-proof,local-proof,ssh-proof,terminal,local,ssh,view,app,clippy,ci}.log.
git diff --check passed. Optional -Full cargo-deny was not run. No Release
rebuild or Windows owner UAT was claimed at that increment. Clear is now
implemented and verified in the continuation above. Duplication, input channels,
remaining settings, factory/startup, SSH orchestration, storage isolation and
multi-pane app-quit coordination remain incomplete.

Program-effect continuation: NativeTerminalPane now reduces progress/bell and
bounded notifications independently of paint, and applies bounded ordered
clipboard operations on its foreground refresh, including non-rendered panes.
Backend policy admission is unchanged; reads additionally require live bound
settings, a live originating session and a size-bounded OSC 52 reply. The local
NativeSession and authenticated NativeChannel use their own original transports;
SSH service/factory orchestration is still not connected. Existing notification
and progress/bell widgets are reused. Exit/shutdown clear pending effects and
suppress later effects; overflow and reply failures remain visible.
Ten focused native owner/panel tests passed. Final fresh verification completed
successfully under sh_1d2fad616001A30E8DWy7DTNKM: native terminal 351 passed;
local 68 passed with 3 existing ignored; SSH 156 passed; view 487 passed with
3 existing ignored. Native app tests, native app all-target Clippy, the bundled
scripts/ci-local.ps1 gate and git diff --check passed. Evidence is retained in
.tmp/us0249-native-effects-{proof,terminal,local,ssh,view,app,clippy,ci}.log.
The gate retains five owner-accepted theme glyph contrast exceptions; no new
failure is reported. Optional -Full cargo-deny was not run. No Release build
or owner desktop acceptance has occurred for this increment.

Next: finish remaining native configuration/input and duplication capabilities;
integrate native factory selection and SSH authentication/channel ownership;
settle test-entry storage isolation before reusing workspace initialization.
Observe multi-pane window/app-quit completion separately from the pane-close
proof. Rebuild Release only after that actual native route exists. No owner-
cancelled pending-search repeat or system-wide session-end action was restarted.
