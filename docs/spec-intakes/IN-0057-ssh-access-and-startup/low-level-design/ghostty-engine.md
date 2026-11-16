# Detail Design: libghostty-vt application engine

## Accepted boundary

US-0208 replaces the application parser and terminal state, not its dependency
spelling. Ghostty owns incremental parsing, screen/history, cursor and modes.
ConPTY/openpty and russh retain process/channel ownership, resize and I/O. The
existing TerminalPump batches events and sends replies to the originating
transport. GPUI continues to draw owned terminal frames.

The external oneterm-vt crate, its optional transport and its package/API gate
remain intact. Reusing pure value types or a separately driven image decoder is
not permission to keep its Terminal running alongside Ghostty in production.

## Native build

### Isolated desktop acceptance owner (US-0245)

The app's default-off native-desktop feature enables only the explicit local
native adapter and native surface. Its separate binary never calls normal run(),
installs no production factory, and performs no application persistence startup.
Owner UAT amendment (2026-11-13): accept an empty command line, with no elevation
opt-in flag or administrator-specific mode. Reject unexpected arguments. Do not
gate startup on token classification: keep the true classification in global
elevation state for underlying checks, without pretending an elevated/unknown
token is ordinary. The shell inherits its caller's privileges; entry/configuration
isolation is not an OS sandbox. No self-elevation, automatic commands, persistence
or extra OSC effects are enabled. One title/banner applies to every launch. A run
under an elevated token still does not prove ordinary-token behavior. Use default
local-shell configuration with the launch directory explicitly supplied. Full
workspace integration is separate (US-0249), not fulfilled by this gate removal.

One dedicated blocking event-consumer thread owns the SessionEvent receiver;
it never calls GPUI or waits for painting. It reduces events to bounded status and
a coalesced dirty flag. Clipboard writes/reads and desktop notification effects
are denied regardless of local policy defaults; terminal title/cwd/status cannot
change OS state. No event payload history or unbounded forwarding queue is kept.
The GUI periodically consumes the dirty flag, invalidates the surface and displays
bounded state. Input errors have a separate visible slot, never a silent retry.

The root owns the surface and a session Arc; its release hook requests close.
The probe uses explicit quit mode so last-window removal does not stop the executor
before detached surface cleanup runs. A detached close task retains the release
hook's Arc and waits until only itself and the launcher own session references,
then quits. Poll at 10 ms with a five-second deadline; timeout logs the remaining
references and quits into the post-loop ownership error, rather than hanging or
pretending cleanup succeeded. No admitted input operation is retried.
A launcher-owned Arc survives the GUI loop. After return, close is requested
again idempotently; GUI entities must release their references before consuming
NativeLocalSession::wait, while the independent consumer continues draining until
the sender closes. Report wait/consumer failures and observe native destruction;
do not claim thread-join or desktop acceptance based solely on a window opening.
Natural session closure remains visible until the user closes the window.

Session-end quit is a separate path: GPUI 0.3.9 waits quit observers for 200 ms
and may then exit the process without returning to the launcher. If app quit
begins before root release, transfer the launcher-owned session and event consumer
to one cleanup worker. Its observer awaits native destruction and consumer join
while GPUI releases windows. Root release requests close but does not start the
ordinary quit task again. A guard created synchronously before polling the quit
future makes cancellation, timeout and cleanup failure visible and exits nonzero;
only observed owner/consumer completion disarms it. This policy is isolated to
the probe and does not claim actual OS logout grace or five seconds for session
end. Ordinary WM_CLOSE retains its five-second reference drain and post-loop wait.

The isolated window intercepts the normal should-close callback: mark the GPUI
window removed and return false so DefWindowProcW does not destroy the HWND first.
GPUI's platform destructor remains the single native-window destruction owner;
Root release still drives the bounded native-session cleanup. Do not directly
destroy handles or suppress platform errors. Forced OS destruction is separate.

Real-close acceptance rework: the platform IME adapter must not own the native
surface entity. GPUI's ElementInputHandler holds a strong Entity, and Windows
can retain the handler after root release. Use a native-only weak adapter that
temporarily upgrades for each synchronous callback and delegates to GPUI's
ElementInputHandler. After entity release, return neutral query results and ignore
edits; never resurrect a session or replay input. Preserve all callback semantics
while alive. Test retained-adapter ownership and actual rendered Windows WM_CLOSE;
the five-second cleanup deadline and admitted-operation rules remain unchanged.

US-0243 linear drag uses the existing acknowledged selection FIFO. A gesture has
an opaque Rust identity and one owner-held tracked start anchor. Every begin/move
sample carries the last-painted Viewport; the dispatcher requires its opaque
grid-revision identity and no intervening grid mutation (including unpublished
output). Selection-only captures retain that identity so queued motion does not
expire merely because highlighting repainted. Revision identities are per-source
Rust allocations, never native pointers or numeric row versions.
Rejected samples never reinterpret old row indices. Begin clears selection;
move creates a temporary tracked end, sets native selection, releases the end,
and captures/repaints through the pump. Finish releases the start, preserving the
last selection. A stale sample is observable/recoverable; the UI ends the gesture
without replay. At most one start and one temporary end exist per owner. The
surface uses one async task, one replaceable pending sample and a separate finish
bit, so high-rate motion cannot grow a queue or overwrite finish. Blur/outside-up
finish at the last valid cell, with no off-grid autoscroll. New gestures reject
while cleanup is pending. Dropping an admitted task retains session fail-stop.
View teardown signals finish and detaches the single gesture task to release an
idle start anchor too; it does not abandon an acknowledged idle gesture. The
embedding caller still owns reliable-event draining and session shutdown.
Left drag is local when mouse reporting is disabled, Shift overrides it, or
the live native Drag Priority preference is Selection First;
already-admitted mouse presses keep their release obligation. Word/line/rectangle
selection and search remain separate. This is not a production selector.

