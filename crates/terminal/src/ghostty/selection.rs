//! Short-lived native selection snapshots and bounded owned text. No clipboard I/O.

use std::ffi::{c_int, c_void};
use std::fmt;
use std::ptr::{self, NonNull};

use super::anchors::{AnchorError, AnchorId, Anchors};
use super::{Error, Terminal, check};

pub const MAX_COPY_BYTES: usize = 1024 * 1024;

/// Owned session update. Endpoint provenance/liveness is checked by the native
/// owner at execution time, not inferred from a stale published frame.
#[derive(Debug, Clone)]
pub enum Update {
    All,
    Gesture(Gesture),
    Set {
        start: AnchorId,
        end: AnchorId,
        rectangle: bool,
    },
    Clear,
}

/// Identity only: no native pointer or row ordering escapes the owner.
#[derive(Debug, Default, Clone)]
pub struct GestureId(std::sync::Arc<()>);

impl GestureId {
    pub(crate) fn same(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Clone)]
pub enum Gesture {
    Begin {
        id: GestureId,
        viewport: crate::ghostty_content::Viewport,
        coordinate: super::anchors::Coordinate,
    },
    Move {
        id: GestureId,
        viewport: crate::ghostty_content::Viewport,
        coordinate: super::anchors::Coordinate,
    },
    Finish(GestureId),
}

impl std::fmt::Debug for Gesture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Begin { .. } => "Begin",
            Self::Move { .. } => "Move",
            Self::Finish(_) => "Finish",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionError {
    Anchor(AnchorError),
    InactiveScreen,
    Expired,
    TextLimit,
}

impl fmt::Display for SelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Anchor(error) => error.fmt(f),
            Self::InactiveScreen => {
                f.write_str("Ghostty selection endpoints must belong to the active screen")
            }
            Self::Expired => f.write_str("Ghostty selection endpoint is no longer present"),
            Self::TextLimit => f.write_str("Ghostty selection text exceeds 1 MiB"),
        }
    }
}

impl std::error::Error for SelectionError {}

// Never returned to callers: valid only inside a serialized worker operation.
#[repr(C)]
#[derive(Default)]
pub(super) struct GridRef {
    node: *mut c_void,
    x: u16,
    y: u16,
}

#[repr(C)]
#[derive(Default)]
pub(super) struct NativeSelection {
    pub start: GridRef,
    pub end: GridRef,
    pub rectangle: bool,
}

pub(super) enum Command {
    All,
    Set(AnchorId, AnchorId, bool),
    Clear,
    Copy,
}

unsafe extern "C" {
    fn ghostty_terminal_select_all(terminal: *mut c_void, out: *mut NativeSelection) -> c_int;
    fn ghostty_formatter_terminal_new(
        allocator: *const c_void,
        out: *mut *mut c_void,
        terminal: *mut c_void,
    ) -> c_int;
    fn ghostty_formatter_free(formatter: *mut c_void);
    fn ghostty_formatter_set(formatter: *mut c_void, option: c_int, value: *const c_void) -> c_int;
    fn ghostty_formatter_format_buf(
        formatter: *mut c_void,
        buf: *mut u8,
        capacity: usize,
        written: *mut usize,
    ) -> c_int;
}

pub(super) fn execute(
    terminal: &mut Terminal,
    anchors: &Anchors,
    command: Command,
) -> Result<Result<Option<String>, SelectionError>, Error> {
    match command {
        Command::All => {
            let mut selection = NativeSelection::default();
            // SAFETY: confined live terminal and writable pinned selection layout.
            // Install immediately; no mutation can expire this temporary snapshot.
            let status =
                unsafe { ghostty_terminal_select_all(terminal.raw.as_ptr(), &mut selection) };
            let value = if status == -4 {
                ptr::null()
            } else {
                check(status)?;
                ptr::from_ref(&selection).cast()
            };
            // SAFETY: key 22 copies the live snapshot; null clears selection.
            check(unsafe { super::ghostty_terminal_set(terminal.raw.as_ptr(), 22, value) })?;
            Ok(Ok(None))
        }
        Command::Set(start, end, rectangle) => anchors
            .set_selection(terminal, &start, &end, rectangle)
            .map(|result| result.map(|()| None)),
        Command::Clear => {
            // SAFETY: null explicitly clears the active screen selection.
            check(unsafe { super::ghostty_terminal_set(terminal.raw.as_ptr(), 22, ptr::null()) })?;
            Ok(Ok(None))
        }
        Command::Copy => copy(terminal, MAX_COPY_BYTES),
    }
}

fn copy(
    terminal: &mut Terminal,
    limit: usize,
) -> Result<Result<Option<String>, SelectionError>, Error> {
    let mut selection = NativeSelection::default();
    // SAFETY: pinned key 31 writes the checked selection layout. No terminal
    // mutation occurs until the formatter and this snapshot have been dropped.
    let status = unsafe {
        super::ghostty_terminal_get(
            terminal.raw.as_ptr(),
            31,
            ptr::from_mut(&mut selection).cast(),
        )
    };
    if status == -4 {
        return Ok(Ok(None));
    }
    check(status)?;
    let mut raw = ptr::null_mut();
    // SAFETY: default allocator; terminal outlives this function-local formatter.
    check(unsafe { ghostty_formatter_terminal_new(ptr::null(), &mut raw, terminal.raw.as_ptr()) })?;
    struct Formatter(NonNull<c_void>);
    impl Drop for Formatter {
        fn drop(&mut self) {
            // SAFETY: unique formatter freed before its borrowed terminal.
            unsafe { ghostty_formatter_free(self.0.as_ptr()) };
        }
    }
    let formatter = Formatter(NonNull::new(raw).ok_or(Error::NullHandle)?);
    let plain: c_int = 0;
    let enabled = true;
    for (option, value) in [
        (1, ptr::from_ref(&plain).cast()),
        (2, ptr::from_ref(&enabled).cast()),
        (3, ptr::from_ref(&enabled).cast()),
        (4, ptr::from_ref(&selection).cast()),
    ] {
        // SAFETY: pinned option types; formatter copies each value immediately.
        check(unsafe { ghostty_formatter_set(formatter.0.as_ptr(), option, value) })?;
    }
    let mut needed = 0;
    // SAFETY: null/zero is the documented sizing query, no output is written.
    let status = unsafe {
        ghostty_formatter_format_buf(formatter.0.as_ptr(), ptr::null_mut(), 0, &mut needed)
    };
    if status != -3 {
        check(status)?;
    }
    if needed > limit {
        return Ok(Err(SelectionError::TextLimit));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(needed)
        .map_err(|_| Error::Allocation)?;
    bytes.resize(needed, 0);
    let mut written = 0;
    // SAFETY: initialized writable buffer of the queried capacity, no intervening
    // mutation; even empty output uses a valid zero-length buffer pointer.
    check(unsafe {
        ghostty_formatter_format_buf(
            formatter.0.as_ptr(),
            bytes.as_mut_ptr(),
            bytes.len(),
            &mut written,
        )
    })?;
    if written > bytes.len() {
        return Err(Error::InvalidSnapshot(
            "formatter exceeded queried text size",
        ));
    }
    bytes.truncate(written);
    let text = String::from_utf8(bytes)
        .map_err(|_| Error::InvalidSnapshot("formatter returned invalid UTF-8"))?;
    Ok(Ok(Some(text)))
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
