//! Incremental native search. Returned coordinates expire on terminal mutation;
//! they are not stable history anchors or legacy search matches.

use std::ffi::{c_int, c_void};
use std::fmt;
use std::marker::PhantomData;
use std::ptr::{self, NonNull};
use std::rc::Rc;
use std::sync::Arc;

use super::anchors::Coordinate;
use super::selection::{GridRef, NativeSelection};
use super::{Error, Terminal, check};

pub const MAX_NEEDLE_BYTES: usize = 4096;
pub const MAX_MATCHES: usize = 4096;

#[derive(Debug, Default, Clone, Copy)]
pub struct Options {
    pub case_sensitive: bool,
    pub whole_word: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchError {
    NeedleLimit,
    MatchLimit,
    UnsupportedOptions,
    Expired,
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NeedleLimit => "Ghostty search needle exceeds 4096 bytes",
            Self::MatchLimit => "Ghostty search exceeds the 4096-match response limit",
            Self::Expired => "Native search coordinates expired; refresh the search",
            Self::UnsupportedOptions => {
                "Native Ghostty search does not support case-sensitive or whole-word options"
            }
        })
    }
}

impl std::error::Error for SearchError {}

pub fn validate(needle: &str, options: Options) -> Result<(), SearchError> {
    if needle.len() > MAX_NEEDLE_BYTES {
        return Err(SearchError::NeedleLimit);
    }
    if options.case_sensitive || options.whole_word {
        return Err(SearchError::UnsupportedOptions);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    FeedRequired,
    Complete,
}

/// Inclusive endpoints in native Screen space, valid for this response only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub start: Coordinate,
    pub end: Coordinate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub alternate_screen: bool,
    pub status: Status,
    /// Newest to oldest, as defined by native search.
    pub matches: Vec<Match>,
    /// Opaque native grid revision captured with these ephemeral coordinates.
    pub(crate) grid_revision: Option<Arc<()>>,
}

impl Snapshot {
    pub(crate) fn attach_revision(&mut self, revision: Arc<()>) {
        self.grid_revision = Some(revision);
    }

    pub fn navigation(&self, index: usize) -> Option<Navigation> {
        Some(Navigation {
            grid_revision: self.grid_revision.clone()?,
            row: self.matches.get(index)?.start.row,
        })
    }
}

/// A short-lived reveal request. The dispatcher checks the revision before scroll.
pub struct Navigation {
    pub(crate) grid_revision: Arc<()>,
    pub(crate) row: u32,
}

pub(super) enum Command {
    Set(String),
    Step,
}

/// One bounded progress operation, serialized with transport controls.
pub enum Request {
    Set(String, Options),
    Step,
    Reveal(Navigation),
}

impl Request {
    pub fn validate(&self) -> Result<(), SearchError> {
        match self {
            Self::Set(needle, options) => validate(needle, *options),
            Self::Step | Self::Reveal(_) => Ok(()),
        }
    }
}

#[repr(C)]
struct NativeString {
    ptr: *const u8,
    len: usize,
}

#[repr(C)]
struct SelectionBuffer {
    ptr: *mut NativeSelection,
    cap: usize,
    len: usize,
}

unsafe extern "C" {
    fn ghostty_search_new(
        allocator: *const c_void,
        out: *mut *mut c_void,
        terminal: *mut c_void,
    ) -> c_int;
    fn ghostty_search_free(search: *mut c_void);
    fn ghostty_search_set(search: *mut c_void, option: c_int, value: *const c_void) -> c_int;
    fn ghostty_search_get(search: *mut c_void, data: c_int, value: *mut c_void) -> c_int;
    fn ghostty_search_feed(search: *mut c_void) -> c_int;
    fn ghostty_search_tick(search: *mut c_void, status: *mut c_int) -> c_int;
    fn ghostty_terminal_point_from_grid_ref(
        terminal: *mut c_void,
        reference: *const GridRef,
        tag: c_int,
        out: *mut Coordinate,
    ) -> c_int;
}