US-0242 clipboard editing uses one in-flight GPUI task per native surface. Copy
reads bounded live native selection text; absent/empty selection preserves the
clipboard. Paste reads text once and calls object-safe SurfaceSession visible
paste, not raw IME commit. Platform clipboard chords and TerminalCopy/TerminalPaste
actions are consumed without native key/IME duplication; held chords do not repeat.
Busy edits reject observably rather than retaining tasks/payloads. Visible paste
uses the existing bounded endpoint paste control/reservation with a reveal flag:
validate/filter and choose live native mode first, then flush reliable events,
scroll primary viewport bottom, capture/repaint and enqueue encoded nonempty bytes
in the original transport FIFO. Empty or rejected paste changes nothing. Existing
low-level paste remains capture-free. Cancellation after admission retains the
endpoint's fail-stop rule; no retry or broadcast is introduced. Pointer selection,
search navigation, settings/menu policy and production cutover remain separate.

US-0241 wheel requests carry signed nonzero steps in -10..=10 (positive up/older)
and the same validated device-grid pointer coordinates as reporting. One owner
operation reads live metadata and applies this priority: primary scrolled history
or Shift scrolls primary history; otherwise active mouse tracking encodes native
button Four/Five press events (no wheel release); otherwise alternate screen with
mode 1007 encodes native ArrowUp/ArrowDown keys; otherwise primary scrolls history,
and alternate screen is a no-op. Native encoders own modifiers and cursor modes.
The pump flushes reliable events before the operation, delivers bytes through the
original transport FIFO, and captures/notifies only the history route. Cancellation
is fail-stop, never replay. The surface accumulates finite vertical line fractions,
resets on missing/stale geometry or blur, caps whole steps to ten per event, drops
excess whole steps, retains only the fractional remainder and does not retry failed
admission. Horizontal deltas/settings binding/desktop UAT remain out of scope.

US-0240 adds typed pointer admission to SurfaceSession and the independent native
surface. GPUI grid hit tests retain fractional cells, then convert to a padding-free
device-pixel grid using the exact prepaint resize cell metrics. Absent/differently
sized publications suppress new pointer input until layout and publication agree.
The UI does not choose or encode terminal mouse protocols; the live owner encoder
does. Shift bypasses press/motion for future local selection; admitted button
ownership still releases once on mouse-up/blur, including outside the grid using
the last valid coordinates. Failed admission is observable and never implicitly
retried. This slice does not implement wheel/selection/menu/clipboard semantics or
production factory selection. The caller still owns events/security and refresh.

US-0239 introduces SurfaceSession, a minimal object-safe native surface contract
implemented by explicit local/SSH endpoints. It combines RenderSource, atomic
resize, typed key admission, raw IME composition/commit and repaint-hint release,
without inventing legacy query/history/selection contracts. Published queries
clone one prepared viewport/state under the publication lock and do not consume
damage, capture or acknowledge delivery. They are last-published state, never a
replacement for the live owner encoder's modes.

The opt-in NativeTerminalSurface embeds the existing TerminalElement with a direct
native session, not FakeTerminalSession or legacy TerminalSession. The caller
retains lifecycle/reliable SessionEvent/security handling and explicitly invalidates
the surface on output. The surface never takes or drops that event stream. Native
functional keys use owner encoding; layout keys lacking physical platform codes
use UNIDENTIFIED, never guessed letter positions. Plain printable text stays
on the GPUI IME path on both primary and alternate screens, so TUIs can open
platform candidates. Screen-buffer changes do not disable composition or
candidate geometry. Functional keys and control chords retain native encoding
outside composition; active composition owns candidate navigation keys.
IME state/errors retain the session bounds and clear-before-write semantics.
Candidate placement conservatively anchors every UTF-16 preedit range at one
published native cursor. It does not simulate Ghostty grapheme layout with the
legacy engine's width tables. Exact range geometry and inline preedit painting
remain deferred. Application shortcuts, broadcast, paste,
selection/search, wheel/mouse policy and production panel selection remain open.

US-0238 connects the actual GPUI Frame to an optional native render source on
TerminalRender (default None). Prepaint polls a persistent per-view Content before
any legacy snapshot. Native rows and damage feed the existing PlanCache; cursor,
reverse-video and exact half-open selection spans come from that publication.
Before publication the paint grid is empty, not an invented legacy terminal.
Legacy keyboard modes are explicitly unavailable for a native frame: the old
encoder must not run against stale defaults. Native GUI interaction dispatch and
complete native TerminalSession implementations remain subsequent work, so this
hook cannot authorize production selection. Native graphics and legacy absolute
gutter identities are unavailable, not claims of implemented parity. Poll failure
retains last-good paint state and is logged; it never falls back to legacy state.
Source changes compare publication ownership and reset reader/cache damage. Native
steady cursors ignore the application blink-off phase. Captured effective padding
background and optional cursor RGB are used by the actual painters; explicit
configured cursor color still wins. This is not production theme setter mapping.
Pinned native render state already reverses default background/foreground for
reverse-video; projected RGB colors must not be swapped a second time.

