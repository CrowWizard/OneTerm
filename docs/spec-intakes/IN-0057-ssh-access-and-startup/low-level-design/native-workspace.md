# Detail Design: Native workspace integration

## Status and owning outcome

US-0249 design in progress. The owner selected the real OneTerm interface with
native terminal sessions, without a privileged test startup flag. This is not
permission to replace production defaults or weaken global permission checks.
Service ownership, configured local sessions, the shared tree/renderer and tagged
panel consumers are implemented incrementally below. Explicit native local
factory/startup selection is wired with isolated storage; shutdown-race and
complete workspace acceptance remain open. Production selection stays legacy.

## Existing seams verified

### Terminal Tab routing (US-0261)

Register Terminal-context Tab/Shift+Tab NoAction overrides in the shared
terminal feature init, not in the legacy view constructor. Native-only app
startup must suppress component focus traversal while the terminal owns
focus, allowing its existing native key encoder to send Tab to the shell
for completion. Ordinary controls outside that context retain Tab navigation.
No shell completion policy or persisted key binding setting is changed.

### Directional split preview (owner rework 2026-11-16)

Titles reveal only while dragging and keep
their release reorder. Each occupied terminal Space chooses the nearest edge
in normalized coordinates, previews that half with the theme drop-target
colour, and creates the corresponding split only on release. A valid other
terminal tab is required; empty Spaces retain direct fill. Preview is visual
only and must not resize or create sessions before release.

Validate source identity and unambiguous movable pane before mutating the
tree; reuse guarded move and lifetime mechanics. Same-tab occupied, foreign,
missing, empty and ambiguous-source drops are inert. No session is restarted.

### Destination reveal during a terminal tab drag (owner rework 2026-11-15)

Dragging selected tab 1 over terminal tab 2 selects tab 2 while preserving
the original dock DragPanel and mouse-down gesture. The pointer may then
enter tab 2's empty split and finish the existing move. Use the shared terminal
title row for strip/expanded/collapsed layouts, resolving live group membership
by panel identity rather than a captured index. Only an existing terminal
panel drag can reveal a destination; ordinary mouse hover does not select.
Selection never reorders or transfers content by itself; releasing on a row
retains the dock's existing reorder behavior. Occupied content uses the
directional preview/drop amendment above.

### Shared terminal ANSI theme defaults (acceptance rework 2026-11-14)

Both legacy and native views use the shared theme builder. ANSI 0/7 use
theme background/foreground; 1-6 use base red, green, yellow, blue, magenta
and cyan. ANSI 8 uses muted foreground. ANSI 9-15 retain their base hue and
saturation and raise HSL lightness by 20% of the remaining distance to white.
Composite translucent base ink against the theme background before conversion
to opaque terminal RGB. GPUI base.*.light tokens are dim/translucent surface
variants, not ANSI bright ink. Settings ANSI overrides take precedence, then
program OSC colors until their reset restores the latest settings/theme default.
Idle theme changes use the existing acknowledged/coalesced native publication;
effective OSC colors are never fed back as defaults. Extended 16-255 palette
and truecolor semantics remain unchanged. No theme schema or stored data changes.

### Native terminal action entry points (acceptance rework 2026-11-14)

Split/tab acceptance amendment 2026-11-15: actual native dock gestures retain
the shared move model. A drag takes the active terminal; when a newly split
empty Space is active, it takes the sole terminal instead. Multiple inactive
terminals are ambiguous and remain untouched. A same-tab move into its new
placeholder is supported. Occupied/missing targets do not extract a session;
successful moves retain pane identity, channel membership and focus, and
only an emptied different source tab is removed without closing the moved
session. Native strip gesture and tree-selection regressions prove these cases.

Empty native/legacy Space menus offer New SSH Here. Both Quick Connect and
saved-session flows carry a weak destination panel and stable SpaceId through
the existing authenticated CreatedSession completion. A later focus change
does not redirect placement. Closed/occupied destinations warn and observe
unplaced connection cleanup, never replace a pane or fall back to a new tab.
Live-terminal New SSH keeps its existing new-tab behavior. See the owning
ssh-context-menu.md design; auth, trust, vault and engine selection are unchanged.

