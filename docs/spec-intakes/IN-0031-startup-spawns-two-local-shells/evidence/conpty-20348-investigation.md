# ConPTY orphan gate investigation on Windows 20348

Date: 2026-11-12
Classification: read-only investigation; no production or test implementation edited.
Owning contract: BUG-0055, DEC-0016, docs/terminal-backend.md sections 6.2/6.3.

## Finding

The reproduced failures are exit-reason assertion failures, not observed orphan
processes. The regression probe's common exit-code whitelist excludes the system
console host's ERROR_BROKEN_PIPE (109 / 0x6d). All measured processes exited inside
the existing five-second liveness bound without probe termination.

This does not establish general absence of leaks, bundled-host behavior, or the
application-quit guarantee that DEC-0016 explicitly excludes.

## Environment and route

- OS reported by System.Environment.OSVersion: 10.0.20348.0.
- C:\Windows\System32\conhost.exe file version: 10.0.20348.1000.
- Bundled conpty.dll and x64/OpenConsole.exe: 1.24.2607.10001.
- Test executable: target/debug/deps/oneterm_local_shell-24008dfa53569970.exe.
- No conpty.dll was present in target/debug/deps. ConptyApi::resolve() looks
  beside the running executable, not its parent directory. The probe observed
  conhost.exe, consistent with the system fallback, not bundled OpenConsole.exe.
- Native staging was not enabled. The only existing diff in the orphan test
  moves its shared spawn lock import/comment; its exit-code assertion is unchanged.
- No diff exists in the inspected production windows.rs, conpty.rs or child.rs.

## Reproduction and measured results

Both commands used project-local CARGO_HOME, CARGO_TARGET_DIR and TEMP/TMP, with
CARGO_BUILD_JOBS=4.

```text
cargo test -p oneterm-local-shell --lib session_orphan_tests -- --nocapture --test-threads=1
cargo test -p oneterm-local-shell --lib orphan_liveness_table -- --ignored --nocapture --test-threads=1
```

Focused run: 0 passed, 2 failed, 1 ignored. Both failures occur at the exit-code
assertion, on conhost.exe 0x6d, at immediate drop and first output. Serial execution
reproduces the failure, so workspace test concurrency is not required.

Existing diagnostic sweep: 30 sessions, five each at 0, 5, 10, 50, 500 ms and
first output. All 60 watched processes exited; none required probe termination.

| Process | Count | Exit code | Observed after drop |
| --- | ---: | --- | --- |
| conhost.exe | 30 | 0x6d | 20-41 ms |
| cmd.exe | 29 | 0xc000013a | 20-41 ms |
| cmd.exe | 1 | 0x1 | 21 ms |

These are polling observations, not precise exit latency: the sampling interval is
20 ms. The ignored sweep prints measurements and has no exit-code whitelist
assertion; its passing status alone is not regression-test acceptance.

Raw logs: .tmp/conpty-orphan-focused.log and .tmp/conpty-orphan-liveness.log.
No source instrumentation, policy changes, or termination of unrelated processes
was used. Probe cleanup is scoped to newly added direct children held by handles.

## Source evidence

Current production teardown preserves the established order:

1. LocalSession drop requests shutdown and detaches owner reaping.
2. The PTY owner returns and drops PseudoConsole.
3. Conpty closes first while the output reader still exists.
4. ChildExitWatcher waits up to two seconds, escalating only its own child handle.

The whitelist in session_orphan_tests.rs currently accepts 0, 1, 0xc000013a and
0xc0000142 for every watched process. Failure at that assertion means the earlier
no-probe-termination assertion has passed for that row; it is not a liveness failure.

Microsoft's historical terminal v1.7.1091.0 source explicitly documents and uses
the observed exit code:

- [PtySignalInputThread.cpp](https://github.com/microsoft/terminal/blob/v1.7.1091.0/src/host/PtySignalInputThread.cpp#L187-L215):
  _Shutdown handles the signal pipe breaking when the parent dies or calls
  ClosePseudoConsole; after CloseConsoleProcessState and ProcessCtrlEvents it
  calls ServiceLocator::RundownAndExit(ERROR_BROKEN_PIPE).
- [VtIo.cpp](https://github.com/microsoft/terminal/blob/v1.7.1091.0/src/host/VtIo.cpp#L339-L389):
  _ShutdownIfNeeded also calls RundownAndExit(ERROR_BROKEN_PIPE) after the VT
  input/output sides disconnect.

Local copies inspected: .tmp/conpty-v1.7-PtySignalInputThread.cpp,
.tmp/conpty-v1.7-VtIo.cpp and .tmp/conpty-v1.7-VtInputThread.cpp. Windows' own
Win32Exception(109) identifies the code as broken pipe. This source establishes
that 109 can be an intentional legacy host teardown result; it is corroborating
source, not an exact source/symbol match to the installed Windows binary. No
debugger stack was captured from that binary.

The earlier BUG-0055 measurements were on Windows 11 build 26200, where conhost
was recorded exiting with zero. Those observations are not a cross-version
contract for the console host's exit code.

## Recommendation and remaining proof

Treat a subsequent correction as a separately documented test-portability outcome:
recognize measured ERROR_BROKEN_PIPE for the verified console-host role only,
while retaining the five-second liveness bound, the no-probe-termination check,
strict shell exit reasons, handle ownership and unknown-code rejection. Do not
globally add 109 for every process, remove the exit-reason assertion, increase the
bound, or alter production teardown merely to turn this gate green.

Before accepting that correction, prove that shell 109 and unrelated host codes
are still rejected, rerun the two real-host regressions and complete the bundled
gate. Bundled-host teardown requires a separate correctly staged probe; the current
system-host sweep cannot claim it. A real escalation fault proof and real process
exit acceptance remain distinct from this no-survivor sample.

No full gate rerun was performed for this read-only investigation. The previous
.tmp/us0246-ci.log failure remains the full-workspace status.