US-0237 introduces a shared object-safe native RenderSource, implemented by the
explicit local session and direct SSH endpoint. Each consumer opens its own
Content/ModelReader. Polling reads one prepared publication and computes only
that consumer's damage; the typed borrowed Viewport exposes rows/cursor/native
metadata/colors/selection from that one Arc. Before any frame it is None, not a
fabricated power-on snapshot. Closed/failed state retains last-good content.
Display-row window queries clamp to that capture, allocate no cell buffer and
never poll/capture/ack repaint. No legacy RowId or lossy ModeSnapshot conversion
is allowed. Complete GPUI session selection remains separate from this content
boundary; no missing graphics/links/semantic state is silently implemented here.

US-0236 keeps IME preedit in shared session-adapter composition state, never in a
second terminal parser. Explicit local session and direct SSH endpoint expose
owned set/read/clear and typed commit. Preedit and single commit each have a 1-MiB
UTF-8 bound; oversize rejects before changing preedit or admitting bytes. Valid
commit clears preedit before one exact raw UTF-8 transport write, matching the
shipping PtySession clear-before-write order. QueueFull/Closed remain observable,
without retry, preedit restoration, paste encoding or automatic shutdown. Success
means original FIFO admission only. Composition edits do not publish native frames;
the future GPUI adapter must request redraw and implement ranges/candidate bounds.
No shipping TerminalIme trait or production factory selector changes in this slice.

US-0236 Unicode follow-up proves local combining/Japanese and supplementary-plane
commits arrive intact at child ReadConsoleW, independent of echo. A same-session
.NET Console.ReadLine comparison still returns wrong codepoints; native input
delivery proof is not a claim of complete .NET console-reader compatibility.
No workaround or production encoding policy is inferred from that discrepancy.

US-0235 adds owned default colors (foreground/background/optional cursor and all
256 palette entries). The native owner applies pinned default options 12-15,
without feeding synthesized OSC or confusing effective override values with
defaults. Existing native OSC overrides survive; reset reveals latest defaults.
Palette replacement can fail after other setters, so any error is fail-stop with
last-good publication, not rollback. Both session endpoints share the existing
64-slot acknowledged control FIFO and cancellation guard. Prior reliable events
deliver before setters; capture/repaint follows all setters, and ack confirms
publication, not paint. No 16-to-256 production theme mapping or color-scheme
callback is inferred; original native color replies still use the transport FIFO.

US-0234 adds typed selection set/clear updates to both session control FIFOs with
bounded one-slot acknowledgments. The owner flushes prior reliable events, applies
the existing tracked-anchor inclusive selection and captures selected cells, then
posts the existing coalesced Output hint. Success acknowledges publication, not
paint/clipboard I/O. Native Selection validation errors (foreign/released/expired/
inactive endpoints) finish the guarded operation without capture or fail-stop;
they leave the previous selection/frame intact. Only that typed rejection is
recoverable; native/projection/delivery failure fails closed and retains last-good.
Future construction does not admit work; cancelling after admission independently
closes, with no replay or rollback of already applied mutation. Set/clear share the
existing 64-slot FIFO and cancellation guard used by paste. SharedModel selection
mutation/copy remains low-level; session selection publication must use the owner.
No native pointer, legacy RowId, clipboard effect or mouse/semantic policy is added.

US-0233 routes explicit session viewport controls through the existing local/SSH
64-slot control FIFOs. Successful synchronous scroll means admission only; the
owner runs a guarded native scroll-and-capture even without more output. First
deliver prior reliable batch events, then mutate the native viewport, publish an
owned projected frame and post the existing coalesced Output hint. Native signed
delta is negative toward history; absolute Row is a native history offset, never
a legacy RowId. The consumer explicitly releases repaint hints. SharedModel's
low-level scroll remains mutation-only and cannot authorize a UI capture that
bypasses reliable delivery. Failures/cancelled active operations retain last-good
and fail closed; independent close drains waiting controls without replay. This
does not add platform wheel policy, auto-bottom-on-key, selection/search navigation,
configurable history or a production render/session selector.

US-0232 extends the explicit direct NativeChannel with acknowledged async paste
using US-0231's pump and live mode query. Paste shares key/mouse/resize's 64-slot
control FIFO and adds a separate 4-MiB pending/active text budget with RAII release.
Validate the shared 1-MiB policy before copy/admission. Future construction has no
effect; after admission a dropped future independently closes through the existing
transport flag and cancellation token. Control and transport queue pressure cannot
block close. Normal completion acknowledges original FIFO enqueue, not physical
peer receipt; no capture, side writer, replay or rollback of submitted bytes.
Preserve the original typed transport failure before teardown noise. Raw writes
may overtake controls during native/UI waits. Direct handle lifetime, auth/trust,
jump/SFTP/tunnel gaps and production selection are unchanged by this slice.

US-0231 adds live PasteMode to the async bridge and policy-aware pump paste. Read
mode from the serialized native owner, never a stale published frame; encode with
the existing 1-MiB paste policy, plain newline normalization and bracketed marker/
ESC/ETX filtering, then submit one atomic payload to the original transport FIFO.
No capture/UI delivery is needed to confirm enqueue. Transport failure is fail-stop;
oversize rejection before operation admission is recoverable. The default-off
local session exposes async paste in the same 64-slot control FIFO, with a separate
4-MiB aggregate text budget. Reservations include active text and release by RAII.
Future construction does not enqueue; after accepted control a cancellation guard
requests independent close and never retries. Completion acknowledges FIFO enqueue,
not physical delivery or rollback of already submitted bytes. A dropped result
receiver is not permission to send queued paste; cancellation closes the endpoint.
This adds no full TerminalSession/IME or clipboard-family parity claim.

