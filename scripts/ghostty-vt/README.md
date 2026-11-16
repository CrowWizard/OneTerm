# libghostty-vt migration probe

This is the first native acceptance probe for US-0208, **not** the application
engine cutover. The application still runs on oneterm-vt until its shared adapter
is replaced and parity is verified.

## Local native controls and session staging

### Isolated desktop launcher (US-0245)

The default-off app feature provides a separate local-only acceptance binary:

```powershell
$env:GHOSTTY_VT_PREFIX = "$PWD\.tmp\ghostty-migration\install"
cargo run -p oneterm-app --features native-desktop --bin oneterm-native-desktop
```

Use the project-local Cargo environment described in AGENTS.md. This window is
marked "Native Desktop Probe (not production)". It opens a real native local shell
in the launch directory without loading application configuration, saved sessions,
vault, normal workspace or update services. Launch without arguments: there is
no administrator opt-in flag and no elevation-specific probe UI. The former
--allow-elevated-test argument is removed. The true token is still recorded for
underlying checks; startup neither rejects nor changes it. This is not an OS
sandbox: the shell inherits the caller's privileges. Keep commands within the
test project; an elevated launch does not verify ordinary-token behavior.
The current simple surface is not the requested full OneTerm workspace; that
integration is tracked separately in US-0249 and has not been implemented yet.
Program OSC clipboard read/write and notification effects are denied and
counted; user-initiated surface copy/paste is separate. Title/cwd/progress events
are drained but do not drive OS effects. A dedicated event thread keeps reliable
events moving while GUI painting is paused. Errors and lifecycle are visible in
the footer. Close the window to request shutdown; explicit-quit mode allows up to
five seconds for detached surface cleanup before stopping the GUI executor. A
timeout is an error, not success. Successful process teardown logs
native owner destruction and consumer termination. See US-0245 for verified results
and gaps; an open window alone proves neither input parity nor desktop UAT.

App/session-end quit is separate from normal window close. The pinned GPUI quit
observer budget is 200 ms. The isolated launcher attempts owner/consumer completion
before that path exits; cancellation, timeout or cleanup failure is explicitly
logged and exits nonzero instead of allowing a misleading successful exit. It
does not extend Windows session-end grace or the GPUI budget. Normal WM_CLOSE
keeps its existing five-second surface drain. Targeted synthetic session-end
currently exercises the nonzero failure path, not successful graceful cleanup;
actual OS logout and close during admitted endpoint work remain unverified.

The normal oneterm binary and factory are unchanged even with this feature enabled.

US-0244 adds the independent native search bar and next/previous reveal path.
Use Ctrl+F, type a literal needle, Enter/Shift+Enter or arrow buttons, then Esc.
The UI offers pinned native ASCII-insensitive semantics only; 4096-byte needles,
4096 returned matches, 64 progress operations/query and a partial-state counter
are explicit. Search runs on both real endpoint FIFOs. Generation supersession
waits for admitted work; owner-validated reveal tokens and prepaint/live-revision
checks prevent stale navigation/highlights. Clear/close drive an ordered native
clear. Native internal result memory remains an upstream limitation. Automated
and desktop acceptance status is tracked in US-0244; no production cutover.
Window typing and Enter/Shift+Enter, widget Unicode composition/commit, and
idle/busy entity cleanup have automated proof. This is not OS IME desktop UAT:
US-0245 now provides the separate opt-in launcher for NativeTerminalSurface.
A normal application build still exercises the legacy production surface, not
this staging surface. Real OS IME and pointer/clipboard interaction remain manual
acceptance items until separately recorded.

US-0243 connects independent-surface linear drag to both real endpoint selection
FIFOs. Last-painted samples carry opaque source/grid revisions; selection-only
captures keep them valid, while output/resize/scroll rejects stale coordinates.
One owner-held start anchor, temporary end cleanup, one coalescing surface task
and an independent finish bit bound resources. Shift overrides reporting;
outside-up/blur retains the last valid selection. Teardown signals finish and
detaches the task. Focused proof: terminal346, SSH153, local25, surface9 and native
all-target Clippy pass. The full gate still stops at the core PowerShell Enter
guard; desktop UAT, search UI and production cutover remain open. See US-0243 for
the first-run SSH failure and the precise cleanup/reflow proof limits.

US-0242 adds independent-surface async native-selection copy and visible clipboard
paste. Both real endpoints implement the object-safe paste operation using the
same bounded control FIFO/budget. Live mode/filtering precede reliable-event flush,
viewport reveal/capture/repaint and original transport admission. Low-level paste
stays capture-free. One surface edit task rejects busy admission; empty selection
preserves clipboard and held clipboard chords cannot generate terminal keys.
Focused proof: native terminal345, SSH153, local25, surface8 and native all-target
Clippy pass. US-0243 adds linear drag; search UI, menu/broadcast, desktop UAT and
production selection remain open; see US-0242 for gate evidence and gaps.

US-0241 adds SurfaceSession wheel input on the same 64-slot local/SSH controls.
One live owner operation selects native mouse wheel presses, alternate-screen
cursor keys only when mode 1007 is enabled, or primary history scrolling. Shift
and already-scrolled primary history override reporting. Reliable events precede
mutation; encoded bytes use the original transport FIFO; only history captures
and repaints. Surface line fractions accumulate with a ten-step/event ceiling,
no replay after rejection, and reset on absent/stale geometry or blur. Horizontal
wheel, settings binding, production cutover and desktop UAT remain outside this slice.

