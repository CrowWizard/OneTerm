# Native SFTP ownership

## Accepted outcome and boundary

US-0252 extends direct native SSH with optional SFTP on the same authenticated
russh handle. Existing backend/browser/path/transfer policy is reused. No second
login, legacy parser, production cutover, schema or permission change.

## Interfaces and ownership

NativeSession gains default-empty TerminalCapabilities. NativeSshSession
exposes a live SFTP backend, its native channel SharedState for cwd/counters,
and no unsupported logging controller. NativeTerminalPane and TerminalPane
forward capabilities to existing workspace-scoped AppState publication.

Open the subsystem and complete its handshake under existing ConnectPhases
and cancellation before transferring the russh handle. No task starts before
successful handshake. Failure other than cancellation warns and retains shell.
The native SFTP owner holds backend, cancellation and task JoinHandle, and is
moved into the native channel owner task. Its Drop cancels; normal completion
signals cancellation and awaits its task before dropping the SSH handle or
reporting native completion. UI-retained backend Arcs never own the connection
or prevent shutdown. Cancellation closes command admission and marks it dead.

## Bounded teardown

Reuse SFTP command dispatch/transfer implementation with an explicit native
shutdown policy. Native lookup is cancellation-selectable. Native shutdown
cancels the owned SFTP stream, aborts metadata request futures and observes
both request and transfer JoinSet drain. Transfers are not force-aborted:
transport cancellation wakes blocked reads/writes and lets staging cleanup and
result delivery finish. Native failed-download handle cleanup stays inside the
transfer, never a detached close task. The stream wrapper's destruction is
observed before consuming completion, including the library's split I/O tasks;
legacy retains its existing five-second drain policy. No extra timeout or delay
is added to pane/window five-second or app-quit 200 ms budgets. A cancelled
operation must return failure or lose its reply (mapped to a visible error),
never success. Task panic is a consuming-completion error.

## UI

Existing SSH Client dock and SFTP panel are reused unchanged:

```text
Native SSH tab / active Space --> workspace active backend + live cwd
                                      |
                               SFTP file browser
                               path / entries / transfers
Local tab / disconnected pane --> no usable remote backend
```

## Proof

Loopback one-login shell/SFTP, unsupported subsystem, setup cancellation,
metadata and transfer I/O, blocked lookup/transfer teardown with retained UI
backend, pane capability forwarding and existing workspace switching tests.
Full native regressions and local gate precede a preserved optimized build.
