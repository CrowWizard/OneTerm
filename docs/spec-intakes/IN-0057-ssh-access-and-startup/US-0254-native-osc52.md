# Work: Native OSC 52 clipboard

ID: US-0254
Intake: IN-0057
Created: 2026-11-14

## Status

- [x] Planned
- [ ] In progress
- [x] Implemented

## Classification

- Change type: new capability
- Risk lane: high-risk
- Spec Intake: IN-0057

## Outcome

Native local and SSH sessions deliver OSC 52 through existing security policy
and source-session asynchronous UI effects without duplicate read responses.

## Scope

- [x] In scope: native OSC boundary capture, bounded decoding, owned events,
  shared policy routing and tests.
- [x] Out of scope: other clipboard protocols, schema/dependency/theme changes,
  production cutover and deferred GPUI/persistence work.

## Acceptance

- [x] Fragmented OSC 52 writes and reads work through the native pump.
- [x] Invalid, cancelled and oversized payloads cause no clipboard effect.
- [x] Remote default-deny, live read permission and shutdown remain intact.
- [x] Native reads produce no premature or duplicate response.

## Documentation

### Owning Docs Reviewed

- docs/PROJECT.md; docs/HARNESS.md; docs/templates/work.md.
- docs/agents/code-style.md, structure.md, crate-dependency-rules.md,
  error-policy.md and dependencies.md.
- IN-0057 HLD/native-ssh LLD; pinned terminal.h write-until/callback ABI;
  native events/pumps, OscRouter, security_policy and native session replies.

### Documentation Action

Update required: native-osc52 LLD establishes the asynchronous bridge design;
reconcile PROJECT/HLD/native-ssh capability statements after verified completion.

### Reconciliation

Updated docs/PROJECT.md, intake high-level-design.md and native-ssh.md to
describe native clipboard support without claiming production cutover.

## Context

Native synchronous read callbacks send empty responses on return. Native
parser OSC boundaries avoid installing those callbacks. Harness discovery
returned no tool; no database registration is claimed.

## Plan

- [x] Implement native bounded observation and existing policy routing.
- [x] Add focused proof and run regression/full CI; reconcile docs.

## Decisions

Follow low-level-design/native-osc52.md and existing accepted security policy.

## Verification Plan

Fragmentation, cancellation, malformed/oversized input, local/remote policy,
reply ordering and shutdown tests; native regression, strict native all-target
Clippy and scripts/ci-local.ps1.

## Evidence and Gaps

2026-11-15 focused proof: native clipboard tests cover every split, BEL/ST,
exact 256 KiB acceptance, oversized/invalid UTF-8/base64/cancelled rejection
and title/clipboard/bell ordering. Router tests prove local and remote
enabled/default-deny policy. Native SSH tests prove enabled effects and one
explicit source-session response, with no premature response. Existing native
pane tests cover live read permission changes and shutdown suppression.
Native security suites: terminal 365 PASS, SSH 176 PASS
(.tmp/native-parity-security-tests.log). Earlier full native suites and complete
ci-local passed; final combined gate/release evidence follows after completion.
No external-server or physical clipboard acceptance is inferred.

Final verification: all five native library suites PASS (local 76, session-ui
109, SSH 176, terminal 365, view 522; 7 existing ignored), strict native app
all-target Clippy PASS and full scripts/ci-local.ps1 PASS. Logs:
.tmp/native-parity-tests.log, native-parity-clippy.log and native-parity-ci.log.
Shared local Release and smoke evidence is recorded in US-0257. No production
cutover, commit, push or publication. Harness tool is unavailable; this is a
Markdown status only, not a claim of harness.db registration.

## Handoff

Same-session implementation.
