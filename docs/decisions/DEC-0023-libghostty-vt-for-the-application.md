# DEC-0023 libghostty-vt for the application

Date: 2026-11-10

## Status

Accepted migration direction; application cutover not yet implemented or verified.

## Context

The owner explicitly requested libghostty-vt as the replacement application
engine in IN-0057. This revisits DEC-0014's engine choice, not its independent
transport boundary or the subsequently established external oneterm-vt contract.
The current upstream C API supplies a headless terminal, incremental VT writes,
reply callbacks, render state and OSC 7501 callbacks. The old survey's API facts
must be rechecked against the pinned upstream revision rather than assumed.

## Decision

Use upstream libghostty-vt as the application's sole parser/state engine after
parity is proven. Retain GPUI drawing and existing ConPTY/openpty/russh transports.
Keep native handles, callback lifetime and borrowed render state encapsulated in
the gpui-free terminal adapter; publish owned frames and policy events. Pin source,
headers and toolchain together. No fork, hidden PTY, dependency rename or second
production terminal state may stand in for the migration.

DEC-0014 remains descriptive of the current implementation until cutover; this
decision supersedes its no-FFI/application-engine choice for US-0208. The external
oneterm-vt package remains independent and keeps its API, features and tests.
Missing graphics or OSC functionality is an explicit acceptance gap, not implicit
permission to regress. The detailed boundary and proof sequence are owned by
IN-0057's `low-level-design/ghostty-engine.md`.

Historical acceptance tooling amendment (2026-11-12, superseded below): the independent
US-0245 binary may accept --allow-elevated-test for a verified elevated token,
because ordinary desktop launch is unavailable in the owner's environment.
Default elevated launches and every unknown-token launch remain rejected. Preserve
real elevation state, explicit privileged-mode labels, no persistence and OSC
effect denial. This is not sandboxing, production authorization or evidence of
ordinary-token behavior; never generalize this opt-in to the production factory.

Owner UAT amendment (2026-11-13): withdraw the independent launcher's elevation
opt-in requirement and administrator-specific mode UI. No-argument startup must
work under ordinary, elevated and unknown token classifications; retain the real
classification for underlying checks, never spoof it or self-elevate. Configuration
isolation, default OSC denial and observable cleanup remain. This amends only
the test entry, not production permission or persistence rules. The owner also
requests the real OneTerm workspace with the native engine (US-0249), not a
cosmetically similar standalone probe or a fallback to the old engine.

## Alternatives

Native workspace storage amendment (2026-11-13): the owner selected persistent
test-only storage in `oneterm-native-data` beside the native test executable.
Install the absolute root once, before configuration, vault, session, layout or
crash initialization; never import or fall back to production data. Failure to
prepare the root aborts startup. Do not automatically connect saved SSH sessions
or check for updates in this test entry. Existing token-based persistence rules
still apply inside that root. This is not a persisted engine selector or a
production-default change. Manual SSH remains explicitly unsupported until its
native authentication/connection orchestration is integrated, not legacy fallback.

- [x] Direct pinned libghostty-vt C ABI with existing transports and renderer.
- [ ] librio-vt: superseded by the owner's explicit choice after its transport
  coupling was investigated.
- [ ] Renaming oneterm-vt or keeping it as a shadow production engine: does not
  achieve the requested replacement.
- [ ] Removing the external oneterm-vt package: unnecessary and breaks consumers.

## Consequences

- [ ] Confirm native Windows build/linking and safe native ownership.
- [ ] Confirm render/input/history/selection/search/graphics and OSC-policy parity.
- [ ] Update application architecture, native dependency notices and CI/release
  build instructions when actual integration is established.