US-0240 extends SurfaceSession with native mouse admission and wires the surface's
three-button press/motion/release, including GPUI outside-up callbacks and blur
cleanup. Fractional device-grid coordinates follow the exact prepaint metrics;
padding/absent/mismatched publications suppress new events. Shift bypasses new
press/motion, not releases owed to an admitted press. Live owner modes choose
reporting; a real SSH server proves disabled/enabled/disabled output and exact SGR
press/release bytes. No UI pre-encoding, implicit retry, local selection, wheel,
clipboard, menu, settings mapping or production cutover is claimed.

US-0239 adds feature-gated NativeTerminalSurface and shared SurfaceSession on real
local/direct SSH endpoints. Direct native prepaint requires no fake legacy session.
Typed resize/key/IME failures reach the embedding UI callback; published queries
do not capture, advance damage or acknowledge repaint. Functional keys support
press/repeat/release and cleanup; layout keys use UNIDENTIFIED. IME owns primary
plain text/candidate keys, while alternate-screen keys cannot double-commit.
Candidate bounds anchor at the native cursor, not guessed Unicode cell widths.
Inline preedit/range layout, real-window blur acceptance, full panel/events/security,
application interactions and production selection remain open. Real endpoint
trait-object proof and three surface tests pass; full native view453/1/3 retains
the Fira Code DirectWrite failure. Native terminal339/SSH150/local24 pass.
US-0239 retains full-gate and native Clippy evidence; no selector changed.

US-0238 connects native Content to the actual GPUI Frame/prepaint/PlanCache and
cursor/selection painters through an optional TerminalRender source. Native row
keys/damage, steady cursor, background/cursor RGB and exact spans are used without
legacy snapshot calls. Three new tests cover headless GPUI updates/failure/resize
and source switching. Their session fixture carries a real native model but is
not a complete native backend TerminalSession or manual GUI acceptance. Native
GUI key/IME/input dispatch, history/graphics/parity and factory cutover remain open.
Native terminal338/SSH149/local24 and native all-target Clippy pass. Full view
native450/1/3, default446/1/3 retain Fira Code failure; local57/2/3 retains conhost
failures. Gate remains blocked at core Enter guard after fmt/both Clippy;
`.tmp/us0238-ci-local.log`. No production selector changed.

US-0237 adds shared RenderSource/Content/Viewport for native local/direct SSH.
Each content owns its watermark and reads one immutable prepared publication;
rows/cursor/native modes/scrollbar/effective colors/selection remain atomic.
Window reads are clamped and damage-free; closed/failed sources retain last-good
content. No legacy RowId or mode snapshot is fabricated. Real endpoint fixtures
and the existing view scanner now read through Content. Native terminal338/SSH149/
local focused24 and four-crate native Clippy pass. Full view447/1/3 retains the
Fira Code failure; full local57/2/3 retains conhost 0x6d failures. Full gate still
stops at core Enter guard after fmt/both Clippy (`.tmp/us0237-ci-local.log`). This
is a content boundary, not full render/session wiring or production cutover.

US-0236 adds `set_marked_text`/`marked_text`/`clear_marked_text` and typed
`commit_text` on NativeLocalSession and direct NativeChannel. Shared adapter state
holds at most 1 MiB of preedit UTF-8; single commits have the same byte bound.
Oversize rejection preserves preedit and admits nothing. Valid commit clears
preedit before one exact raw UTF-8 FIFO write, not paste; QueueFull/Closed propagate
without retry, restoration or auto-close. Preedit edits do not feed/capture native
state; a future GPUI adapter must request redraw and implement candidate/ranges.

Four shared unit tests, real ConPTY ASCII commit and real SSH Unicode/control-byte
proof pass; existing SSH pressure proof includes IME QueueFull. Native SSH149/
terminal335/local23 and native all-target Clippy pass. Full local56 pass/2 existing
conhost 0x6d orphan failures/3 ignored. Full gate still fails unchanged PowerShell
Enter guard after fmt/both Clippy variants (`.tmp/us0236-ci-local.log`).
A ConPTY Unicode experiment echoed the right text but Console.ReadLine returned
wrong codepoints even with explicit UTF-8 InputEncoding. US-0236's follow-up uses
child ReadConsoleW to prove exact combining/Japanese and supplementary-plane
UTF-16 units arrive through native commit; the same-session .NET reader comparison
still fails. The new strict native-input regression passes three additional runs;
latest local native24/SSH149/terminal335 and native Clippy pass. This narrows the
gap to the reproduced runtime-reader path, not missing native Unicode input for
these samples; complete .NET/host compatibility remains open. No production
encoding change was made. Gate still stops at the unchanged Enter guard after
fmt/both Clippy (`.tmp/us0236-unicode-ci-local.log`). Selectors remain unchanged.

US-0235 adds async `set_default_colors(colors::DefaultColors)` to NativeLocalSession
and direct NativeChannel. The caller provides foreground/background, optional
cursor and all 256 palette entries. Updates share the existing 64-slot FIFO and
one-slot ack/cancellation guard. Prior reliable events deliver before native
default setters, followed by capture/repaint without more output. Success confirms
publication, not paint. OSC overrides stay separate from defaults; native reset
reveals the latest defaults, and color queries use the original transport FIFO.
Do not derive defaults from effective frame colors or synthesize OSC for updates.
Partial native setter failure fails closed with last-good publication retained.