US-0230 adds an explicit NativeLocalSession lifecycle adapter. Both local paths
share guarded resolve_shell and Options construction, including resolved env/cwd
and Windows program quoting. The native constructor passes the caller's security
policy to OscRouter::with_security. It owns one bounded event receiver, implements
TerminalLifecycle and exposes native owned frames and typed raw/key/mouse/resize
operations. Repaint hints have explicit release. It never constructs oneterm-vt
Terminal, never masquerades as the full legacy TerminalSession and exposes no
logging/scrollback/IME capability it cannot implement. US-0231 adds explicit local
paste as described above, not full TerminalSession. LocalSession and the
production SessionFactory remain unchanged in their engine selection. Native
full-session/content/render/selection/search parity still precedes cutover.

US-0229 adds one 64-slot key/mouse/resize FIFO to NativeLocal. Validate owned input
and capture dimensions before admission. Admission/closure share the existing
native gate. Execute controls at batch boundaries while raw FIFO writes still
run during native/UI waits and may overtake queued controls. Encoded bytes and
native replies use LocalTransport, never a side writer. For resize, mutate native
state, consume the original pending-size slot, resize physical PTY with the
requested cell metrics, then capture. Only one control executes at a time; a
later control cannot coalesce/replace an active physical resize. Errors after
mutation fail-stop with last-good publication retained, never false rollback.
Close closes/drains the control FIFO independently of capacity; no retry/replay.

US-0228 stages an autonomous local endpoint, not a session selector. One dedicated
PTY thread creates/registers/reads/writes/deregisters/drops the passive console;
the native bridge retains its separate thread-confined engine owner. A Future
waker notifies the same Poller. While awaiting native or reliable UI delivery,
the PTY thread services the original bounded input FIFO with partial writes but
does not read more output. Read interest is suspended during those waits to
avoid spinning on unread bytes; native completion resumes draining immediately
until WouldBlock, then rearms readable interest. Child status is retained during
waits and published after the currently drained batch. There is no extra output
queue. Public writes and admission closure share a native-only gate, so teardown
cannot miss a racing accepted reservation. No change to the shipping notifier.

Endpoint creation admits input before child startup; wait reports startup errors.
Close/Drop/cancelled polled wait set an independent request and notify the poller.
Teardown closes admission, releases remaining reservations, deregisters/drops PTY
on its owner, and awaits native destruction before final bounded Closed delivery.
Final delivery normally backpressures; explicit close can cancel it, recorded in
the report. Completion observes resource teardown, not an OS-thread join. No
deadline, forced-close orphan guarantee, resize/control API, Unix runtime proof,
logging parity or production session/factory integration is claimed by this slice.

Evaluate official Ghostty commit fbb6dbe7ea5c1ce6df14ef82e00779a568cdb2a2
with Zig 0.17.1. Match headers, binaries and bindings to the same commit; never
load an arbitrary system DLL. The pinned source declares library version
1.1.0-dev; pin the revision rather than assuming development ABI stability. Start with the upstream
Windows MSVC static target; upstream bundles compiler-rt and explicitly links
ntdll/kernel32, disables MSVC stack protector generation and Windows UBSan.
Prove actual build/link output before selecting the distribution mechanism.
All downloads, caches and tools for this investigation stay under project .tmp.
Cargo build scripts must not silently fetch or install a toolchain.

## Ownership and callbacks

One Rust owner frees each terminal exactly once. A boxed callback context stays
at a stable address until the terminal is freed. Callbacks copy borrowed bytes
into owned bounded queues; they never retain upstream pointers, unwind into C,
perform UI work, launch processes or lock the terminal recursively. Drain reply
and policy-event queues before releasing the pump batch. Unknown sequences need
an explicit byte bound; recognized sensitive OSC callbacks still pass through
OneTerm's existing security policy. Missing callbacks must not grant permission.

Audit upstream thread-affinity constraints before implementing Send. The existing
fair mutex alone does not prove a native handle can move threads. If required,
use a dedicated engine-owner thread rather than an unjustified unsafe impl.

US-0209 implements that dedicated-owner option behind the staging feature.
Create/use/drop Terminal and RenderBuffer inside a named native worker. The
transport-side owner endpoint is movable but not Clone; mutable blocking methods
serialize requests and responses, with one bounded request slot and at most
64 KiB of input per feed. Reject larger reads whole before copying/enqueueing;
transport loops must chunk larger reads and drain replies between chunks. Native
reply capture retains its 4096-byte per-feed bound; overflow is fatal rather
than silently splitting/retrying already-consumed output. Return owned reply
bytes for each feed/resize before another method can request publication. The
transport remains responsible for enqueueing those replies before capture/UI
notification; this staging endpoint does not perform transport I/O.

Only explicit capture publishes an Arc-owned Frame through a short-lived mutex
around an immutable publication pointer/status. Readers never invoke native
code or wait for parsing/capture. One latest publication is retained; callers
own any older frames they keep. Each reader compares its own previous owned
frame, including skipped publications, for full/row/cursor damage; no reader
consumes another's watermark. Global palette/default-color or dimension changes
require full damage. Cursor changes invalidate both old and new cursor rows.
Fatal native errors close the worker, retain the last frame with an explicit
failed status, and never publish a successful partial result. Startup handshakes
report initialization failures. Drop disconnects requests without joining;
explicit close joins only on a backend worker, after native destruction.
On request/response disconnection, the already-blocking endpoint disconnects
and joins before reporting status: unwinding drops response senders before the
outer panic handler publishes failure. Native operation failure takes priority
over abandoned response delivery, preserving its causal error in reader status.
This first endpoint is blocking; do not call it on GPUI or directly on a tokio
executor thread. Async transport integration and shutdown wiring remain cutover
work, not evidence provided by the standalone worker tests.