The native surface participates in the existing Terminal key context. Its
settings-bound right-click yields to the enclosing native pane menu when
show_context_menu is enabled; that press is not also sent to the shell.
With menus disabled, native mouse protocol routing remains unchanged.
The pane uses its attached SplitContext and the shared split_items builder
for all four directions. Duplicate destinations and Close Space operate on
the originating Space, not whichever terminal later acquired focus. Native
edit entries call the pane's native control operations; no legacy menu
session or adapter is fabricated. Existing tab-to-empty-Space drag mechanics
and ownership transfer remain shared, without edge splitting or a new move
model. Action-level tests click actual native popup rows, drive keyboard
splits, and exercise move_tab_into with unchanged native pane identity.

### Native startup continuation (owner-approved 2026-11-13)

Reuse the registered real workspace and its default-shell, new-tab and empty
Space constructors. Add an engine-neutral factory local-creation operation
returning CreatedSession; its default delegates to the existing legacy operation.
Only the explicit native composition root overrides it to return configured
native ownership. Its legacy local and SSH operations reject, never fall back.
Keep explicit legacy/native duplication dispatch. Surface failures remain visible.

Before any initialization, select the absolute `oneterm-native-data` sibling of
the test EXE as a once-only configuration root. The owner chose cross-launch
test settings/layout retention, not disposable state. No production import,
environment-variable home spoofing or fallback path. Test the root in isolated
processes so a test does not contaminate other configuration tests. Refuse root
selection after config_dir has been used; preserve production paths by default.
Existing elevation guards remain authoritative. Initialization reuses the same
feature registration and service bundle, injected with the native-only factory.
Native window opening suppresses automatic SSH/update/crash-dialog effects.

Wire the entry only after these seams are proven. Multi-pane ordinary window
close must collect pane completion without retaining controls and keep GPUI
running while observing the existing five-second cleanup budget. Forced app
quit retains its 200 ms budget and must report incomplete cleanup, never success.
Do not build/launch a new acceptance Release while those shutdown obligations
remain unproven. Gutter, full semantic OSC role parity and native SSH remain
separate acceptance gaps; basic semantic foreground scanning is described below.

Implemented startup proof uses the actual workspace with a real ConPTY native
session and exercises the shared asynchronous window-close observer immediately
after initial layout, with no ready-output or quiet wait. The earlier quiet-only
proof exposed `lifecycle delivery incomplete` on immediate close; the separate
orderly-seal rework below now passes that headless immediate-close proof. Retain
admitted-work/real-HWND and 200 ms multi-pane quit as acceptance gaps. A weak
NativeWorkspaceSessions registry holds pane weak handles and completion-only
observations; successful old observations are pruned and errors stay sticky.
It blocks new local creation once shutdown starts and never owns a session Arc.
Forced-quit observer cancellation is proved with isolated child processes for
both unpolled and polled-pending futures, not a real OS session-end broadcast.
Live-platform testing additionally requires the quit observer to capture
completion-only handles and sticky failure synchronously: GPUI holds its App
borrow while awaiting shutdown futures, so AsyncApp::update is not permitted
there. Targeted real-HWND close passes. Direct three-pane quit additionally
requires palette publication and idle release search-clear to await on the
background executor: foreground tasks do not resume under synchronous shutdown
and must not own session references needed by consuming completion. The color
and search future contracts are Send; weak foreground palette UI result delivery
owns no session. Actual platform direct quit now passes inside the unchanged
200 ms budget (slowest pane about 32 ms in the diagnostic run). This does not
prove all active search/edit/channel shutdown cases or all-machine latency.


- app::init installs AppServices with AppSessionFactory and feature-owned commands.
- OneTermWorkspace constructs panels through PanelRegistry by stable panel name.
  Its chrome, dock skin, tab rail, right dock and status bar should be reused.
- TerminalPanel stores a SpaceTree; SpaceContent::Terminal owns TerminalView.
  TerminalView stores Entity<Box<dyn TerminalSession>>, and its render, edits,
  search, channel fan-out and metrics assume legacy engine capabilities.
- SessionFactory returns Box<dyn TerminalSession> for local and SSH sessions.
  NativeLocalSession deliberately does not implement that legacy render contract.