US-0235 proof: eight new Cargo tests plus local/SSH FIFO/fatal-ack extensions and
one standalone owned-selection seam test (without dead-code warning suppression).
Native SSH148/terminal331/local22 and native local/SSH all-target Clippy pass.
C ABI and standalone owner probe: 69 tests pass. Native tests cover OSC override/
reset, BEL/ST color queries, optional cursor clearing, full independent-reader
damage, reliable-event ordering/cancel, real local/SSH ack/frames, peer query bytes,
64-slot pressure and pending-result cleanup. Full native local55 pass/2 existing
conhost 0x6d orphan failures/3 ignored. Full gate still fails the unchanged core
PowerShell Enter guard after fmt/both Clippy variants; later steps are not reached.

A local ConPTY experiment emitted OSC 110 then a reliable title marker, but after
that marker and a control barrier the old program foreground override persisted.
Direct native reset tests pass. This is an unresolved local transport-path parity
gap, not proof of where the sequence was lost; no raw-wire capture was added.
The real local test therefore proves update/override preservation, not reset
compatibility. Production theme mapping, color-scheme query callback, full native
session/render/IME and GUI acceptance still precede any production cutover.

### Selection baseline (US-0234)

US-0234 adds async `update_selection(selection::Update)` to NativeLocalSession
and direct NativeChannel. Set carries owned tracked start/end anchors plus the
native rectangle flag; Clear removes selection. Both execute in the existing
64-slot FIFO after prior reliable events, then capture selected cells and request
coalesced repaint without more output. Ack means publication, not physical paint.
Native foreign/released/inactive/expired anchor rejection is recoverable, with no
capture or mutation of the old selection. Other pump failures fail-stop; dropping
an admitted future independently closes, never rolls back/replays. SharedModel
set/clear remain mutation-only; its existing copy path stays bounded at 1 MiB and
never writes the clipboard. Production selection interaction remains unconnected.

US-0234 proof: eleven new tests plus the local mock's fatal-ack extension. Native
SSH146/terminal327/local20 and native local/SSH all-target Clippy pass. Real local
and authenticated SSH selection ack/highlight/copy/clear and invalid recovery,
64-slot pressure, post-admission cancellation and pending-ack cleanup pass. Pump
proof covers linear/reversed rectangle cells, repaint coalescing, reliable-event
order, foreign/released/inactive/expired rejection and last-good cancellation.
Local/SSH injected reliable-delivery failure returns EventsClosed through the ack
before owner error, retaining last-good. Full native local: 53 pass/2 existing
conhost 0x6d orphan failures/3 ignored. Full gate passes fmt/both workspace Clippy
variants then stops at the unchanged core PowerShell Enter guard; later steps
are not reached. No GUI, Unix or production engine-switch acceptance is claimed.

### Viewport baseline (US-0233)

US-0233 adds `NativeLocalSession::scroll(Scroll)` and direct
`NativeChannel::scroll(Scroll)`: synchronous admission to the existing 64-slot
control FIFO, then native scroll/capture/repaint without subsequent output.
Top, Bottom, Delta (negative toward older history) and Row (native absolute
history offset, not a RowId) preserve real native viewport metadata and rows.
Prior reliable events deliver before scroll/capture; consumer-owned repaint
release and independent damage readers are unchanged. SharedModel::scroll is a
low-level mutation only, not permission for UI capture outside the pump.

US-0233 proof: six new tests; native pump history/damage/coalescing/event order/
cancellation, real ConPTY session and authenticated SSH loopback scroll without
new output, UI-pressure FIFO progress and full/closed admission assertions.
Native SSH142/terminal322/local18 and native local/SSH all-target Clippy pass.
Full native local: 51 pass/2 existing conhost 0x6d orphan failures/3 ignored.
Full gate again passes fmt/both workspace Clippy variants then stops at core's
unchanged PowerShell Enter guard. This is not platform wheel policy, automatic
bottom-on-input, selection/search navigation or production GUI/render acceptance.

### SSH paste baseline (US-0232)

US-0232 extends `NativeChannel::paste(&str).await` to the explicit direct SSH
endpoint, using the same shared native pump/policy as US-0231. Paste shares the
64-slot key/mouse/resize FIFO with a separate 4-MiB pending/active text budget.
Unpolled futures do nothing; pre-admission rejection recovers; after admission,
cancellation requests independent close through the existing token/transport
flag even with full UI and command queues. RAII releases text once on success,
error or teardown. Acknowledgment means original FIFO enqueue, not peer receipt;
there is no side writer, capture, retry or retraction of submitted bytes.

US-0232 proof: four new tests; authenticated loopback proves mode enable/disable,
plain and bracketed filtering, no paste capture, oversize recovery, control order,
raw-write progress, full text/control queue recovery and cancellation cleanup.
An isolated owner acknowledgment fixture holds the original transport FIFO full
to prove typed QueueFull/fail-stop/active reservation release without a drain race;
it is not physical network-failure proof. Native SSH140/terminal319, native local17
and native local/SSH all-target Clippy pass. Full gate passes fmt/both workspace
Clippy variants, then still stops at the unchanged PowerShell Enter guard. The
last full local suite's two conhost 0x6d failures remain unresolved. Direct-only
lifetime, full session/render/IME and OSC/security gaps still block production.