pub(super) struct Search {
    raw: NonNull<c_void>,
    _thread_confined: PhantomData<Rc<()>>,
}

impl Search {
    pub fn new(terminal: &mut Terminal) -> Result<Self, Error> {
        let mut raw = ptr::null_mut();
        // SAFETY: registration and all later access happen on the owner worker.
        check(unsafe { ghostty_search_new(ptr::null(), &mut raw, terminal.raw.as_ptr()) })?;
        Ok(Self {
            raw: NonNull::new(raw).ok_or(Error::NullHandle)?,
            _thread_confined: PhantomData,
        })
    }

    pub fn execute(
        &mut self,
        terminal: &mut Terminal,
        command: Command,
    ) -> Result<Result<Snapshot, SearchError>, Error> {
        if let Command::Set(needle) = command {
            let needle = NativeString {
                ptr: needle.as_ptr(),
                len: needle.len(),
            };
            // SAFETY: pinned key 1 copies the string during the call, including
            // the empty string which clears native tracked search state.
            check(unsafe {
                ghostty_search_set(self.raw.as_ptr(), 1, ptr::from_ref(&needle).cast())
            })?;
        }
        // SAFETY: same-worker exclusive terminal access. One feed and one tick,
        // never the unbounded native run/select convenience operations.
        check(unsafe { ghostty_search_feed(self.raw.as_ptr()) })?;
        let mut status = 0;
        check(unsafe { ghostty_search_tick(self.raw.as_ptr(), &mut status) })?;
        let status = match status {
            0 => Status::Running,
            1 => Status::FeedRequired,
            2 => Status::Complete,
            _ => return Err(Error::InvalidSnapshot("unknown search status")),
        };
        let alternate_screen = terminal.metadata()?.alternate_screen;
        let mut buffer = SelectionBuffer {
            ptr: ptr::null_mut(),
            cap: 0,
            len: 0,
        };
        // SAFETY: null/zero queries capacity of the pinned selection buffer.
        let result =
            unsafe { ghostty_search_get(self.raw.as_ptr(), 6, ptr::from_mut(&mut buffer).cast()) };
        if result != -3 {
            check(result)?;
        }
        if buffer.len > MAX_MATCHES {
            return Ok(Err(SearchError::MatchLimit));
        }
        let mut selections = Vec::new();
        selections
            .try_reserve_exact(buffer.len)
            .map_err(|_| Error::Allocation)?;
        selections.resize_with(buffer.len, NativeSelection::default);
        buffer.ptr = selections.as_mut_ptr();
        buffer.cap = selections.len();
        // SAFETY: initialized output slots, no mutation since query.
        check(unsafe {
            ghostty_search_get(self.raw.as_ptr(), 6, ptr::from_mut(&mut buffer).cast())
        })?;
        if buffer.len > selections.len() {
            return Err(Error::InvalidSnapshot(
                "search exceeded queried match count",
            ));
        }
        let mut matches = Vec::new();
        matches
            .try_reserve_exact(buffer.len)
            .map_err(|_| Error::Allocation)?;
        for selection in selections.iter().take(buffer.len) {
            let convert = |reference: &GridRef| -> Result<Coordinate, Error> {
                let mut coordinate = Coordinate::default();
                // SAFETY: fresh untracked search refs of the active screen,
                // consumed before any terminal mutation. Screen tag is 2.
                check(unsafe {
                    ghostty_terminal_point_from_grid_ref(
                        terminal.raw.as_ptr(),
                        reference,
                        2,
                        &mut coordinate,
                    )
                })?;
                Ok(coordinate)
            };
            matches.push(Match {
                start: convert(&selection.start)?,
                end: convert(&selection.end)?,
            });
        }
        Ok(Ok(Snapshot {
            alternate_screen,
            status,
            matches,
            grid_revision: None,
        }))
    }
}

impl Drop for Search {
    fn drop(&mut self) {
        // SAFETY: unique registered search; worker drops it before terminal.
        unsafe { ghostty_search_free(self.raw.as_ptr()) };
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
