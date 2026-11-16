# Work: Terminal Tab completion routing

ID: US-0261
Intake: IN-0057
Created: 2026-11-16

## Classification

- Change type: bug
- Risk lane: scoped terminal keyboard routing; no persistence or protocol change

## Outcome

While a terminal is focused, Tab reaches the shell for completion instead of
moving focus between application controls. Preserve Shift+Tab terminal input
and ordinary focus traversal in non-terminal controls.

## Scope

- In scope: shared terminal key binding registration and dispatch regressions.
- Out of scope: shell completion algorithms, global focus-policy changes,
  user settings, engine cutover and unrelated shortcuts.

## Acceptance

- [ ] Native-only startup registers terminal Tab/Shift+Tab overrides.
- [ ] Real dispatched Tab reaches the native session and retains focus.
- [ ] Shift+Tab reaches the native session without focus traversal.
- [ ] Legacy behavior and non-terminal control traversal remain intact.

## Documentation

### Owning Docs Reviewed

- AGENTS.md, docs/HARNESS.md and docs/templates/work.md: workflow and proof.
- docs/PROJECT.md and docs/agents/{code-style,structure,crate-dependency-rules,error-policy,dependencies}.md: boundaries and local reference policy.
- IN-0057/low-level-design/native-workspace.md: shared native workspace.
- crates/terminal-view/src/terminal_view/view.rs: existing terminal-only
  NoAction bindings, incorrectly installed by the legacy constructor.
- crates/terminal-view/src/native_surface.rs: Terminal context and Tab mapping.

### Documentation Action

Update required: native-workspace.md records shared registration and scope.
No persisted data, public engine interface or completion implementation changes.

## Context

Both views use the Terminal key context. Only the legacy constructor currently
registers the Tab/Shift+Tab NoAction overrides, so native-only startup leaves
component focus-traversal bindings active. Move these existing bindings to
the terminal feature initialization used by both app entry points.
Pinned reference/gpui-kit and srcwalk are absent in this checkout. Inspect
the locked registry source if needed once Cargo restores dependencies.
Harness tool discovery returned no tool; no database status is claimed.

## Plan

- [ ] Move existing scoped bindings to feature initialization.
- [ ] Add native dispatch/focus regression and verify shared controls.
- [ ] Run bundled CI plus native proof and reconcile evidence.

## Verification Plan

Use actual Root/keyboard dispatch (not only a direct key_down call), assert
native key admission and retained focus, and preserve ordinary form traversal.
Run scripts/ci-local.ps1, native terminal-view tests and strict native Clippy.
The prior cleanup removed caches: verification necessarily recreates ignored
build intermediates. Preserve the accepted Release and all user data.

## Evidence and Gaps

Moved existing bindings to shared init and removed constructor registration.
Added a shared Root dispatch/focus fixture plus a native endpoint dispatch
regression; compilation/runtime proof remains pending dependency restoration.
The checkout has no registry cache or native Ghostty build inputs, despite
their prior retention record. No unscoped cache search or substitute native
library was used. Cargo fetch and the bundled CI are running with project-local
cache/temp paths. Current release is named target/release/oneterm.exe and
still has the prior accepted SHA256
45B15423B49BA5C0309CDCCDE81792DD0875A27784211BEFA22A620DE145178C;
it has not been replaced. No user data was edited.

Passing lightweight checks: check-doc-paths.py, check-english.py,
verify-dependency-graph.py and git diff --check. These are not compiler or
GUI proof. Shell-side completion remains the shell's responsibility;
forwarding Tab is not a new application completion engine.

Initial cargo fetch failed because the inherited proxy route attempted an
unreachable localhost endpoint. A direct HTTPS registry probe succeeded.
Retry uses only process-local CARGO_HTTP_PROXY/NO_PROXY overrides; no user
global proxy/Cargo configuration was changed. Fetch retry remains pending.

The original bundled CI also stopped at dependency resolution with the same
proxy error, before Rust compilation. It is not a code-test failure or a pass.
A separate bundled CI retry now inherits the process-local direct settings.
Locked registry sources have begun restoring. Native proof additionally
requires restoring the exact source and Zig archives specified by the existing
build script/README, checking their pinned SHA256 values before extraction
or execution. That restoration/build is pending; no alternative upstream
revision or toolchain is authorized by this fix.

Direct locked dependency fetch PASS. Restored gpui-pre 0.3.9 and gpui-base
0.8.0 registry sources confirm that Root binds Tab/Shift+Tab traversal and
that the Terminal override belongs in shared feature initialization. Added
legacy actual-dispatch proof asserting Tab byte 0x09, Shift+Tab ESC [ Z and
retained focus. The shared Root fixture separately exercises ordinary form
forward/backward traversal. Focused tests and full CI are running; native
archive restoration is independent and still pending its completion result.

First focused run compiled successfully: legacy Tab byte/focus regression
passed, as did two existing tab-close regressions. The shared form-navigation
fixture failed after changing focus without drawing the new dispatch path.
Added explicit draws after focus changes; rerun pending. This is not yet
proof that form navigation is preserved. Native source archive hash matched,
but the Zig download timed out at approximately 10 MB of 100 MB; resumable
download of the same pinned archive is running with a longer timeout.
No native build or replacement Release is claimed.

Latest direction: stop local compilation and move verification to Actions
(US-0262). The explicit-draw retry also FAILED form traversal; drawing alone
was not the cause. Full local CI reached workspace tests: 456 passed, one
failed, three ignored in terminal-view. Registry source shows explicitly
tracked focus handles bypass element-level tab-stop assignment. Configure
the fixture handles themselves with tab_stop/tab_index and render nonzero
targets; leave runtime verification to cloud CI. The owned resumable-download
worker was stopped (exit 255); no further local build will be launched.

Configured retained form handles with tab_stop(true)/ordered tab_index and
nonzero rendered target sizes. rustfmt check passes. This fixture correction
has NOT been executed after the user's cloud-only direction. Dedicated native
workflow now gates packaging on the bundled full check and native workspace
lint/tests, covering shared, legacy and native Tab regressions. The previous
failed result is not superseded until a real cloud run passes.