- NativeTerminalSurface takes Arc<dyn SurfaceSession>. The caller owns reliable
  events, lifecycle, settings/color updates and observable errors.
- NativeChannel is not an SSH connection factory: it requires an authenticated
  direct connection and opened shell channel. Existing host-key/auth/jump/tunnel
  orchestration must not be bypassed merely to render a native terminal.

## Proposed integration boundary

### First implementation increment: typed session ownership

Add CreatedSession in the existing gpui-free factory module: Legacy carries
Box<dyn TerminalSession>; the feature-gated Native branch carries
Arc<dyn NativeSession>. Do not change SessionFactory return types or startup
selection until pane consumers can handle both branches. NativeSession combines
SurfaceSession and TerminalLifecycle with a consuming Arc receiver for completion.
Completion returns a Send future and a neutral native report (exit code and
whether lifecycle delivery completed), not a legacy render handle or thread join.
The caller must request close, retain exactly one completion reference, release
view/control references, and drain reliable events independently before awaiting.
The local implementation fails explicitly if references remain; it must not
invent completed teardown or retry an admitted operation. Unsuccessful completion
still requests close when the endpoint is dropped. Normal-close orchestration
retains its existing deadline and must satisfy this precondition before calling.

Prove the local adapter through the erased native service with a real local
endpoint, exactly-once event receiver and observed owner completion. Separately
prove a retained service reference returns an error rather than completion. This
increment introduces no factory selection, SSH connection path, workspace UI,
settings or persistence changes. Those remain required by US-0249; do not expose
an incomplete factory that silently drops configuration.

Use a tagged created-session value at the gpui-free terminal service boundary,
with an explicit legacy or native branch. Production AppSessionFactory stays
legacy. Only the native test entry installs native selection. Avoid adding
unrelated process-global toggles or persisted engine selectors. Feature-disabled
builds must retain the same default dependency graph and behavior.

Native session ownership has three separate responsibilities:

1. A cloneable surface/control handle exposes owned publications and ordered
   native operations, never a fake TerminalHandle, legacy grid or raw FFI pointer.
2. Exactly one reliable-event receiver feeds bounded UI/session state without
   depending on paint or an active tab. Program effects follow the originating
   session policy; no clipboard reply goes to a different pane or transport.
3. One completion owner requests stop and awaits native destruction independently
   of the surface. View release cannot cancel admitted native work unnoticed.

The service factory creates all three, with typed failure and no hidden legacy
fallback. Native local configuration must honor supported settings explicitly;
the current three-argument spawn cannot silently ignore scrollback/logging.
The SSH path reuses existing connection/authentication ownership and SFTP linkage,
changing the parser/surface only after an authenticated channel is obtained.

## Pane and workspace ownership

### Native pane owner increment

Immediate-close acceptance rework adds a distinct NativeSession orderly shutdown
request. The default preserves hard close for implementations without a stronger
contract; NativeLocalSession overrides it with a local owner seal. Under the
existing admission mutex, seal new raw/control input without setting the hard
stop flag. The active bounded native publication and previously admitted control
FIFO finish under independent reliable-event draining. Once sealed, service the
remaining controls but start no additional output read; when controls are empty,
return to the existing teardown/Closed-delivery path. Seal does not retry or
claim physical delivery of queued PTY bytes. Hard close, abandoned acknowledgments,
transport failures and explicit cancellation retain fail-stop behavior and must
not be reclassified as graceful completion. Pane shutdown still releases surfaces
and controls, then observes the consuming completion within the unchanged budget.
This is bounded by the existing caller deadline, not an unbounded drain guarantee.

Introduce a native pane entity around NativeTerminalSurface, not a TerminalView
adapter. Its detached reliable-event consumer keeps receiving through teardown
and uses a weak pane recipient: release cannot cancel draining. While alive it
reduces title/cwd/status/progress/exit state and invalidates the real surface.
Program effects retain backend OscRouter admission, sanitization and rate limits.
Reduce progress and bell independently of paint. Keep at most five pending
notifications, dropping oldest with a visible count, and reuse the existing
notification and progress/bell widgets. Clipboard operations use a bounded
ordered queue; overflow is counted and reported rather than growing memory.
Apply clipboard operations on the foreground refresh, including hidden panes,
not during render. Clipboard reads additionally require the live bound setting
(unbound is denied), a live originating session and a size-bounded OSC 52 reply.
NativeSession exposes only a clipboard-reply operation, not a new raw-write
facade; the local adapter encodes it and uses its original transport. The
authenticated NativeChannel exposes the same narrow operation through its
own transport; it is not yet an orchestrated NativeSession/factory. Never
broadcast or paste the reply. Shutdown rejects queued effects and drains later
events without OS effects; reply admission errors remain visible. Bound error state
and do not manufacture native capabilities. No test factory selects this owner
until those remaining effects/configuration seams are ready.