US-0210 adds a blocking staging pump in the existing backend layer. It privately
owns EngineOwner and uses PtyTransport, chunking reads to 64 KiB and enqueueing
each chunk's native replies before another chunk or explicit batch capture.
Only finish_batch captures. US-0211 now routes title/directory/bell events before
capture, and posts a coalesced Output afterward. Other reliable OSC families are
not adapted yet. Successful pty_write means FIFO enqueue, not physical delivery.
Reply rejection (including QueueFull), native operation failure or transport
resize failure stops parsing/publication, closes the engine and requests
out-of-band transport close. Never retry consumed output. Return the causal
typed error; readers retain the last complete frame and see engine closure or
native failure. Validate resize budgets before mutation, resize native state,
forward rows/columns to the transport, then deliver in-band resize replies.
Any failure after native mutation is fail-stop rather than a false rollback.
This seam is not selected by either production backend yet.

US-0211 captures title/PWD changes and bell using one fixed native arena:
256 records and 64 KiB text. Title/PWD callbacks use upstream's explicitly
permitted read-only getters and copy strings before returning. No allocation,
UI calls, recursive writes or retained native string pointers cross callbacks.
Query/UTF-8 errors and arena overflow fail explicitly. Owner take_events drains
owned strings; callers must drain between feeds. The transport pump enforces
the same event/text bounds across its entire pending batch and fails closed
on overflow, never publishing a successful partial batch.

After replies enqueue, pending values retain native event order. At finish_batch,
the existing router sanitizes titles and percent-decoded unresolved directory
paths (including drive-URL slash removal), updates the shared cache and delivers
reliable events with backpressure outside native work. Then capture publishes
and Output is posted using existing repaint-hint coalescing. The render consumer
still owns SharedSessionState::release_repaint_hint; FrameReader alone does not
acknowledge UI events. Explicit close flushes pending reliable events without
capture/repaint; fail-stop discards the unsuccessful batch. Drop never waits for
UI delivery. A reliable event already sent cannot be rolled back if a later
capture fails. Closed UI channels retain existing sink logging/counter policy.

This only proves delivered callback values, not arbitrary OSC length parity:
the pinned engine ignored a 2048-byte title in testing while 512-byte titles
produced callbacks. Its parser limits need reconciliation before production.
Existing sanitizer truncation is preserved, not confused with native arena
overflow. Clipboard, program status, semantic prompts and graphics still block
overall event/production parity.

The first Rust boundary lives in `crates/terminal/src/ghostty.rs`, compiled only
with the explicit `ghostty-native` staging feature. It owns its native terminal
and a boxed fixed-capacity reply context, rejects zero dimensions and reports
reply overflow as a sticky error. It is deliberately !Send and !Sync. Neither
transport constructs it yet. The build script requires an explicit project-local
prefix and never fetches tools; the headless C probe checks the narrow Rust enum
discriminants against the installed headers before testing the Rust owner.

Clipboard callback adaptation is not a simple EventBatch translation. The pinned
header requires calling the clipboard request's reply function before returning,
and terminal.zig constructs its request context on the stack. Never retain that
request, its reply context or its function as an asynchronous UI token. The
existing application sends SessionEvent::ClipboardRead for later UI processing;
bridge this without blocking the UI behind the engine lock or granting new
remote clipboard access. The default remote-deny policy is not a replacement for
preserving allowed local reads. This remains a cutover blocker, not implemented
behavior or permission to silently disable clipboard support.

The pinned headers default Kitty file, temporary-file and shared-memory image
media to disabled. Do not enable them as an incidental effect of installing
graphics callbacks: remote output must not gain local-file access. A future
graphics adapter must preserve the existing file/side-effect policy explicitly.

## Snapshot and interaction adaptation

US-0221 adds an asynchronous staging transport pump, privately owning AsyncEngine.
PtyTransport calls are nonblocking FIFO admission. Feed chunks enqueue native
replies before draining bounded events; finish_batch awaits policy-routed reliable
events before requesting prepared capture and coalesced Output. Closed reliable
queues fail-stop. A guard on every polled operation closes transport/admission
on cancellation or failure; no consumed output is replayed. Bridge publication
and abort serialize under the publication lock, retaining last-good data after
abort. A capture already committed before cancellation cannot be rolled back.
Graceful close flushes pending events without capture; explicit shutdown signals
transport/admission immediately before polling and discards an unfinished batch.
Both await native join off the executor; Drop never joins or waits for UI. Model
clones cannot feed/capture. Production read loops/session contracts remain open.

US-0218 drives a worker-owned native search with a 4096-byte needle cap and a
4096-result collection cap. Each explicit step feeds once and ticks at most once;
status distinguishes partial from complete. Native matches are ASCII-insensitive,
byte-exact otherwise and may cross soft wraps; unsupported case/whole-word flags
must return a recoverable error rather than silently changing the old contract.
Responses own current screen-space endpoint coordinates only, not stable RowIds
or native refs. Set/step/clear do not set terminal selection or publish frames.

