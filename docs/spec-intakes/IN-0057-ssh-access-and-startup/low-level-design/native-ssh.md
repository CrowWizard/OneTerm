# Native direct SSH workspace integration

## Boundary

US-0251 connects explicitly requested direct SSH terminals in the isolated
native workspace. Production continues to select the legacy factory. The
existing accepted intake authorizes native SSH; this first slice is direct
terminal only, not SSH/SFTP parity or production cutover.

## Ownership and interfaces

SessionFactory adds create_ssh returning the existing tagged CreatedSession;
the default delegates to legacy connect_ssh. Native-only composition overrides
creation and continues rejecting explicit legacy connect_ssh. Session-ui uses
the tagged result for the existing dialog, retry and destination placement.
Duplicate completion carries that same tag rather than fabricating a legacy
render engine. Feature flags are explicit across app/session-ui/backend.

The SSH backend reuses ConnectPhases, ordered connect_hop carriers, the existing
SshClientHandler, authentication helpers, remote environment and shell bootstrap.
Keep the 60-second overall and 20-second per-phase deadlines and cancellation.
Unknown-key confirmation, changed-key rejection, zeroizing credentials and
keepalive policy remain unchanged. No network runs at native startup.

NativeChannel owns the authenticated handle, actual native pump, remote-origin
OscRouter with the supplied security policy, configured scrollback, one reliable
event receiver and consuming completion. No oneterm-vt terminal is constructed.
Only one UI consumer takes events. Close/drop signal the original owner;
completion reports physical/native teardown and actual lifecycle delivery,
including cancellation rather than inventing success. Existing pane/window
five-second and app-quit 200 ms budgets remain authoritative.
Pending connection guards carry cancellation and a separate attempt-completion
type. Placement transfers responsibility to the registered pane. A missing
destination registers unplaced consuming completion in the workspace snapshot;
initial-CWD admission failure awaits cleanup under the connection guard. Event
draining is independent of painting and the foreground window. Cancellation of
an observer is a visible failure, never a synthesized successful owner report.

## Native capability extensions and remaining exclusions

US-0254 through US-0257 extend the initial direct-only slice, following
native-osc52.md, native-logging.md, native-jump-routes.md and native-forwards.md.
The native factory now retains JumpHandles in Services, uses isolated trust for
each hop/target, initializes logging before output and starts configured forwards
against the target. Native ownership services forward-open requests concurrently
with terminal I/O; TaskTracker and whole-operation cancellation cover listener,
relay and unfinished SOCKS handshake destruction before completion. Target and
same-connection SFTP teardown precede reverse-order carrier release. Setup
failure drops owned handles and signals forwarding cancellation. Existing
four-hop/cycle validation remains in session-ui's route resolver.

Before network/authentication, reject agent forwarding with a corrective
message. Authentication through the local agent remains supported; forwarding
it is a separate effect. US-0252 opens optional same-connection SFTP.
No credentials, daily settings or layouts are imported. Existing isolated
storage and token-derived restrictions apply without any policy bypass.
Native connection orchestration selects config_dir()/known_hosts explicitly;
the isolated entry therefore never reads or appends the daily OpenSSH trust
file. The shared strict verification/approval logic is unchanged.
Native OSC 52 observes bounded completed native-parser OSC sequences and emits
owned events through OscRouter. Remote reads/writes default off. Read replies
await the existing live UI permission gate and go only to the source session;
native synchronous callbacks stay disabled to avoid premature empty responses.
Invalid, cancelled or oversized payloads are inert. No second screen engine
or clipboard effect after pane shutdown is introduced.

## Proof

Use in-process loopback SSH fixtures and project-local temporary trust data;
never contact saved or external hosts. Verify preflight, trust/auth failures,
cancel, native publication/input/resize/replies, remote OSC policy, exit/close
and consuming completion; verify tagged UI placement/duplicate behavior and
production default compatibility. Run native regression/Clippy and ci-local.
Real external-server desktop acceptance remains separate and needs an explicit
user-selected target and credentials; never infer it from loopback success.