The pane keeps its session and surface in releasable slots. Shutdown requests
close exactly once, drops the surface and pane control reference, then moves
the remaining completion reference to a background observer. The observer
retains the ordinary five-second deadline across reference release, native
completion and reliable consumer completion. Its shared observation distinguishes
pending, successful completion and failure; cancellation marks failure even
when the future was never polled. There is no blocking join on GPUI. The fixed
200 ms application-quit orchestration remains a separate unsolved integration
requirement; background-pane completion alone is not proof of it.

### Shared tree increment

### Tagged panel consumer increment

TerminalPanel uses a tagged TerminalPane handle in its existing SpaceTree.
Legacy owns Entity<TerminalView>; native owns Entity<NativeTerminalPane>. Identity
and focus belong to the actual branch, never an adapter entity. Keep existing
legacy PanelSpec and constructors, adding only an explicit native session spec
until factories are ready. Split, move, placement failure, tab reset and panel
removal dispatch close through the branch. Preserve the legacy active_view
accessor only for genuinely legacy callers while migrating workspace Find to
an engine-neutral panel action. Native status/title/cwd use the native owner
and supported input channels/edit operations use the increments below; remaining
unsupported operations are reported explicitly. Native
local SFTP/network capabilities are absent, not made up; SSH capability wiring
remains unavailable and no SSH factory selects this path yet.

Prove the real TerminalPanel, not only the generic tree, can hold a native pane
and preserve its identity through split/move/close while legacy regressions
remain green. Keep the production factory and startup unchanged.

Parameterize the existing Space tree over its cloneable terminal handle, with
Entity<TerminalView> as the existing default. Tree mutation must not know the
engine: split, fill, take, close and collapse return the exact supplied handle.
Keep focus resolution supplied by the panel through a closure; empty leaves
retain their existing placeholder focus. Rendering remains the existing tree
renderer, not a second native layout implementation. This permits the eventual
tagged pane handle without forcing native surfaces through TerminalView.

The shared renderer requires only a pane entity identity and its element. Use
one crate-private PaneView contract, implemented for GPUI render entities, so
the same split layout can render either the existing TerminalView entity or a
NativeTerminalSurface entity. Placeholder rendering stays generic over the
leaf handle; actions still route through the containing TerminalPanel. Identity
continues to key existing input-channel badges, never a newly allocated wrapper.

Before selecting native panes, prove the same tree algorithms with a tagged
non-legacy handle: fill failure returns ownership, split/move preserve identity,
closing a sibling returns only its handle, collapse preserves the survivor, and
the last-leaf close continues to leave ownership with the panel. Run existing
Space/panel regressions and native/default all-target checks. This increment
alone does not select a native engine or claim workspace acceptance.

Introduce one terminal-pane abstraction in terminal-view used by the existing
TerminalPanel/SpaceTree. Each branch owns its actual view type. Focus, identity,
title/cwd/status, duplication metadata, edits/search and close dispatch through
that abstraction. Keep tree/split/layout operations shared; do not create a
second workspace or a new dock-panel implementation that loses Space behavior.

Native lifecycle/event aggregation belongs to the native pane owner. Extract
backend-independent bounded reduction logic only where both owners need it;
backend-specific waiting remains below the UI or in the composition root.
AppServices metrics and commands must refer to the active pane in the current
workspace, not a process-global most-recent terminal.

Input actions route once to the selected surface. Admitted paste/search/drag
operations retain their completion obligations. Native settings updates must
publish native colors and renderer inputs together without a legacy palette
handle. Unsupported native features remain explicit gaps rather than fabricated
rows, modes, history, search matches or acknowledged completions.