### Local paste baseline (US-0231)

US-0231 adds `NativeLocalSession::paste(&str).await`: live native mode lookup,
existing 1-MiB paste policy (plain newline normalization; bracketed marker/ESC/ETX
filter), one original-FIFO payload, typed acknowledgment without capture. The
same 64-slot control FIFO has a separate 4-MiB pending/active text budget with
RAII release. Constructing a future does not enqueue; cancelling after admission
requests independent close and cannot retract already enqueued bytes. Policy,
queue and text-budget rejection before admission is recoverable; transport
rejection after lookup fails closed and retains last-good, never retries.

US-0231 proof: 17 native local fixtures pass (12 owner, four session,
manual ConPTY). Native SSH136/terminal319 and native all-target local/SSH Clippy
pass. Full native local: 50 pass/2 existing conhost 0x6d orphan failures/3 ignored.
Full gate still stops at the existing PowerShell Enter guard after fmt/both
workspace Clippy variants. New proof covers unpublished mode changes/filtering,
oversize recovery, lookup cancellation, active/queued budget release, FIFO order,
transport rejection and a real session paste ACK. No production cutover, SSH
production paste, GUI/Unix or complete clipboard/IME parity is claimed. US-0232
above now adds the explicit staging SSH paste seam.

US-0229 adds validated key/mouse/resize requests to one 64-slot control FIFO.
Controls execute between bounded reads/captures even during sustained output.
Native encoders use live modes and send bytes through the original local FIFO;
raw writes can overtake controls while reliable UI delivery waits. Resize updates
native state, consumes the original pending-size slot, updates physical PTY with
the requested cell metrics, then captures. A later control cannot replace that
physical resize. Errors after mutation fail-stop; no partial frame or rollback.
Independent close drains controls regardless of capacity.

US-0230 exposes `NativeLocalSession::spawn(cfg, initial, security)`. It shares
guarded shell Options construction with LocalSession, forwards explicit security
policy and implements existing TerminalLifecycle: single take_events, liveness,
close, identity/title/cwd. Native model, write/key/mouse/resize, repaint release
and async wait are explicit methods. No old parser is constructed. It deliberately
does NOT implement full TerminalSession with fake render/input/IME defaults;
production factory, native content/selection/search/IME/paste, configurable
scrollback, logging, real platform key mapping and remaining OSC/security parity
are still open. Policy forwarding is not missing clipboard-family parity proof.

US-0229/0230 baseline proof: ten owner tests, three native session tests and the manual
ConPTY fixture pass. Native full local suite: 47 pass/2 existing conhost 0x6d
orphan failures/3 ignored; default local suite: 33 pass/2 same failures/3 ignored.
Native local Clippy, SSH136/terminal316 pass. Real fixtures prove child/native
100x30 resize, key ACK, bounded backpressure release/close and continuous-output
control progress; mock proves state-derived DECCKM/SGR mouse FIFO and physical
resize failure retaining last-good. Session proof covers resolved env/cwd,
sanitized title/OSC7 cwd, one event consumer, native controls and ordered exit.
Full gate still stops at the unchanged PowerShell Enter guard after fmt/both
workspace Clippy variants. No production GUI or Unix acceptance is claimed.

## Autonomous local owner (US-0228 baseline)

US-0228 adds `oneterm_local_shell::ghostty_local::NativeLocal`, a hidden explicit
default-off staging endpoint, not LocalSession or a factory selector. Its PTY
owner thread autonomously reads/feeds/captures and services the original bounded
FIFO with partial writes while native/UI futures wait. Future wakeups notify the
same Poller; read interest is suspended during waits, then draining resumes.
One 64-KiB buffer, no additional output queue or performance claim. Windows live
empty-ring reads are not EOF. That slice exposed no controls; US-0229 above adds
them. Logging remains unsupported.

Close/Drop/cancelled polled wait independently requests shutdown, with no UI join.
Teardown closes admission, reclaims queued/in-flight budget, deregisters/drops PTY
on its owner and waits for native destruction before final Closed. Normal delivery
waits for UI; explicit close may return LifecycleDelivery::Cancelled. Startup
failure arrives through wait. Completion is resource-teardown evidence, not an
OS-thread join, deadline or forced-drop orphan guarantee; Unix is unverified.

Six native owner tests and the earlier manual ConPTY fixture pass. Full native
local suite has 40 pass/2 existing conhost 0x6d orphan failures/3 ignored. Native
local Clippy and SSH 136/terminal 316 pass. Full gate still stops at the existing
PowerShell Enter guard after fmt/both workspace Clippy variants. Validate with
the same environment below and `cargo test -p oneterm-local-shell --features
ghostty-native native_local_owner`. Production session/content, local controls,
remaining parity and real GUI acceptance still precede application cutover.

## Pinned inputs

### Cloud native desktop artifact

Use GitHub Actions **Native Windows Build** (`.github/workflows/native-build.yml`)
and **Run workflow**, or push relevant source/build changes to main. No local
compilation is required. The workflow runs the bundled workspace quality gate,
verifies pinned Ghostty/Zig downloads, builds the native library, runs strict
native workspace Clippy and tests (including Tab dispatch), then builds the
`oneterm-native-desktop` release-profile binary. Existing cross-platform CI
remains unchanged. A failed gate prevents runtime artifact upload.

