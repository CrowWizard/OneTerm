# Work: Native SSH forwarding

ID: US-0257
Intake: IN-0057
Created: 2026-11-15

## Status
- [x] Planned
- [ ] In progress
- [x] Implemented

## Classification
- Change type: new capability
- Risk lane: high-risk
- Spec Intake: IN-0057

## Outcome
Native SSH supports configured Local/Remote/Dynamic forwarding safely.

## Scope
- [x] In scope: target forwards, handle service and observed lifetime.
- [x] Out of scope: agent forwarding, schema/dependency/theme changes.

## Acceptance
- [x] All three kinds carry traffic with existing bind/failure semantics.
- [x] Session completion observes listener/relay destruction.

## Documentation
### Owning Docs Reviewed
docs/PROJECT.md; docs/HARNESS.md; docs/templates/work.md; docs/agents code,
structure, dependency and error policies; IN-0057 HLD; tunnel.rs, handler.rs,
legacy task handle-request service and native channel teardown.
### Documentation Action
Update required: native-forwards LLD and verified capability statements.
### Reconciliation
Updated PROJECT, intake HLD and native-ssh ownership contract. Forward
configuration, endpoint restrictions and agent-forwarding rejection are retained.

## Context
Existing detached forwarding tasks need observed native completion ownership.
Harness discovery returned no tool; no database registration claimed.

## Plan
- [x] Integrate existing forwards with tracked native ownership.
- [x] Verify and reconcile.

## Decisions
Follow low-level-design/native-forwards.md.

## Verification Plan
In-process traffic, bind failures, cancellation and destruction proof; native
regression, strict native Clippy and scripts/ci-local.ps1.

## Evidence and Gaps
2026-11-15: combined native two-hop/SFTP/logging test carries Local, SOCKS5
Dynamic and Remote traffic; an occupied listener emits the existing warning
without blocking other forwards. Completion observes task destruction, permits
local/dynamic port rebinding and closes an unfinished SOCKS handshake.
TaskTracker covers listeners and relay futures; cancellation wraps connection,
handshake and copy rather than just the final copy. Existing tunnel tests
retain unknown-remote-port denial and configured endpoint semantics.
SSH 176 PASS (.tmp/native-parity-security-tests.log). Physical/external-server
acceptance remains separate. Final combined gate/release evidence pending.

### Final gate and local Release (2026-11-15)

All five explicit-native library suites PASS: local 76/3 existing ignored,
session-ui 109/1 existing ignored, SSH 176, terminal 365, view 522/3 existing
ignored (1,248 passed total). Strict native app all-target Clippy PASS; complete
scripts/ci-local.ps1 PASS; git diff --check PASS. Logs:
.tmp/native-parity-tests.log, native-parity-clippy.log, native-parity-ci.log.
The optional cargo-deny -Full gate was not run.

Optimized native build PASS (.tmp/native-parity-release-build.log). Artifact:
target/release/oneterm-native-desktop.exe, 46,434,816 bytes, GUI subsystem 2,
SHA256 6D9FFD1AD86594B1AADD6C119BC7018059555322C72F21CED9EDF8EAA335770E.
Manifest: .tmp/native-parity-release-manifest.json. Hash-verified predecessor:
.tmp/native-parity-predecessor-20261115-003035/oneterm-native-desktop.exe,
SHA256 63BD7F2A2A53C736666308FEA7A5B044E96592FF13CD468D833BED9917723888.

Owned no-argument startup and WM_CLOSE smoke PASS: PID 13960, HWND 3998288,
exact executable/title rechecked before close, exit 0. Evidence:
.tmp/native-parity-release-smoke-20261115-004257.json and stdout/stderr siblings.
Inherited token Elevated; bundled ConPTY; isolated EXE-sibling storage. All
four pre-existing isolated data-file hashes unchanged, no new data files
(.tmp/native-parity-data-before.json). No saved server/credential was used.

The deferred GPUI shutdown diagnostic did reproduce: stderr contains
`ERROR window not found` after native surface release. Exit remained 0; this
smoke is startup/exit/data evidence, NOT a fix or clean-log acceptance for
that explicitly deferred defect. Ordinary-token persistence remains deferred.
External-server duplication/disconnect, active-operation desktop shutdown,
physical drag/drop, OS IME and visual menu/indicator acceptance remain separate.
No theme/schema/dependency policy change, production cutover, public release,
commit or push. Harness discovery returned no tool; no database status claimed.

## Handoff
Same-session implementation.
