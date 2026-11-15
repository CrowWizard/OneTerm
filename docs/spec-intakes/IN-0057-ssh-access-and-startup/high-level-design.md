# High-Level Design: SSH access and startup

## Outcome and accepted scope

US-0259 owner amendment (2026-11-15): full ordinary application functionality for
confirmed administrators, including persistence, credentials, programs, startup
and updates. Unknown stays restricted; retain identity, marking and domain checks.
Supersedes earlier administrator feature restrictions below. See
low-level-design/administrator-functional-parity.md.

The owner accepted explicit administrator SSH/SFTP access while retaining other
elevation safeguards, a libghostty-vt application-engine replacement, and an optional
default saved SSH session for application startup.

## Architecture

### Terminal New SSH context-menu entry (2026-11-14)

Native, legacy and empty-Space terminal menus expose the same New SSH submenu.
Quick Connect... comes first, followed by saved SSH sessions in store order,
with existing group headings. No saved sessions is a disabled placeholder.
Read the list on every menu open and dispatch saved clicks by stable id through
WorkspaceCommands; deletion must never select a different session. Existing
dialogs own authentication, host-key approval, native/legacy factory selection
and new-tab placement. Opening this menu never starts a network connection.
Unknown-token restrictions remain; confirmed administrators keep explicit
connections but no new saved-session management entry is added.

```text
Terminal right-click
  New Terminal [Here]
  New SSH > Quick Connect...
            ----------------
            Ungrouped session
            Group heading
            Saved session
  ... existing split, edit and close actions unchanged ...
```

### Native feature-completeness requirements (owner amendment 2026-11-14)

The direct-only native slices below are staging boundaries, not final product
requirements. The owner requests these capabilities be brought to parity:

- Native OSC 52 clipboard read/write, through the native engine's supported
  event seam and existing terminal security policy, never a second VT engine.
  Preserve remote default-deny gates, live read permission, bounded payloads,
  originating-session replies and suppression after exit/shutdown. Explicit
  user copy/paste remains independent.
- Jump-host routes: reuse the existing ordered route and per-hop credentials,
  host-key approval, cancellation and deadlines. Target key approval never
  approves a hop, and hop approval never approves the target. Preserve the
  existing four-hop/cycle validation and close downstream before upstream.
- Local, remote and dynamic port forwards: reuse current loopback defaults,
  explicit configured endpoints and warning-on-bind-failure semantics. Never
  silently ignore configured forwarding or infer permission to broaden binds.
  Session close must cancel and release listeners/relays. SSH-agent forwarding
  remains a separately scoped unsupported option.
- Native local and SSH output logging: reuse terminal-logging.md capture,
  precedence, file collision, manual Start/Stop, indicators and failure rules.
  Capture printable output only, not input or control/OSC payloads. Preserve
  elevated-process restrictions, isolated configuration and observable failures.

US-0254 through US-0257 implement these four capabilities with separate detail
designs and focused native integration proof. OSC 52 uses the pinned native
parser's OSC boundaries rather than synchronous clipboard callbacks, preserving
asynchronous UI consent and avoiding premature read replies. Native local/SSH
pumps share the existing printable logger and expose manual Log Start/Stop,
recording state and errors. The SSH owner retains ordered authenticated carriers
and cancels/observes forwarding tasks before target and reverse carrier release.
Agent forwarding remains rejected. These are isolated native-entry capabilities,
not a production cutover or external-server/physical owner acceptance. No
schema/dependency or implicit permission changes are authorized.

Split-close acceptance: the terminal and empty-Space right-click menus expose
Close Space when siblings exist. It targets the clicked Space id (not whichever
Space was active earlier), shuts down only that session and collapses the tree.
The surviving last Space becomes the ordinary borderless terminal. The distinct
Close Terminal Tab command still closes the whole tab; do not conflate them.

```text
Terminal tab
+----------------------+----------------------+
| Space A (keep alive)  | Space B (right-click) |
|                      |  ...                 |
|                      |  Close Space         |
|                      |  Close Terminal Tab  |
+----------------------+----------------------+
Close Space on B -> A remains in the same tab
```