On success download `oneterm-native-windows-x64-<commit>` from that run's
Artifacts within 14 days. It contains a ZIP and its SHA256 sidecar. Extract
the whole ZIP before running `oneterm-native-desktop.exe`; keep `conpty.dll`
and `x64/OpenConsole.exe` beside it. Notices, the Ghostty license, native
library provenance and commit/run metadata are included. Package entries
are checked against an explicit allowlist: caches, sources, PDBs and user
runtime data are excluded.

This is the isolated native acceptance entry, not a production cutover. It
creates EXE-sibling `oneterm-native-data` at runtime; existing connections,
credentials and settings are not packaged or migrated. Do not replace an
existing installation or delete its runtime data. No version bump, tag, GitHub
Release or automatic updater publication occurs. Automated CI is not real GUI
acceptance; until an actual cloud run passes, this workflow is unverified.

- Official Ghostty source: `fbb6dbe7ea5c1ce6df14ef82e00779a568cdb2a2`.
- Source tarball SHA-256:
  `2ff4f0aaf595bc07d36b4a08cf3e85a52581e3be792b295ae34a5db481197f99`.
- Zig: `0.17.1`, Windows x86_64 archive SHA-256
  `b03f477a1943673e02aca0f570e8030dffd536ce1f45cbc4d3326ec31159562e`.
- Library target: `x86_64-windows-msvc`, upstream static library and matching
  installed C headers. Do not substitute headers from a different revision.

Keep downloads, extracted sources, toolchain and build outputs inside `.tmp/`.
No system toolchain installation or floating dependency is required by the probe
runner. Native library build/distribution integration is still under evaluation;
the normal Cargo build does not fetch or link Ghostty. An explicit staging
feature links the native owner for tests; it does not select a production engine.

Build from the validated source archive, then run the probe:

```powershell
pwsh -NoProfile -File scripts/ghostty-vt/build.ps1 `
  -Zig .tmp/ghostty-migration/zig-x86_64-windows-0.17.1/zig.exe `
  -SourceArchive .tmp/ghostty-migration/ghostty.tar.gz

pwsh -NoProfile -File scripts/ghostty-vt/probe.ps1 `
  -Zig .tmp/ghostty-migration/zig-x86_64-windows-0.17.1/zig.exe `
  -Prefix .tmp/ghostty-migration/install
```

The runner compiles `probe.c` with warnings as errors and executes it. It checks
every two-chunk split boundary and byte-at-a-time parsing, UTF-8 display width
through DSR, cursor state, parser ground/error state, bounded owned reply capture,
resize, snapshot text/styles and snapshot independence after terminal mutation.
It never starts a shell or opens a PTY. This is native API evidence
only: it does not prove GPUI rendering, search, graphics, OSC policy,
production Rust ownership, SSH/ConPTY integration, release packaging or GUI acceptance.

The runner also runs the actual `oneterm-terminal` crate tests with the
`ghostty-native` feature against the same static library. The obsolete standalone
rustc fixture was removed: published viewport and gesture ownership require the
real crate graph rather than a partial module copy. These tests exercise moving the
Rust owner without moving callback userdata, owned reply draining, bounded reply
overflow, invalid resize rejection, and repeated create/write/drop cycles. Its
handle is deliberately neither Send nor Sync until native thread migration is
audited. This is not production engine selection or a substitute for pump/GUI
integration. The C probe statically checks the enum values used by this narrow
Rust boundary against the matching headers.

## Cargo boundary validation

After the native build and C probe pass, enable the explicit staging feature:

```powershell
$env:GHOSTTY_VT_PREFIX = Join-Path $PWD '.tmp/ghostty-migration/install'
cargo test -p oneterm-terminal --features ghostty-native
cargo clippy -p oneterm-terminal --all-targets --features ghostty-native -- -D warnings
```

Use the project's Cargo environment as for the normal gate. The build script
checks the project boundary, target, installed revision marker and library
presence; the native build script validates the source archive and emits the
library SHA-256 in `oneterm-build.json`. The marker alone is not an integrity
signature. Run the C probe against matching headers whenever bindings change.
No native owner is constructed by a production session yet. Do not interpret
this feature as completed renderer, transport or security-policy adaptation.

The same standalone Rust runner now also imports the owned viewport projection
and its tests. The C probe guards the render keys and mirrored struct sizes,
alignments and offsets; Rust tests check their corresponding layouts. Viewport
fixtures cover combining/ZWJ graphemes (mode 2027), wide cells, split writes,
colors/SGR flags, background-only erasure, wrap/cursor state, alternate screens,
independent consumers, size/text budgets, and owned frames after resize/drop.
Captures currently rebuild native render handles and copy full viewports to
avoid stale cells between consumers. This is a correctness staging path, not
production damage tracking or a performance claim. Hyperlinks, images, history,
production selection/search and the asynchronous clipboard bridge are still pending.

## Dedicated worker staging

US-0209 adds `ghostty/owner.rs` to the same standalone runner. The movable
EngineOwner endpoint creates, uses and drops native handles on one named worker;
the native handles remain !Send/!Sync. Methods block and belong on backend
workers, not GPUI or a tokio executor thread. Feed accepts at most 64 KiB whole,
uses one request slot and returns ordered owned replies; the caller must deliver
them to the originating transport before capture/UI notification. Resize does
not resize the transport. Explicit close joins; Drop only disconnects/detaches.