## Startup and persistence

### Native channel integration

Registry membership remains keyed by the actual pane, not its surface or
session. Add engine-neutral peer callbacks beside legacy session targets;
native callbacks capture only WeakEntity and enqueue bounded owned input.
Callbacks never invoke origin handlers or registry fan-out. Native membership
leaves on session end, shutdown and pane release; exited panes cannot join.
Each native pane drains one ordered peer inbox, applies raw bytes/UTF-8/Ctrl+C
through an acknowledged raw-visible backend control and paste through its
existing native-mode operation. Raw-visible shares the paste payload budget,
reveals the live viewport, bypasses paste encoding, writes once and retains
fail-stop cancellation. Admission/processing failures are visible on the pane;
other peers still run. All actual panel/menu paths use tagged pane identity.
Native origins serialize key receipt, committed text, paste and releases; only
successful press/repeat/text/paste operations fan out, Ctrl+C uses Interrupt,
and releases/blur never fan out. Pending work must not retain a pane/session
through registry membership or silently retry after close.

### Native input-channel prerequisite

IN-0022 broadcasts origin-encoded key bytes, text, paste and interrupts; it
does not re-encode a key in each peer's modes. Native key admission currently
returns before confined encoding and original FIFO admission. Add an explicit
key receipt operation through the existing local/SSH control queue: validate
before admission, encode once on the native worker, write once to the original
transport, then acknowledge the exact accepted bytes (including an empty
encoding). Failures return no receipt. Reuse admitted-operation cancellation
guards and bounded queues; dropping an admitted wait closes independently,
with no replay or retry. Ordinary key/release admission stays unchanged.
SurfaceSession exposes a borrowed typed receipt future with an explicit
unsupported default; only real native endpoints implement it. Do not expose
the engine encoder to the UI or invent legacy modes. This prerequisite alone
neither enrolls a pane nor changes broadcast behavior. Subsequent integration
must retain stable pane identity, weak membership ownership, origin-success
gating, ordered mixed-engine routing and origin-only release/blur events.

Implemented prerequisite evidence: six focused pump/local/SSH proofs cover
mode-sensitive exact receipts, one origin write, empty encoding, invalid input,
transport failure, bounded queue rejection, unpolled no-op and admitted
cancellation. Native regressions and the full bundled gate pass; see US-0249
and .tmp/us0249-native-key-receipt-*.log. Channel enrollment and peer dispatch
are still pending; no channel acceptance follows from receipt availability.

### Native local duplication increment

Add only the explicit feature-gated spawn_native_local factory operation, with
an unsupported default error and a composition-root configured native local
implementation. Never fall back to spawn_local, select a production engine or
change persisted settings. The source pane determines duplication engine;
recorded shell config and live cwd follow the existing duplicate contract,
while scrollback/security/logging use current bound settings. Carry the actual
CreatedSession through tab, empty Space and split placement. Shared native
pane construction binds settings and owns event draining/completion. Failed
placement shuts down that owner using the same five-second observation budget,
not a raw Arc drop or invented completion. Native SSH duplication rejects
before the legacy authentication dialog until native orchestration is ready.
Prove source identity stays unchanged, destination is genuinely native and
unplaced owners reach observable completion. Normal factory/startup remains
legacy; this is an explicit service seam, not native test-entry acceptance.

### Native clear command increment

SurfaceSession exposes a narrow acknowledged clear_command operation. The
original local/SSH control FIFO owns it as one operation: enqueue exactly
b"clear\r" on that session's transport, clear native selection and capture
its publication before replying. No paste encoding, RIS/reset, local history
erasure or input-channel fan-out is permitted. Completion acknowledges input
admission and native publication, not command execution by the shell. A
transport failure remains the returned error even when selection clearing
succeeds; failures do not retry, and cancelled admitted waits request close
through the existing ControlWait policy. The actual panel clear action uses
bounded surface edit admission and a detached task that retains its session
until acknowledgment after UI release. Existing close deadlines are unchanged.

### Native select-all increment

