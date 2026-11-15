//! `BUG-0055`: what a discarded [`LocalSession`] leaves behind on Windows.
//!
//! The probe spawns the shipped default shell (`cmd.exe /K chcp 65001 >nul`),
//! drops the session at a parameterised delay after spawn, and then polls the
//! liveness of every process the spawn added as a child of this test process —
//! the `cmd.exe` client and the console host that serves its pseudo-console.
//!
//! It only ever looks at, and only ever terminates, processes whose parent is
//! this test process and which were not there before the spawn. No process is
//! matched by name for termination.

use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::time::{Duration, Instant};

use oneterm_terminal::TerminalRender;
use windows_sys::Win32::Foundation::{
    ERROR_BROKEN_PIPE, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Storage::FileSystem::SYNCHRONIZE;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
    QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject,
};

use super::session_tests::{spawn_unguarded, wait_until};
use crate::spawn_guard::lock_spawns;

/// The exit codes a watched process is allowed to end with.
///
/// `0x0` is the console host exiting with its last client. `0xc000013a` is
/// `STATUS_CONTROL_C_EXIT`, the host's close request processed normally.
/// `0x1` is OneTerm's own `TerminateProcess(handle, 1)` — the `DEC-0016`
/// escalation, and the only code that proves the escalation ran. `0xc0000142`
/// is `STATUS_DLL_INIT_FAILED`, the fault a client can take when its console
/// disappears mid-start. Other shell codes remain failures. A verified legacy
/// system console host additionally permits broken pipe in `expected_exit`;
/// see the retained Windows 20348 investigation for that measured close path.
const EXPECTED_EXIT_CODES: [u32; 4] = [0x0, 0x1, 0xc000_013a, 0xc000_0142];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProcessRole {
    SystemConsoleHost,
    Other,
}

fn process_role(image: &str, system_directory: &str) -> ProcessRole {
    let host = format!("{}\\conhost.exe", system_directory.trim_end_matches('\\'));
    if image.eq_ignore_ascii_case(&host) {
        ProcessRole::SystemConsoleHost
    } else {
        ProcessRole::Other
    }
}

fn expected_exit(role: ProcessRole, code: u32) -> bool {
    EXPECTED_EXIT_CODES.contains(&code)
        // Legacy system ConPTY explicitly exits with broken pipe on close.
        || (role == ProcessRole::SystemConsoleHost && code == ERROR_BROKEN_PIPE)
}

fn system_directory() -> String {
    let mut buffer = vec![0u16; 32768];
    // SAFETY: buffer is writable for the supplied capacity.
    let length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    assert!(
        length > 0 && length < buffer.len(),
        "query system directory: {}",
        std::io::Error::last_os_error()
    );
    String::from_utf16(&buffer[..length]).expect("system directory UTF-16")
}

/// How long a survivor is given before it counts as an orphan.
const LIVENESS_BOUND: Duration = Duration::from_secs(5);
/// Poll interval for the liveness sampling.
const POLL: Duration = Duration::from_millis(20);
/// Repetitions per delay — the baseline in the packet says the teardown is racy.
const REPETITIONS: usize = 5;

/// When the session is dropped, relative to its spawn.
#[derive(Clone, Copy)]
enum DropAfter {
    Millis(u64),
    FirstOutput,
}

impl DropAfter {
    fn label(self) -> String {
        match self {
            Self::Millis(ms) => format!("{ms} ms"),
            Self::FirstOutput => "first output".to_owned(),
        }
    }
}

/// A process this probe is responsible for: opened before the drop so the pid
/// cannot be recycled underneath the measurement.
struct Watched {
    pid: u32,
    name: String,
    handle: OwnedHandle,
    role: ProcessRole,
}

impl Watched {
    fn raw(&self) -> HANDLE {
        self.handle.as_raw_handle() as HANDLE
    }

    fn exited(&self) -> bool {
        // SAFETY: the handle is live and was opened with SYNCHRONIZE.
        unsafe { WaitForSingleObject(self.raw(), 0) == WAIT_OBJECT_0 }
    }

    fn exit_code(&self) -> Option<u32> {
        let mut code = 0u32;
        // SAFETY: the handle is live and opened with QUERY_LIMITED_INFORMATION.
        let read = unsafe { GetExitCodeProcess(self.raw(), &mut code) };
        (read != 0).then_some(code)
    }

    /// Last resort for a process this probe started and that refuses to exit.
    fn terminate(&self) {
        // SAFETY: the handle is live and was opened with PROCESS_TERMINATE.
        unsafe { TerminateProcess(self.raw(), 1) };
    }
}