Explicit capture publishes one shared Arc Frame. Readers keep independent damage
watermarks, including skipped publications, and never wait for native parsing or
capture. Full owned-frame comparisons are staging work, not a performance claim.
Fatal failures retain the last complete frame with failed status. Disconnected
requests join before reporting errors so worker panic cannot race into a normal
closed error; native failure takes priority over abandoned response delivery.
Fixtures cover thread transfer, replies, request bounds, fan-out/damage, fatal
overflow, abandoned responses, panic and active-worker Drop. No production
model, pump, local transport or SSH transport uses this endpoint yet.

US-0210 adds the default-off `backend::GhosttyPump`, which uses the existing
PtyTransport trait and privately owns the endpoint. It chunks large reads,
enqueues replies between chunks and captures only at explicit finish_batch.
Rejected replies, native failures or failed transport resize stop the pump;
consumed output is never retried or partially published. Close requests are
out of band; failed cleanup is reported/logged, not assumed successful. Resize
reports are delivered after transport resize and before capture. The native
Cargo suite tests this seam with real Ghostty state and a fake transport; the
standalone std-only Rust probe does not compile the backend pump. This remains
blocking staging, not actual local/SSH session cutover or proof of physical reply
delivery. US-0211 adds bounded title/PWD/bell capture and existing-router policy:
256 native events/64 KiB text, the same aggregate pending-batch bounds, reliable
delivery outside native work before capture, then coalesced Output. Explicit
close flushes pending reliable events. The caller's render path must acknowledge
the shared repaint hint; FrameReader does not do it automatically. Callback
strings never escape native scope; ownership/overflow/split tests run in both
Cargo and the standalone runner. Clipboard and other OSC families remain open.
Native parsing ignored 2048-byte titles in testing: callback capture is not
proof of long-OSC parity and must not justify a production switch yet.

US-0212 adds the Cargo-only application row projection (`ghostty_projection`),
which deliberately is not part of the standalone FFI ownership runner. It lends
the same RenderRow values the shipping GPUI cell reader consumes; native keys
compare exact immutable content and expose no fabricated ordered history id.
Run `cargo test -p oneterm-terminal-view --features ghostty-native render::frame`
with the same prefix to exercise actual native rows through that reader and its
text scanner. This is not whole-frame/session integration: native modes,
history anchors, selection, hyperlinks/images/semantic marks, theme/contrast
intent and production backend selection still need adaptation.

US-0213 extends the standalone probe with typed native modes, active screen,
Kitty keyboard flags, scrollbar and serialized viewport operations. C guards
pin the metadata and by-value scroll union layout. Tests cover real history
scrolling, skipped publication damage, cursor-only damage, alternate-screen
keyboard flags and reset; capture rejects inconsistent scrollbar height.

US-0214 adds Cargo-only transactional paint preparation. ProjectedFrame retains
the exact immutable source Arc with its paint rows; ProjectedReader prepares
before committing damage and retains its last good frame on projection failure.
An unchanged capture reuses the projection, while lifecycle remains observable.
This allocating preparation belongs on backend workers, not GPUI prepaint or
tokio executor threads. The std-only runner tests the underlying reader
transaction but does not compile application projection. These seams still do
not replace TerminalContent or either production local/SSH session engine.

US-0215 adds owned native key/mouse requests, transient thread-confined RAII
event/encoder handles and live set_from_terminal mode reads. Key text is bounded
to 4096 bytes; encoded results to 64 KiB. The pump delivers once or fail-stops;
invalid local requests do not write or poison it. Paste retains OneTerm's 1 MiB
cap/sanitization with native mode 2004, not Kitty clipboard paste events. The
pinned SGR-pixel encoder emits terminal-space pixels without a one-based cell
offset. Buttons Ten/Eleven have no wire encoding upstream and yield empty bytes.
Fresh per-event encoders do not provide motion deduplication; the owned physical
key contract is not a complete GPUI/platform key mapping. Input mode/lifetime,
bounds, named-key, wheel/button, padding/modifier and paste-mode tests run in the
standalone probe as well as Cargo; transport/paste-policy tests are Cargo-only.

US-0216 adds worker-owned tracked cell anchors and opaque Rust-only tokens, not
native pointers or ordered RowIds. At most 256 handles live per worker; explicit
release never revives a stale token. Resolve names a coordinate space and returns
an optional coordinate with the owning screen identity. Foreign/released ids and
invalid points are recoverable. The native point API omits some bounds, so the
adapter checks native round trips, excludes active cells from history, and caps
active/viewport rows at native terminal height. Tests cover scroll, bidirectional
reflow, original screen ownership, history erasure/reset, token provenance/cap,
and freeing after terminal destruction. The pump exposes serialized queries but
does not select a production model or provide production selection/search yet.