Extend the existing selection update with All. The confined native worker
derives and installs the pinned select-all snapshot in one operation, clearing
selection when no selectable content exists. No native snapshot escapes the
operation. The bridge and existing backend selection FIFO capture/publish the
result before acknowledging it. The real panel action invokes the surface's
bounded edit admission; its detached task awaits admitted selection even after
surface release, retaining the session reference observed by ordinary close.
Errors reach the existing callback, or are logged if the recipient is gone.
Do not route select-all as program input or approximate history with frame rows.
Clear uses the narrow shell-command seam above; native local duplication uses
the explicit service operation above. Native SSH duplication remains unsupported.

### Native default-color publication increment

Extend SurfaceSession with the existing borrowed acknowledged default-color
future. Local and authenticated native channels forward into their original
FIFO and publication acknowledgment; no parser or legacy session is constructed.
Only explicitly bound workspace surfaces synchronize colors. Build defaults from
the shared static theme/settings (ANSI 16 and standard extended palette), never
from native effective colors. Observe settings and active Theme, also refresh
on render. Renderer-only selection/contrast overrides follow the shared theme.
Native cell/background/cursor publications remain authoritative for OSC effects.
Native projected rows also retain per-cell foreground exactness: defaults and
ANSI 0-15 use the shared minimum-contrast policy against the effective published
background; explicit RGB and extended palette foregrounds stay exact. The view
tags non-exact effective RGB rather than re-resolving through potentially newer
UI defaults. Intent participates in immutable row equality/cache invalidation.
Packed erase backgrounds, native inverse/reverse behavior and OSC reset remain
owned by the same capture; effective colors never become caller defaults.

Maintain one driver per surface with one latest intent, one acknowledged palette
and a failed latch. Supersession coalesces only unadmitted intent; an admitted
future is always awaited. The detached driver owns its session reference through
acknowledgment even after surface release; it sends no further work once the
surface has gone. Errors are delivered through the surface error callback or
logged when the recipient has gone, and disable implicit retries. Ordinary pane
close observes reference release under its existing five-second deadline. No
change to production selection, storage, SSH authentication or app-quit budgets.

### Live cursor binding continuation

Workspace-bound surfaces read live cursor shape/color/blink settings alongside
typography. Reuse the existing setting-to-render shape mapping and CursorConfig;
configured shape and explicit cursor color override published appearance, but
published hidden/position and native steady-cursor policy remain authoritative.
Unbound surfaces keep native cursor appearance and a visible blink phase.
One surface-owned weak task ticks on the shared 500 ms grid. Only a bound,
focused, visible native blinking cursor with blinking enabled toggles and
invalidates; focus/blur and disabled blinking reset visible phase. Native steady
and hidden cursors do not repaint for timer ticks. Dropping the surface cancels
its task without retaining session controls or changing pane close deadlines.
No persistence/schema, backend defaults or production startup changes occur.

### Live typography binding increment

Only explicit workspace panel construction binds its existing TerminalSettings
entity to NativeTerminalSurface. Observe it for invalidation and read it again
before each render, so font family/size/weight/features/fallbacks/ligatures and
line height/cell width/padding cannot remain at construction-time values.
Reuse the existing terminal font builder rather than approximating its policy.
The shared RenderState metrics/cache keys and native resize control remain the
only geometry path. Unbound acceptance surfaces preserve caller-supplied fonts.
No settings load/save, schema or new global initialization is introduced.
Native published cell/default colors remain authoritative; backend defaults now
synchronize as above. Cursor settings use the continuation above; gutter and
full semantic OSC roles remain gaps; basic foreground scanning follows the
semantic parity continuation below. Program
effects follow the pane owner increment above.

### Configured local-session increment

Before native factory selection, carry an explicit optional scrollback line
limit through native local endpoint, async pump, dispatcher and confined owner
initialization. Existing probe constructors keep upstream defaults; configured
workspace construction supplies the actual settings value. Set the native line
limit and remove the upstream default byte cap so it cannot silently override
the requested history; zero disables history using the byte limit. The pinned
API prunes whole pages, so a positive line limit is approximate, not an exact
retained-row promise. Configure before the owner accepts any output.