/// Every live process whose parent is `parent`, as `(pid, image name)`.
fn children_of(parent: u32) -> Vec<(u32, String)> {
    let mut found = Vec::new();
    // SAFETY: a process snapshot takes no input buffer.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return found;
    }
    // SAFETY: the snapshot handle is owned here and closed exactly once.
    let snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot as _) };
    let raw = snapshot.as_raw_handle() as HANDLE;

    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    // SAFETY: `entry` is sized as the API requires and lives for both calls.
    let mut more = unsafe { Process32FirstW(raw, &mut entry) };
    while more != 0 {
        if entry.th32ParentProcessID == parent {
            let end = entry
                .szExeFile
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(entry.szExeFile.len());
            found.push((
                entry.th32ProcessID,
                String::from_utf16_lossy(&entry.szExeFile[..end]),
            ));
        }
        // SAFETY: as above.
        more = unsafe { Process32NextW(raw, &mut entry) };
    }
    found
}

fn watch(pid: u32, name: String, system_directory: &str) -> Option<Watched> {
    // SAFETY: `pid` came from a snapshot; a failed open returns null.
    let handle = unsafe {
        OpenProcess(
            SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
            0,
            pid,
        )
    };
    if handle.is_null() {
        return None;
    }
    // SAFETY: `OpenProcess` succeeded, so the handle is owned here.
    let handle = unsafe { OwnedHandle::from_raw_handle(handle as _) };
    let mut image = vec![0u16; 32768];
    let mut length = image.len() as u32;
    // SAFETY: the query-capable handle is live; image and length are writable.
    let queried = unsafe {
        QueryFullProcessImageNameW(
            handle.as_raw_handle() as HANDLE,
            0,
            image.as_mut_ptr(),
            &mut length,
        )
    };
    assert_ne!(
        queried,
        0,
        "query image for {name} (pid {pid}): {}",
        std::io::Error::last_os_error()
    );
    let image = String::from_utf16(&image[..length as usize]).expect("process image UTF-16");
    let role = process_role(&image, system_directory);
    Some(Watched {
        pid,
        name,
        handle,
        role,
    })
}

/// One row of the liveness table.
struct Row {
    name: String,
    pid: u32,
    exited_after: Option<Duration>,
    exit_code: Option<u32>,
    terminated: bool,
    role: ProcessRole,
}

/// Spawn a default session, drop it after `delay`, and measure what survives.
fn probe(delay: DropAfter) -> Vec<Row> {
    let ours = std::process::id();

    // Held across both snapshots: a sibling test's shell starting in between
    // would be adopted here, waited out and terminated. Every spawn in this
    // binary takes the same lock (see `crate::spawn_guard`).
    let guard = lock_spawns();
    let system_directory = system_directory();
    let before: Vec<u32> = children_of(ours).into_iter().map(|(pid, _)| pid).collect();
    let session = spawn_unguarded(oneterm_core::LocalShellConfig::default()).expect("spawn");
    let watched: Vec<Watched> = children_of(ours)
        .into_iter()
        .filter(|(pid, _)| !before.contains(pid))
        .filter_map(|(pid, name)| watch(pid, name, &system_directory))
        .collect();
    drop(guard);

    match delay {
        DropAfter::Millis(ms) => std::thread::sleep(Duration::from_millis(ms)),
        DropAfter::FirstOutput => {
            assert!(
                wait_until(Duration::from_secs(10), || !session
                    .snapshot()
                    .text()
                    .trim()
                    .is_empty()),
                "the shell produced no output within 10 s"
            );
        }
    }

    let dropped = Instant::now();
    drop(session);

    let mut rows: Vec<Row> = watched
        .iter()
        .map(|process| Row {
            name: process.name.clone(),
            pid: process.pid,
            exited_after: None,
            exit_code: None,
            terminated: false,
            role: process.role,
        })
        .collect();

    let deadline = dropped + LIVENESS_BOUND;
    loop {
        let mut pending = false;
        for (row, process) in rows.iter_mut().zip(watched.iter()) {
            if row.exited_after.is_some() {
                continue;
            }
            if process.exited() {
                row.exited_after = Some(dropped.elapsed());
                row.exit_code = process.exit_code();
            } else {
                pending = true;
            }
        }
        if !pending || Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(POLL);
    }

    // Cleanup obligation: only processes this probe started, only by pid.
    for (row, process) in rows.iter_mut().zip(watched.iter()) {
        if row.exited_after.is_none() {
            process.terminate();
            row.terminated = true;
        }
    }

    rows
}