US-0252 (owner continuation 2026-11-14) adds optional SFTP to native direct SSH
on the same authenticated connection and existing browser. Its explicit task
ownership, native capability/cwd publication and cancellation/teardown contract
are in low-level-design/native-sftp.md. Prior terminal-only slices remain
historical evidence, not SFTP parity proof.

- Keep core/settings/state/workspace/feature/backend layering unchanged.
- Actions classify SSH connection and SFTP operations as allowed in elevated windows.
  Session-store mutation actions remain unavailable; unclassified actions fail closed.
- Build the SSH Client dock in administrator windows, but retain the fixed startup
  layout and all config-write guards. A failed token query still hides network surfaces.
- Session-ui owns a single optional startup session id, presented as a checked
  session context-menu item. Selecting another session replaces it; selecting it
  again disables startup connection. Only ordinary windows can change the preference.
- App requests startup connection once after the main window's Root is ready.
  Skip it when pending crash reports need the recovery dialog.
  Administrator launches continue to answer the requested elevated local shell,
  rather than automatically consuming an unprivileged network configuration.
- Startup uses the ordinary saved-session dialog, with one automatic attempt when
  credentials are ready. Missing credentials remain interactive, and a locked vault
  still requires explicit unlock. Host-key approval and cancellation are unchanged.
- The application engine migration must use supported upstream APIs, not a fork or
  dependency rename. Preserve oneterm-vt for external consumers until migration is proven.
- libghostty-vt owns the application's incremental VT parser, screen, history and
  modes. ConPTY/openpty and russh remain transports; GPUI remains the renderer.
  Reply callbacks append owned bytes to the existing originating transport queue.
- Keep FFI handles and borrowed render iterators inside the terminal adapter's
  lock. Publish owned frame data, not C pointers. Never run two state engines on
  production output to fill gaps in the migration.
- Pin upstream source and the Zig toolchain; use the upstream C ABI rather than
  an unverified wrapper. See `low-level-design/ghostty-engine.md` for the migration
  boundary and validation sequence. DEC-0014 remains the shipping-engine record
  until the application cutover is proven; this intake authorizes that change,
  not removal of the external oneterm-vt package.

## UI Wireframe

Owner-requested executable setting (US-0250): General > Shell reuses the existing
optional shell.program input for PowerShell 7. A blank path selects ordinary
auto-detection; a provided path keeps Pwsh integration arguments. Restricted
processes continue to resolve trusted executables independently of configuration.

```text
General / Shell
  Shell                 [ PowerShell 7                  v ]
  Startup Executable    [ D:\Program\PowerShell\7\pwsh.exe ]
  Empty: auto-detect. Administrator windows use trusted paths.
```

Historical US-0245 fixture (superseded at runtime by US-0249 below): an independent default-off native-desktop binary in the app
composition root. It opens only a local native surface, never the normal workspace
or factory. As amended by owner UAT on 2026-11-13, no-argument startup has no
elevation opt-in gate and uses one title/banner. The actual token is still recorded
for underlying checks; it is not spoofed, changed or automatically elevated.
The shell inherits the caller's privileges and is not sandboxed. This does not prove
ordinary-token behavior or change production permissions. Startup reads no persisted
configuration, saved sessions or vault. A dedicated consumer drains every reliable
event even when the window is not painting. Program-controlled clipboard and
notification effects are denied; explicit user copy/paste remains surface-owned.
Bounded status reports exit, closure, denied effects and input failures. Closing
requests stop and keeps explicit-quit processing alive for bounded asynchronous
surface cleanup; after the GUI loop returns the launcher observes native destruction
and consumer termination. This is an acceptance tool, not production parity.

US-0249 (owner-selected, implementation and verification in progress) integrates native sessions into the actual
OneTerm workspace: existing title bar, tab strip, Space layout, right SSH Client
dock, theme/settings and status bar. Do not substitute a lookalike window or open
legacy sessions invisibly. The explicit test executable remains separate from
production default selection. A native session/service seam must be accepted in
detail before implementation; legacy TerminalSession's render handle cannot be
faked with an empty oneterm-vt engine. Native local and SSH ownership, event/security
handling, settings and teardown must stay explicit. Preserve production permission
rules; removing the probe startup flag does not authorize a global policy rewrite.