Add a configured NativeLocalSession constructor taking scrollback and logging.
Enabled logging is not yet supported by the native path and must return an
explicit error before creating a PTY, native worker or file. Disabled logging
must create no file. Do not feed a legacy parser to emulate native logging.
Verify native option readback at the confined boundary and real local zero-
history output through the configured session. Leave factory selection unchanged
until all pane/event/lifetime consumers are ready. No persisted schema changes.

Reuse the actual workspace initialization and feature registration. Define the
test entry's configuration-storage boundary before enabling normal init: the
current isolated entry loads no application configuration, while normal init
can create defaults, load a vault/session store and register writers. Do not
silently broaden the test entry's file effects while changing its appearance.
No automatic saved-host connection or updater should be enabled as an accidental
consequence of reusing window::open_window.

No-argument launch must not depend on an elevation opt-in. Record the real token,
inherit privileges as-is, and keep production permission/persistence policy
unchanged. The native test entry must not call run() and pretend it selected the
native engine, or label an elevated token ordinary to unlock workspace controls.

## Cleanup

### Ordered native input-channel continuation

The IN-0022 registry LLD now defines mixed legacy/weak-callback targets. Both
origin and peer queues bound 64 queued requests and 4 MiB including active
payload. Local/SSH raw-visible controls share the paste payload budget and
original transport FIFO; raw bytes are never independently encoded or pasted.
Origins wait for acknowledgment before fan-out, snapshot their enrollment
token before waiting and suppress fan-out after leave/rejoin, close or move.
Keys return their exact single-write receipt; text uses committed UTF-8, paste
retains recipient-native transformation and Ctrl+C retains Interrupt. Release
and blur stay origin-only. Successful empty key receipts still allow original
release; queue-rejected releases remain held for explicit key-up/blur cleanup.
Surface release cancels queued and admitted waits with backend fail-stop
semantics; registry callbacks never retain the native session. Actual panel
menu/tab-wide actions use pane identity and skip ended sessions. This does not
select a native startup factory or finish SSH connection orchestration.

Keep the existing ordinary-close five-second reference-drain contract and GPUI's
200 ms app-quit budget distinct. Coordinate all live panes at window close;
closing one tab must not quit other panes. Completion owners outlive GPUI view
references. Close UI resources on the foreground executor, then observe each
native owner and reliable consumer. Timeout/cancellation remains a recorded
failure, not exit 0. The US-0245 session-end owner-completion gap is not solved
by adding tabs and must stay separately visible.

## Required proof before workspace acceptance

### Semantic foreground parity acceptance rework

Workspace-bound native surfaces configure the existing shared SemanticOverlay
from live semantic-highlighting settings and the same shell-profile mapping as
legacy views. Standalone unbound surfaces retain disabled scanning. Native
projection retains default-foreground provenance separately from exactness,
with both participating in immutable row equality. The view preserves effective
native RGB while allowing the shared semantic style merge to recognize actual
default foregrounds; equal-valued explicit ANSI/RGB must not gain eligibility.
Native inverse/reverse and OSC effective colors remain authoritative, with no
second parser or fabricated OSC 133 cell roles/exit-code anchors. Native roles
remain unavailable, so the shared scanner uses its existing textual fallback.
This closes basic foreground scanning only, not gutter or full OSC role parity.

- Factory: explicit test selection constructs native local/SSH sessions only;
  ordinary startup still constructs legacy sessions only.
- Pane tree: split/fill/move/focus/duplicate/close actions preserve identities,
  sibling sessions, title subscriptions and per-workspace metrics.
- Native pane: output invalidation, settings/color/resize, search/edit/IME,
  exit/error display and offscreen event draining use real native publications.
- Security: host-key/auth approval, clipboard request policy and reply origin,
  notifications, logging/persistence restrictions and no automatic startup effects.
- Cleanup: tab, Space, window and app quit; no successful report before all
  required owner/consumer observations. Synthetic and real-window proofs remain
  distinct. Do not auto-restart owner-cancelled real pending-search repeats.
- Regression: native terminal/view/local/SSH/app tests and Clippy, full bundled
  local gate, optimized build, then real Windows owner UAT on the resulting UI.

## Open design items

Finalize the tagged session/completion-owner signatures against both backends,
the settings features native sessions can support honestly, and the test entry's
configuration scope. These must be settled before implementation of the workspace
service seam; this document is not a claim of completed integration.