/// Every process the spawn added must be gone within [`LIVENESS_BOUND`], and
/// gone for a reason this packet measured.
fn assert_no_orphan(delay: DropAfter, runs: usize) {
    for run in 1..=runs {
        let rows = probe(delay);
        assert!(
            !rows.is_empty(),
            "the spawn added no child process, so the probe measured nothing"
        );
        for row in rows {
            assert!(
                !row.terminated,
                "run {run}: dropping a session after {} left {} (pid {}) alive for \
                 {LIVENESS_BOUND:?}; the probe had to terminate it",
                delay.label(),
                row.name,
                row.pid,
            );
            let code = row
                .exit_code
                .expect("an exited process must have an exit code");
            assert!(
                expected_exit(row.role, code),
                "run {run}: {} (pid {}, role {:?}) exited with an unexpected {code:#x} after a drop at {}",
                row.name,
                row.pid,
                row.role,
                delay.label(),
            );
        }
    }
}

#[test]
fn broken_pipe_is_only_accepted_for_the_verified_system_host() {
    let system = r"C:\Windows\System32";
    let host = process_role(r"c:\WINDOWS\system32\CONHOST.EXE", system);
    assert_eq!(host, ProcessRole::SystemConsoleHost);
    assert!(expected_exit(host, ERROR_BROKEN_PIPE));
    assert!(!expected_exit(host, 0xdead_beef));
    for image in [
        r"C:\Windows\System32\cmd.exe",
        r"D:\foreign\conhost.exe",
        "",
        r"C:\Windows\System32\conhost.exe.other",
    ] {
        let role = process_role(image, system);
        assert_eq!(role, ProcessRole::Other, "{image}");
        assert!(!expected_exit(role, ERROR_BROKEN_PIPE), "{image}");
        assert!(!expected_exit(role, 0xdead_beef), "{image}");
    }
    for role in [ProcessRole::SystemConsoleHost, ProcessRole::Other] {
        for code in EXPECTED_EXIT_CODES {
            assert!(expected_exit(role, code));
        }
    }
}

/// `BUG-0055`: the session is discarded while `cmd.exe` is still starting, so
/// the client never processes the exit request its host sends when the
/// pseudo-console closes. Without the bounded wait and escalation in
/// `ChildExitWatcher::drop` (`DEC-0016`) this leaves a `cmd.exe` and its console
/// host behind — measured, still alive 15 s later in 8 of 9 runs.
///
/// A green run is **not** proof that the escalation ran: on hardware where
/// `cmd.exe` finishes initialising before the spawn path returns, the orphan
/// never occurs and this passes vacuously. The exit codes say which happened —
/// `0x1` is the escalation, `0xc000013a` is the shell exiting on its own — and
/// the ignored `orphan_liveness_table` is the manual proof that prints them.
#[test]
fn a_session_dropped_before_its_shell_starts_leaves_no_orphan() {
    assert_no_orphan(DropAfter::Millis(0), REPETITIONS);
}

/// The case that already worked before `BUG-0055`, pinned so the fix cannot
/// regress it: a fully started shell exits on its own when the pseudo-console
/// closes, well inside the grace period.
#[test]
fn a_session_dropped_after_its_shell_is_up_leaves_no_orphan() {
    assert_no_orphan(DropAfter::FirstOutput, 1);
}

/// The `BUG-0055` diagnosis sweep, kept for the record. Ignored because it
/// starts a shell per row and waits out [`LIVENESS_BOUND`] for every survivor;
/// run it with
/// `cargo test -p oneterm-local-shell --lib -- --ignored --nocapture orphan_liveness_table`.
#[test]
#[ignore = "spawns real cmd.exe processes and can take minutes; run explicitly"]
fn orphan_liveness_table() {
    println!("| drop delay | run | process | pid | exited after | exit code | terminated |");
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    for delay in [
        DropAfter::Millis(0),
        DropAfter::Millis(5),
        DropAfter::Millis(10),
        DropAfter::Millis(50),
        DropAfter::Millis(500),
        DropAfter::FirstOutput,
    ] {
        for run in 1..=REPETITIONS {
            for row in probe(delay) {
                println!(
                    "| {} | {run} | {} | {} | {} | {} | {} |",
                    delay.label(),
                    row.name,
                    row.pid,
                    row.exited_after
                        .map_or_else(|| "never".to_owned(), |at| format!("{} ms", at.as_millis())),
                    row.exit_code
                        .map_or_else(|| "-".to_owned(), |code| format!("{code:#x}")),
                    if row.terminated { "yes" } else { "no" },
                );
            }
        }
    }
}