US-0217 adds inclusive linear/rectangle selection from two validated active-screen
anchors, native terminal-owned tracking after caller anchor release, active clear,
and bounded plain/unwrap/trim copy. Missing selection returns None; blank selected
text returns an empty string. Copy queries size before allocation, rejects output
over 1 MiB without partial text, and never writes a system clipboard. Invalid,
foreign, released, expired or inactive-screen endpoints and copy overflow leave
the worker/pump healthy. The native screen switch clears the destination screen's
selection; no inactive-screen selection is restored by the adapter. C/Rust guards
pin selection layouts and formatter option values. Native fixtures cover forward
and reverse text, rectangle, Unicode, soft wrap, reflow, history, screen ownership,
anchor release and actual UTF-8 copy overflow. Cargo-only projection prepares exact
half-open viewport spans from the same source Arc, independently from paint cache
keys; tests cover selection-only damage, independent readers, clear and explicit
batch publication. Production gestures/overlays, search and async integration
remain pending. These tests still do not exercise a real local/SSH session.

US-0218 adds a worker-owned native incremental search. Set/clear and each step
feed once and tick at most once, returning explicit progress and owned inclusive
Screen-space endpoints (newest first), never durable RowIds/native pointers.
Needles are capped at 4096 UTF-8 bytes; complete response lists at 4096 matches,
with explicit recoverable rejection instead of truncation. This is a response
bound, not a cap on native internal search memory or a wall-clock deadline.
Native matching is ASCII-case-insensitive and may cross soft wraps; case-sensitive
and whole-word flags are explicitly unsupported, so legacy search parity remains
open. History, reflow, screen switches, reset, Unicode and error recovery are
tested in the standalone runner; pump ordering/no-publication is Cargo-only.

US-0219 adds Cargo-only `ghostty_bridge::AsyncEngine`. It starts a blocking
dispatcher off the caller, uses eight bounded command slots and one response slot
per accepted request, and returns Busy immediately instead of accumulating waiting
send futures. Native handles remain on their existing owner worker. Close closes
admission immediately, cancels queued work, finishes in-flight work, then awaits
native destruction/join. Dropping the backend initiates close even if models live.
Cancelled interactions are not replayed; unconsumed nonempty replies/reliable
events stop the bridge instead of being silently lost. Buffered replies are also
checked on Pending drop. An abandoned newly created anchor is either released on
the worker or triggers cleanup by shutdown. No transport is driven by this bridge;
the backend must deliver returned replies/events before explicitly requesting
capture. This is not the async transport-pump or clipboard callback integration.

US-0220 adds Cargo-only `ghostty_model::SharedModel`: asynchronous scroll, anchors,
selection/copy and search against that same owner, and prepared shared frames.
Only the backend endpoint exposes feed/replies/capture. Capture and paint
projection run on the dispatcher, publishing a complete Arc only after success;
each model reader compares its own source frames for damage. Poll performs no
native access or projection, but row-damage comparison may allocate and is not an
optimized/zero-allocation renderer claim. Close/failure retains last-good paint
with explicit lifecycle. Tests cover queue/close barriers, manual pending Future
poll, cancellation, abandoned reply delivery, panic/native/projection failure,
shared frame identity, independent damage and selection/search/output ordering.
This model does not implement the shipping TerminalModel/TerminalSession contract
or connect either real backend, and it intentionally fabricates no legacy RowIds.

US-0221 adds Cargo-only `backend::AsyncGhosttyPump`. It privately owns AsyncEngine
and uses the existing nonblocking PtyTransport contract. Output is chunked at
64 KiB; each native reply enqueues on that pump's originating transport before
events are drained or the next chunk runs. Bounded pending title/directory/bell
events reuse policy and await reliable queue capacity before prepared capture;
only then is coalesced Output posted. A closed reliable queue fails the batch.
Unlike the shipping sink's log-and-count policy, this explicit staging checked
send cannot silently lose an event and then publish a successful native frame.

A guard spans active operation awaits. Cancellation/failure closes admission and
transport without joining on the caller, prevents later prepared publication and
retains last-good paint. Cancellation is not rollback: already queued events,
cache updates and a frame committed before cancellation remain observable.
Local validation errors do not close a healthy pump. After an operation starts,
queue Busy/native/delivery errors fail-stop; no consumed output is retried.
An unpolled async operation has no effect. Graceful close flushes reliable events
without capture/repaint; `shutdown()` instead signals transport/admission before
its returned future is polled and discards pending events. Awaiting either waits
for native destruction on the dispatcher, not a blocking join inside Future poll.
Neither close path interrupts native code or promises a deadline.

The pump supplies SharedModel clones for async selection/search/scroll and frame
reads, but never exposes its raw engine endpoint. Fixtures cover reply-first
publication, FIFO/backpressure, invalid input, pending-event overflow, reliable
channel closure, transport/native/resize errors, cancellation of queued feed/
capture and graceful close, immediate shutdown/native join and live model/input
interaction. This is still a fake-transport staging integration: real local/SSH
read loops, production session/content contracts, paste bridging, clipboard and
remaining OSC/graphics/theme/input/search parity are not established by it.

US-0222 adds real encrypted loopback SSH fixtures using the existing SshTransport
FIFO and strict temporary known_hosts helpers. Run with the same native prefix:

```powershell
cargo test -p oneterm-ssh --features ghostty-native native_ssh
```

The default-off SSH feature only forwards native linking. No production backend
selector is added. The peer observes prior input followed by the exact native
DSR reply; split remote CSI output reaches only Ghostty, and owned paint contains
the remote text. Two resizes coalesce to the last size, observed both by the peer
and the next native frame. A full reliable queue separately gates capture;
release permits publication, while cancellation retains last-good and closes
transport admission. The fixture then explicitly closes the russh channel and
checks peer receipt. This is real transport proof, not shipping ssh_main_task,
SFTP/session teardown, local ConPTY or GUI acceptance. The existing production
loop still needs independent close wakeup while awaiting reliable UI delivery.

