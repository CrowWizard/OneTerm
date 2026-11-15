# Work: Native session default colors

ID: US-0235
Intake: IN-0057
Created: 2026-11-11

## Status

<!-- HARNESS:STATUS:BEGIN -->
- [x] Planned
- [x] In progress
- [ ] Implemented
- [ ] Changed
- [ ] Reopened (acceptance rework)
- [ ] Retired
<!-- HARNESS:STATUS:END -->

## Classification

- Change type: existing-contract change
- Risk lane: high-risk
- Spec Intake: IN-0057; native LLD; depends on US-0212/0214/0230/0234.

## Outcome

Explicit native local/SSH sessions apply caller-owned default colors through the
native owner and acknowledge a new complete frame without needing more output.
Program OSC overrides remain authoritative until reset, and native color queries
reply through the existing originating transport FIFO using the effective colors.

## Scope

- [x] In scope: fixed owned foreground/background/optional cursor/256-color
  palette value, pinned typed FFI options, owner/bridge commands, guarded pump
  publication, acknowledged local/SSH 64-slot control admission and local session
  forwarding, native override/reset/query and full-damage proof.
- [x] Out of scope: production theme selection or factory cutover, 16-to-256 theme
  palette policy, color-scheme callback, full dynamic-colors facade, complete
  TerminalSession/render/IME, clipboard, GUI/Unix acceptance or native ABI changes.

## Acceptance

- [x] Default foreground/background/cursor/palette update publishes one complete
  frame and repaint without output; independent readers observe full damage.
- [x] OSC 4/10/11/12 overrides survive default changes; native reset reveals the latest
  defaults. Native queries use effective colors and original reply-first FIFO.
- [x] Prior reliable events deliver before mutation/capture; local/SSH controls
  share the existing FIFO and success means publication, not physical paint.
- [x] Unpolled futures do not enqueue; QueueFull/Closed reject recoverably;
  admitted cancellation closes independently and queued result senders drain.
- [x] Native/delivery/publication errors acknowledge then fail-stop retaining
  last-good; no rollback/replay of partial native defaults or consumed output.
- [x] Pinned C discriminants and RGB layout are checked by the C ABI probe;
  native handle affinity and the external oneterm-vt API remain unchanged.

## Documentation

### Owning Docs Reviewed

- `docs/PROJECT.md`, `docs/HARNESS.md`, `docs/templates/work.md`; agent code style,
  structure, dependencies, crate rules and error policy.
- IN-0057 native LLD and DEC-0023; backend migration note and US-0234.
- TerminalRender default-color contract; native render color identity/palette,
  global damage, owner/bridge commands and local/SSH acknowledged controls.
- Pinned `terminal.h` color options 12-15 and `terminal.zig` default setters:
  defaults and OSC overrides are distinct; palette replacement may allocate/fail.

### Documentation Action

Update required: native LLD, backend migration note, probe README, US-0208 handoff.
No persisted schema, production theme policy or external VT contract change.

### Reconciliation

Updated native LLD, backend migration note, probe README and US-0208 handoff.
Public API docs distinguish defaults from effective OSC override colors and
publication from paint. No persistence, dependency or production selector change.
ConPTY OSC 110 reset experiment is retained below as an unresolved parity gap;
the native reset criterion is not a local end-to-end compatibility claim.

## Plan

- [x] Add fixed owned colors and ABI-checked native/owner/bridge setter.
- [x] Add guarded publication and bounded acknowledged session controls.
- [x] Prove native override/reset/query, frames/damage/order, real local/SSH operations,
  admission/cancellation/fatal ack; run regressions and full gate.

Probe reconciliation: its standalone crate does not import session adapters, so
US-0234's owned selection Update is otherwise unused under -D warnings. Exercise
both update variants against the real owner in the standalone runner; do not
silence dead-code warnings or remove the existing native selection assertions.

## Decisions

DEC-0023 and native LLD. Accept all 256 caller-provided palette entries, not a
palette read from effective OSC state (which would promote overrides to defaults).
Use native default setters; do not synthesize OSC into the parser. Partial native
setter failure is fatal; publication follows all setters, never claims rollback.

## Verification Plan