US-0219 bridges EngineOwner commands through a bounded Cargo-only async-channel
dispatcher; the blocking endpoint and join stay off GPUI/tokio. Admission is
nonblocking with explicit Busy, accepted requests execute at most once even if
their response future is dropped, and close closes admission out of band, rejects
queued work and awaits native worker destruction. This is an owner bridge, not
yet the transport pump or synchronous-native/asynchronous-UI clipboard bridge.
Feed still returns replies that a transport caller must deliver before capture.
The command queue has eight slots, admission returns Busy immediately, and each
accepted request owns one bounded response slot. Dropped interaction futures do
not poison state or retry commands; undelivered nonempty transport replies or
reliable events instead fail-stop, including results already buffered at drop.
Unclaimed newly-created anchors are released on the worker, or cleaned up by
shutdown if their buffered response is dropped. Close finishes an in-flight
operation and cancels queued work; it is not an interrupt/deadline for native code.
The search response cap does not bound native internal result storage; native
search semantics/options and memory policy still block legacy search parity.

US-0244 exposes that bounded search through the independent native surface.
Search requests share the backend owner's serialized command path; a query
generation discards superseded responses. Results are usable only while both
their query generation and opaque grid revision match the current publication.
Any terminal-grid mutation invalidates coordinates before they are painted or
used for navigation. Search-only progress/captures do not themselves mutate the
grid. Navigation centers a match using native `scroll_viewport` semantics and
then reacquires/validates the publication; it never turns search coordinates into
tracked anchors. UI polling of incremental search has a finite work budget and
surfaces incomplete status rather than running an unbounded native loop.

One GPUI task serializes set/step/reveal/clear operations. Edits debounce for
150 ms and replace only pending work; an admitted endpoint future must finish
before a superseding generation is issued. Clear/close hide highlights immediately
and enqueue native clear after that acknowledgment, rather than cancelling the
endpoint and triggering its fail-stop rule. Input focus never forwards bubbled
search editing to native key input. Unsupported option toggles are absent.
Surface release leaves a running driver responsible for acknowledgment and clear;
an idle surface release schedules its own detached native clear while retaining
the session. Cleanup failures are diagnostic and do not change caller lifecycle.
One query takes at most 64 progress operations; FeedRequired or budget exhaustion
shows partial results. Refresh after grid mutation restarts at index zero, not an
old coordinate/index pretending to be a stable match identity. Navigation wraps
within the current returned list; a queued direction is a single replaceable slot.

Query checks include the live dispatcher revision, so retained last-good frames
cannot validate search after feed/resize/scroll but before capture. Prepaint
rechecks both that live revision and the exact painted viewport. Reveal uses an
opaque revision/row token; the dispatcher validates it immediately before native
absolute-row scroll, then the pump captures and reacquires search coordinates.
This closes the check-versus-enqueued-scroll race without stable anchors. Invalid
tokens, needle limits, result limits and unsupported options acknowledge domain
errors without closing a healthy owner. Endpoint needle validation precedes FIFO
admission; match-limit errors in the UI driver also clear retained native results.
The result cap still does not bound native internal allocation during a tick.

US-0220 adds a shared native model on that bridge. Capture/projection execute on
the dispatcher before publishing a complete immutable prepared frame. UI readers
clone that frame and maintain independent damage without native access/projection.
Model methods enqueue interactions but expose no output feed/raw replies; only
the backend endpoint can request reply-first capture. Native coordinates/modes
remain explicit: no fabricated TerminalInfo RowIds or default legacy ModeSnapshot.
This is shared staging integration, not either production TerminalSession yet.
Model reader damage comparisons may allocate as the staging FrameReader does;
the no-projection read guarantee is not a zero-allocation or performance claim.

US-0217 sets inclusive linear/rectangle selections using fresh snapshots of two
worker-owned tracked anchors. Validate token provenance/liveness and active-screen
ownership before creating either untracked ref; snapshot and native set execute
without any intervening terminal mutation. Native set copies/tracks selection
endpoints, so releasing caller anchors does not invalidate the native selection.
Clear and copy concern only the active screen. Each copy uses a short-lived
formatter, plain/unwrap/trim, and bounds owned UTF-8 at 1 MiB with no partial
return. Invalid endpoints and text overflow are recoverable, not reasons to
close a healthy session. No system clipboard write or remote OSC permission is
introduced. Native selected-cell masks project to immutable viewport spans from
the same capture as rows/cursor; spans are separate from paint row cache keys.
This does not yet wire production gestures, overlays or a local/SSH session.
The pinned native switchScreen clears the destination screen selection, including
when returning to primary. The adapter preserves this behavior; it does not
restore a cached selection from an inactive screen.

US-0216 establishes a bounded worker-owned tracked-grid-reference registry for
history positions. A token contains Rust-only terminal provenance and a unique,
never-reused identifier; it is equality-only and has no RowId ordering/arithmetic.
The registry caps live handles at 256, rejects foreign/released tokens without
native access, and frees every tracked handle on its owner worker. Typed points
name active, viewport, screen or history space. Resolution returns an optional
native coordinate plus the anchor's original primary/alternate screen identity;
the native tracked API resolves against that screen even when inactive. Native
NO_VALUE stays absent. Invalid points and capacity/stale-token errors are
recoverable; unexpected native errors still fail-stop. No untracked ref or native
address leaves the worker, and no production model cutover is implied.