US-0223 adds Windows real ConPTY child proof with the same native prefix:

```powershell
cargo test -p oneterm-local-shell --features ghostty-native native_local_pty
```

This default-off feature only forwards native linking, not backend selection.
The fixture owns the original PseudoConsole on its worker, drives the original
LocalTransport/ShellNotifier FIFO and AsyncGhosttyPump, and constructs no legacy
terminal state. Native paint shows child readiness and ACK for two FIFO input
writes; byte reservations return to zero. Two sizes coalesce to 100x30, observed
by the child's console API and the next native frame. Child exits with success;
explicit native shutdown closes notifier admission. A registration guard removes
poll sources before owner-thread PTY destruction, including failed assertions.
I/O waits are bounded; this does not impose a native-operation deadline.

This proves Windows child input/output/resize/exit, not Unix openpty, raw DSR
(ConPTY can consume VT itself), forced-drop orphan cleanup or shipping
ShellEventLoop/session/GUI cutover. The existing two conhost orphan fixtures still
fail with 0x6d both with native enabled and in the default build on this host.

US-0224 adds `oneterm_ssh::ghostty_channel::NativeChannel`, available only with
ghostty-native. It accepts an already authenticated direct client handle and an
opened shell channel; it does not connect, authenticate or select an app engine.
The owner task autonomously reads remote data, drives AsyncGhosttyPump and drains
the original transport FIFO even while reliable UI delivery is pending. Explicit
close uses an independent cancellation token, not a command queue slot. Drop or
cancelling a polled wait requests close. Single teardown closes admission, drains
reserved writes, closes the physical channel, awaits native destruction, then
drops the direct connection. An already committed frame/event is not rolled back.

```powershell
cargo test -p oneterm-ssh --features ghostty-native native_owner
```

Eight owner fixtures prove automatic remote paint/reply, UI release/publication,
close with both UI/command queues full, Drop/cancelled wait, idle close, native
failure, reliable-event loss and unpolled write reservation cleanup. Peer exit
status was initially only returned in the task report; US-0227 below adds reliable
lifecycle UI events.
That initial endpoint had no resize/physical input command surface (extended by
US-0225/0226 below) and does not own jump carriers/SFTP/tunnels. Shipping TerminalSession, ssh_main_task,
factory, local loop and GUI remain unchanged. Native destruction and SSH close
have no deadline guarantee; test deadlines are only failure detection.

US-0225 extends that endpoint with `key`/`mouse` admission. Callers supply actual
physical key codes and layout text, or validated mouse geometry, not guessed
GPUI mappings. A separate 64-slot input FIFO validates before admission (at most
4096 key-text bytes per request). The same owner encodes at completed batch
boundaries using current Ghostty modes, then queues bytes in the original SSH
FIFO. Raw writes may overtake pending physical input; admission is not delivery.
Full/closed/invalid input returns a typed error. UI pressure defers physical
input; explicit close drops pending input without encoding/replay. Four added
real owner fixtures cover mode-derived key/SGR mouse bytes, invalid rejection,
full input queue close, and resumption after UI release. Resize, platform mapping
and shipping session integration were still open at that slice.

US-0226 adds `resize(cols, rows, cell_width_px, cell_height_px)` to the same
64-slot control FIFO. Viewport dimensions are validated against the native
capture budget before admission. The owner applies native resize, drains the
original transport's resize work and only then captures. A bounded FIFO barrier
preserves earlier encoded input even when the pump completes before the resize
marker is consumed; full-queue marker suppression also cannot strand a resize.
Physical window-change submission is not remote acknowledgment. Raw writes may
overtake queued controls; endpoint controls remain FIFO. A physical resize error
stops teardown with its typed cause and retains last-good without resized capture
or replay. Six new fixtures prove real dimensions/input order, invalid rejection,
UI-release resumption, full control queue close, dead-writer failure and the
resize barrier with/without a marker. Platform mapping, lifecycle events,
session/local-loop integration and shipping selection remain open.

US-0227 adds native `publish_exit` and NativeChannel lifecycle adaptation.
Observed exit updates the state cache (including clearing a prior code on signal)
and reliably sends Exited after earlier policy events, without capture. Reading
continues for late remote output. Final Closed is sent after command/control
admission closes, queued budget drains, physical channel closes and native
destruction completes. It normally waits for bounded UI capacity; this wait
does not retain native handles or an open shell channel. The direct connection
handle is retained until the task result, including final UI wait.

Explicit close/Drop/cancelled wait still wakes delivery. Immediately ready Closed
delivery wins over cancellation; a blocked lifecycle send is cancelled and
`Report.lifecycle` is `LifecycleDelivery::Cancelled`, not delivery confirmation.
If Exited was cancelled earlier, that outcome persists even if Closed fits.
Event loss returns a typed error; an original native/SSH failure wins over final
delivery failure. No pending lifecycle task or hidden buffer is introduced.
Seven added real owner fixtures and three native pump fixtures cover late output,
status/signal order, post-teardown UI pressure, exit/final cancellation, Drop/
cancelled wait and lost final events. Native session/content/local-loop work,
peer-close monitoring under UI pressure, hops/SFTP/tunnels and GUI remain open.