The owner selected persistent isolated `oneterm-native-data` storage beside the
test EXE. Root selection precedes all feature initialization; do not import or
fall back to production data. The actual workspace/local-startup route is wired
and headless proof covers a real native local session, but immediate/admitted-work
close and real-window acceptance remain open. No automatic saved SSH connection
or update check runs. US-0251 implements manual direct native SSH with explicit
preflight rejection of unsupported configurations rather than legacy fallback.
External-server desktop acceptance is separate. The standalone source remains
a test fixture only. This is not authorization to distribute an unverified build.

```text
OneTerm workspace / native test entry
+-------------------------------------------------------------+
| Existing OneTerm title bar and menus                         |
| Existing terminal tabs / Space layout | SSH Client dock      |
| Native terminal rendering and input   | Sessions / SFTP      |
| Existing status bar / theme / settings                       |
+-------------------------------------------------------------+
```

```text
OneTerm Native Desktop Probe (not production)
+-------------------------------------------------------------+
| LOCAL / NATIVE ONLY - OSC clipboard and notifications denied  |
| native terminal surface (Ctrl+F search)                       |
| state: running/exit/closed | denied effects | last input error |
+-------------------------------------------------------------+
```

```text
Administrator window
+---------------------------+--------------------+
| local / SSH terminal tabs  | SSH Client         |
|                           | Sessions | SFTP    |
+---------------------------+--------------------+
| OneTerm (Administrator); settings remain read-only |

Saved session context menu (ordinary window)
+-------------------------------+
| Open                          |
| Properties                    |
| Duplicate Saved Session       |
| Move to Group >               |
| [x] Connect on Startup        |
| ----------------------------- |
| New Session                   |
| ----------------------------- |
| Delete                        |
+-------------------------------+

Independent native terminal surface (default-off staging)
+-------------------------------------------------------------+
| native rows     [Find in terminal] [1 / 3] [up] [down] [close] |
| highlighted native matches; active match is distinguished     |
| Ctrl+F opens/closes; Enter / Shift+Enter navigate; Esc closes  |
+-------------------------------------------------------------+
```

## Data ownership and interfaces

US-0258 (owner approved 2026-11-15) adds confirmed-administrator Session creation:
the existing header plus/action/blank-area menu open the form and Save performs a
separate background locked append. Existing saved-session mutations, startup and
vault restrictions remain; unknown elevation cannot create. Detail contract:
low-level-design/administrator-session-create.md.

```text
Administrator SSH Client / Sessions
+---------------------------------------+
| Session                           [+] |
| Search sessions                       |
| existing sessions (Open only)         |
| blank-area menu: New Session          |
+---------------------------------------+
New SSH Session: existing metadata form
Password note: enter password when connecting
Save -> background append -> saved / error notification
```

US-0251 stages manual direct native SSH terminal creation through a tagged
factory result and the existing auth/trust dialogs. Its accepted migration
boundary is low-level-design/native-ssh.md: no production selection change,
automatic connection, legacy fallback or silent omission of configured services.
US-0252 adds same-connection SFTP; US-0254 through US-0257 extend the initial
direct-only slice with clipboard, logging, jump routes and forwarding. Only
agent forwarding remains an explicit unsupported preflight option.
Implementation includes loopback trust/authentication, native I/O/lifecycle and
tagged UI placement proof; the work packet records current gate/release evidence
and remaining desktop acceptance. Pending attempts and unplaced cleanup remain
workspace close obligations, not fabricated native lifecycle reports. Native
trust stays in the isolated root for target and every hop; clipboard effects
retain existing remote origin and live UI permission gates.

ssh_session.json retains schema v2 with one optional startup_session_id. No new
secret field or settings-to-feature dependency is introduced. Session-ui exposes
one startup entry point to the composition root. Stable ids, not row positions,
identify the selected session; duplicate sessions never inherit the preference.
Legacy schema migration or identity repairs disable the preference rather than
letting a reassigned id target a different server.

## Verification and migration stop condition

Unit tests cover action policy, persistence, selection/deletion, and startup
readiness. Run the full project gate and report unavailable platform proof.
Do not switch an engine unless both local PTY and SSH byte-stream parsing preserve
replies, security events, selection/search, and renderer semantics.