The pinned native coordinate implementation does not enforce every space's
upper bound: history lookup may accept active content, and viewport conversion
may return rows below the visible viewport. The adapter requires exact native
round-trip coordinates at creation, excludes positions representable in native
Active space from History, and bounds Active/Viewport rows by native terminal
height (native resize updates both screens). It does not derive ordered row ids
or history positions from render-row identity. GhosttyPump exposes the same
serialized create/resolve/release seam without bypassing reply-first capture;
recoverable token/point errors do not close its transport or publish a frame.

US-0215 serializes owned key/mouse input requests with feed, scroll, resize and
capture on EngineOwner. Native event/encoder handles are created, configured,
encoded and freed on that worker. Key/mouse set_from_terminal reads current
native modes (including modifyOtherKeys, backarrow and pixel mouse formats);
never reconstruct them from legacy ModeSnapshot. Each key request limits text
to 4096 bytes and each native encoded result to 64 KiB, with an explicit error
instead of truncation. Invalid local input is rejected before worker mutation.
Mouse events carry finite surface pixel positions, explicit geometry and button
state. Fresh encoders do not claim motion deduplication. GhosttyPump delivers
successful encoded bytes to its own PtyTransport exactly once, fail-stopping
on transport rejection rather than replaying. Paste uses existing OneTerm
encoding/sanitization and the 1 MiB policy cap with live native mode 2004;
this does not enable Kitty clipboard paste events or change OSC permissions.
The native owned-key contract is not yet a complete GPUI physical-key mapping.

US-0214 adds transactional owned paint consumption. ProjectedFrame retains the
Arc of the exact source capture beside its rows. A backend-side ProjectedReader
prepares the projection before committing its independent FrameReader watermark;
on error it returns the cause and retains its last complete publication. Polling
an unchanged capture reuses the same immutable projection. Lifecycle status is
still read even when the capture is unchanged. Projection is allocating work:
do not run this preparation from GPUI prepaint or a tokio executor. This does
not yet adapt TerminalContent's ordered history/selection contract or select a
production session engine.

US-0213 captures native mode flags, active-screen identity, keyboard flags and
scrollbar coordinates with each owned viewport on the native owner thread.
All requested metadata must succeed before publication; no default-filled
metadata is published on getter failure. Metadata changes require full damage
except cursor visibility, which retains its existing old/new cursor-row damage.
Viewport top/bottom/delta/row
commands serialize through EngineOwner; offsets never become stable row ids.
Native input flags remain explicit (including SGR-pixel mouse mode) until the
session encoders support their meaning. This is not permission to silently
map unsupported native input behavior to a legacy default.

Replace direct Terminal/screen/interner access in handle.rs, content.rs and
model.rs. Materialize owned rows/styles/graphemes/hyperlinks under the engine
lock; render iterators and native grid references never escape it. Maintain
stable application anchors using upstream tracking facilities and invalidate
stale anchors after clear, trim or resize. Preserve per-consumer damage semantics.
Adapt search, selection, viewport scrolling, keyboard/mouse and bracketed paste
to Ghostty state, not to a second oneterm-vt grid.

The first owned frame projection uses a per-consumer native RenderBuffer. Capture
creates a fresh GhosttyRenderState, walks rows/cells while borrowing the terminal
exclusively, and returns only Rust-owned text/style/row/cursor data. Native
iterators never escape capture; their owners free cells, then row iterator, then
render state, including partial initialization failures. Every capture copies a
full viewport for now; native dirty flags do not define application damage yet.
Regression testing showed that alternating persistent native render states can
leave one consumer's cells stale after another updates. Recreate all three native
handles on every capture to force a complete copy, then replace the consumer's
previous handles only after success. This intentionally allocates each frame;
do not call it an optimized application rendering path. Before production, audit
single-owner native capture with fan-out of owned frames and per-consumer damage.
Opaque two-word row identities support equality only, not RowId arithmetic.
US-0212 separates the GPUI row reader from SnapshotRow using a borrowed
application RenderRow (cells, resolved runs, cluster arena, wrap, equality key).
Existing snapshots lend their values without copying. Native projection owns
the same pure value types, never a legacy Terminal. Its logical RowId is absent;
native identity must not flow into image-placement or selection arithmetic.
Native cache keys retain immutable projected rows and compare exact identity,
cells, styles and clusters. This avoids hash collisions or invented sequence
numbers at a staging memory/comparison cost. Full-frame metadata, native history
anchors, links, images and semantic marks remain separate cutover requirements.
Projection reserves cells/runs/cluster arenas fallibly within the existing
cell/UTF-8 budgets (a scalar arena can use four bytes per UTF-8 byte). It rejects
malformed widths and unknown style flags rather than emitting partial rows;
selected cells now prepare independent exact viewport spans. Native resolved/default
colors become effective RGB paint values, with foreground exactness retained
per cell for shared default/ANSI minimum-contrast policy. Explicit RGB and
extended-palette foregrounds stay exact; intent participates in row equality.
Broader semantic classification integration remains a cutover requirement.
Cap a frame at 1,048,576 cells and 16 MiB of UTF-8 text. Check native lengths
before resizing scratch/output buffers, use fallible reservations, and return an
explicit error instead of a partial/truncated frame. Preserve foreground,
background and underline color kinds plus all style flags; packed-cell color
backgrounds also need their resolved render-cell color, not just GhosttyStyle.
The projection is staged until hyperlink/image/semantic/history mapping and
existing GPUI frame/damage contracts are implemented and proven.

The emoji ZWJ fixture explicitly enables grapheme clustering (DEC mode 2027).
Without that mode the pinned engine splits the fixture into separate cells;
the adapter preserves native grapheme/cell boundaries and does not silently
change terminal modes. Mode defaults and application Unicode parity remain part
of cutover acceptance.