Native owner/pump tests for effective colors, overrides/resets, query bytes,
independent full damage and immutable old frames. Real configured ConPTY and
authenticated SSH fixtures; blocked UI/control pressure and ack cleanup. Run
native terminal/SSH/local regressions and all-target Clippy, ci-local.ps1, C probe,
doc-path/English/dependency/diff checks. Record pre-existing failures explicitly.

<!-- HARNESS:PROOF:BEGIN -->
- [ ] Unit proof
- [ ] Integration proof
- [ ] E2E proof
- [ ] Platform proof
- [ ] Verify command passed
<!-- HARNESS:PROOF:END -->

## Evidence and Gaps

Windows 2026-11-11:

- Eight new Cargo tests: two native owner, two async pump, two real local
  session/owner and two authenticated SSH endpoint fixtures. Extended the existing
  local mock and SSH publication fixtures to preserve both selection and color
  fatal-ack proof and mixed resize/selection/colors FIFO ordering.
- Owner tests prove full palette/default application, OSC 4/10/11/12 override
  preservation across new defaults, reset to latest defaults, optional cursor
  unset and old-frame ownership after changes/close. Native replies mirror query
  BEL/ST termination; initial expected ST for a BEL query was corrected with an
  additional ST-query assertion, not by changing native parser behavior.
- Pump tests prove no-output capture/repaint and both independent readers' full
  damage; subsequent color-query replies go through the original mock transport.
  Reliable title delivery blocks mutation/capture under full UI pressure; release
  delivers title before Output, or cancellation closes with last-good retained.
- Real local session receives a program foreground override, waits for input,
  then acknowledges defaults with correct background/cursor/palette and preserved
  override, without further child output. Real authenticated SSH acknowledges
  correct frame colors and returns exact queried foreground/palette bytes to the
  server, without synthesizing parser/peer bytes for default updates themselves.
- Local/SSH 64-slot pressure rejects QueueFull without shutdown. Unpolled future
  construction has no effect; dropping one admitted future independently closes,
  drains queued senders and resolves every remaining result Closed. Closed
  admission rejects without retry. Payloads are fixed-size and boxed in queues.
- Local mock proves resize then default-colors publication; SSH proves UI pressure
  holds resize/selection/colors in FIFO order while raw writes progress. Active
  reliable-event receiver closure acknowledges EventsClosed before owner error,
  preserving last-good for both selection and colors. Native allocation failure
  is not injected; native setter errors use the same fatal path without rollback.
- `cargo test -p oneterm-ssh -p oneterm-terminal --features
  oneterm-ssh/ghostty-native --quiet`: SSH148/terminal331 pass. Local native filter
  passes22; native local/SSH all-target Clippy passes -D warnings.
- Updated C option discriminants12-15 and existing RGB layout guards pass the C
  probe. Standalone Rust owner probe passes69 tests, including both owned
  selection Update variants (previously unused by this runner under -D warnings).
  Log: `.tmp/us0235-native-probe.log`. No upstream source/ABI changes.
- Full native local:55 pass/2 existing conhost 0x6d orphan failures/3 ignored at
  session_orphan_tests.rs:252, immediate drop and first output, assertions unchanged.
- ci-local.ps1 passes fmt and both workspace Clippy variants, then fails unchanged
  core shell.rs:916 PowerShell Enter guard, empty verdict vs INSTALLED:AcceptLine,
  92 pass/1 fail. Later gate commands not reached. Log: `.tmp/us0235-ci-local.log`.
- Final documentation checks pass: doc paths (220 current paths), English
  contributor text (1245 files), dependency graph (20 workspace packages), and
  `git diff --check`. These do not replace the failing full gate above.

Local reset experiment: the child emitted OSC 110 then a colors-reset title. After
observing that title and an empty-paste control barrier, the native frame still
reported the prior #aabbcc override, not the configured #112233 default. Direct
native reset passes. The production ConPTY output path may transform/filter this
sequence, but no raw-wire capture proves attribution. The real local fixture now
tests its supported update/override boundary and makes no reset-compatibility
claim; no existing accepted assertion was removed. Retain this as cutover risk.

Functional staging criteria have focused proof; complete acceptance remains
gate-blocked. No GUI/Unix/production theme or engine-switch claim. Harness tool is
not exposed; retain the Markdown work record.

## Handoff

Production content/render/IME and theme palette mapping, selection interaction,
search, remaining OSC/security parity and simultaneous local/SSH cutover remain.