Inventory Sixel, Kitty graphics, OSC 7501/20308/9 aliases, cwd, clipboard, dynamic
colors, title and shell integration before cutover. A missing upstream primitive
requires a documented bounded adapter or explicit unresolved gap; silently
dropping shipped behavior is not acceptance.

## Verification sequence

US-0227 adds ordered native lifecycle delivery. Pump publish_exit flushes pending
events, records exit status/dead cache and reliably sends Exited without capture;
SSH continues reading late output. Final Closed follows admission/budget cleanup,
physical channel close and native destruction, including failures. The existing
endpoint close token cancels backpressured lifecycle waits; immediately ready
delivery wins, otherwise Report.lifecycle is Cancelled (also when Exited was
cancelled earlier). Normal delivery is reliable and bounded, not fire-and-forget.
Event loss is typed; original native/SSH failure retains precedence over final
delivery failure. The direct handle stays alive until task result, so explicit
close also wakes final UI wait. No peer-close monitor or shipping selector is
introduced. Already published events/frames are never rolled back or replayed.

US-0226 adds resize to the same 64-slot NativeChannel physical-control FIFO.
Pre-admission viewport budget validation rejects zero or oversized requests.
At a batch boundary, the owner drives pump resize (native first, original
transport marker second), services pending window-change, then captures. The
driver can consume the marker during native awaits; a bounded drain of already
queued commands followed by the pending-size drain before capture also covers
an immediately ready pump future and a full queue with no resize marker. Earlier
encoded bytes cannot be overtaken by this final resize barrier. Remote submission
is not acknowledgment. Failed/cancelled operations stop and close rather than
rollback/replay; no resized capture follows physical resize failure. UI pressure
defers controls, and explicit close discards them. Raw writes may still overtake
queued controls; endpoint resizes remain FIFO, not an unbounded burst buffer.

US-0225 extends NativeChannel with a separate 64-slot physical key/mouse FIFO.
Public input validation runs before nonblocking admission, bounding key text to
4096 bytes and rejecting malformed geometry/control text without stopping the
owner. The single owner dispatches inputs only at completed batch boundaries,
never reentering a pending feed/capture. Encoding uses current Ghostty modes;
encoded bytes use the existing SshTransport FIFO/budget. Physical input order
is FIFO, but raw byte writes may overtake pending physical input. Admission is
not physical receipt. Close remains out-of-band and teardown drops queued input
without encoding/replay. This is not GPUI mapping, resize or shipping selection.

US-0224 introduces an explicit default-off native direct-channel owner endpoint,
not a TerminalSession or application factory selector. One tokio task owns the
direct client handle, split channel reader/writer, AsyncGhosttyPump and command
receiver. While feed/capture awaits native or UI capacity, a select driver
continues servicing the original SshTransport write FIFO; explicit close has an
independent CancellationToken, selected before pending work, so full UI/command
queues cannot hide it. Cancelling active pump futures retains last-good and
stops native admission. A single teardown closes command admission, requests
transport closure, drains reserved writes, closes the physical channel, awaits
native destruction and then drops the direct handle. Already published events/
frames are not rolled back. Endpoint Drop requests close without joining.
Report observed exit status in the typed task result, not unimplemented reliable
lifecycle events. Resize/physical input/gestures, jump-host/SFTP/tunnel lifetimes,
production local loop and session/content contracts are explicit remaining work.

US-0223 adds Windows-only real PseudoConsole child proof under the local-shell
default-off ghostty-native link feature. The fixture owns spawn/register/read/
write/resize/deregister/destruction on its test worker, uses the original
LocalTransport/ShellNotifier FIFO and byte budget, and drives AsyncGhosttyPump
without a legacy Terminal. It explicitly verifies child ACK, child-observed
console size and successful exit. ConPTY can consume or transform child VT, so
this is not raw DSR parity. Shared test-only spawn coordination preserves orphan
fixtures. Shipping ShellEventLoop and TerminalSession are not selected or proven
by fixture I/O; backpressured async shutdown and production cutover remain open.

US-0222 stages real SSH transport proof behind oneterm-ssh/ghostty-native (off by
default, forwarding the terminal native-link feature). Test-only fixtures read a
real authenticated loopback russh channel into AsyncGhosttyPump, drain the same
SshTransport command FIFO into channel.data and release its byte budget once.
The peer observes input/DSR order and coalesced window size; bounded UI events
separately gate capture. Cancellation retains last-good and signals transport
closure, but the fixture explicitly closes the real channel and awaits native
shutdown. This does not prove shipping ssh_main_task cancellation or SFTP/session
teardown, and constructs no legacy Terminal for the native stream. Real local
PTY proof and simultaneous production cutover remain separate work.

1. Build the pinned library for x86_64-windows-msvc and link a headless C ABI probe.
2. Prove chunk-split CSI/UTF-8 persistence, screen content, resize and DSR replies
   without opening any process or PTY. Compare whole versus split byte streams.
3. Exercise owned Rust handle lifetime, callback lifetime, error paths and
   replies through the existing SSH/ConPTY pump test seams.
4. Prove renderer, history/reflow, input, search/selection, graphics and OSC-policy
   parity, then cut over both production backends together.
5. Update DEC-0014's application scope, backend/architecture/dependency docs,
   notices and build/CI instructions; run the complete project gate and real
   Windows GUI/SSH acceptance. Do not mark cutover complete on a smoke test.
