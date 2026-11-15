//! Worker-owned native positions. Tokens carry Rust provenance, not native
//! pointers or ordered row numbers; only native tracked refs survive mutation.

use std::ffi::{c_int, c_void};
use std::fmt;
use std::marker::PhantomData;
use std::ptr::{self, NonNull};
use std::rc::Rc;
use std::sync::Arc;

use super::{Error, Terminal, check};

pub const MAX_ANCHORS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Space {
    Active,
    Viewport,
    Screen,
    History,
}

impl Space {
    fn tag(self) -> c_int {
        match self {
            Self::Active => 0,
            Self::Viewport => 1,
            Self::Screen => 2,
            Self::History => 3,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Coordinate {
    pub column: u16,
    pub row: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub space: Space,
    pub coordinate: Coordinate,
}

/// Rust identity only; the serial is neither a grid position nor a native id.
#[derive(Debug, Clone)]
pub struct AnchorId {
    namespace: Arc<()>,
    serial: u64,
}

impl PartialEq for AnchorId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.namespace, &other.namespace) && self.serial == other.serial
    }
}

impl Eq for AnchorId {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub coordinate: Coordinate,
    /// Coordinates refer to the anchor's owning screen, even when inactive.
    pub alternate_screen: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorError {
    InvalidPoint,
    Foreign,
    Released,
    Limit,
}

impl fmt::Display for AnchorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPoint => "Ghostty anchor point is outside its coordinate space",
            Self::Foreign => "Ghostty anchor belongs to another terminal",
            Self::Released => "Ghostty anchor has been released",
            Self::Limit => "Ghostty anchor registry reached its limit",
        })
    }
}

impl std::error::Error for AnchorError {}

#[repr(C)]
union PointValue {
    coordinate: Coordinate,
    padding: [u64; 2],
}

#[repr(C)]
struct NativePoint {
    tag: c_int,
    value: PointValue,
}

unsafe extern "C" {
    fn ghostty_terminal_grid_ref_track(
        terminal: *mut c_void,
        point: NativePoint,
        out: *mut *mut c_void,
    ) -> c_int;
    fn ghostty_tracked_grid_ref_free(anchor: *mut c_void);
    fn ghostty_tracked_grid_ref_snapshot(
        anchor: *mut c_void,
        out: *mut super::selection::GridRef,
    ) -> c_int;
    fn ghostty_tracked_grid_ref_point(
        anchor: *mut c_void,
        tag: c_int,
        out: *mut Coordinate,
    ) -> c_int;
}

struct Tracked {
    raw: NonNull<c_void>,
    _thread_confined: PhantomData<Rc<()>>,
}

impl Tracked {
    fn native_point(&self, space: Space) -> Result<Option<Coordinate>, Error> {
        let mut coordinate = Coordinate::default();
        // SAFETY: live worker-owned handle and writable pinned coordinate
        // layout. No untracked grid reference is created or retained.
        let status = unsafe {
            ghostty_tracked_grid_ref_point(self.raw.as_ptr(), space.tag(), &mut coordinate)
        };
        if status == -4 {
            return Ok(None);
        }
        check(status)?;
        Ok(Some(coordinate))
    }

    fn point(&self, terminal: &Terminal, space: Space) -> Result<Option<Coordinate>, Error> {
        let Some(coordinate) = self.native_point(space)? else {
            return Ok(None);
        };
        // Pinned PageList::pointFromPin checks the top but not the bottom of
        // each space. History must not silently include the active screen.
        if space == Space::History && self.native_point(Space::Active)?.is_some() {
            return Ok(None);
        }
        if matches!(space, Space::Active | Space::Viewport) {
            let mut rows = 0_u16;
            // SAFETY: key 2 writes uint16_t rows. Native resize updates both
            // screens to this height (Terminal.zig::resize); failed resize is
            // already fatal, so no subsequent anchor query runs in that state.
            check(unsafe {
                super::ghostty_terminal_get(
                    terminal.raw.as_ptr(),
                    2,
                    ptr::from_mut(&mut rows).cast(),
                )
            })?;
            if coordinate.row >= u32::from(rows) {
                return Ok(None);
            }
        }
        Ok(Some(coordinate))
    }
}

impl Drop for Tracked {
    fn drop(&mut self) {
        // SAFETY: unique live native tracked-ref owner. Upstream permits freeing
        // after terminal destruction, but the registry normally drops first.
        unsafe { ghostty_tracked_grid_ref_free(self.raw.as_ptr()) };
    }
}

struct Entry {
    serial: u64,
    alternate_screen: bool,
    tracked: Tracked,
}

pub(super) enum Command {
    Create(Point),
    Resolve(AnchorId, Space),
    Release(AnchorId),
}

pub(super) enum Reply {
    Created(AnchorId),
    Resolved(Option<Position>),
    Released,
}

/// Confined to one worker and one terminal for its entire lifetime.
pub(super) struct Anchors {
    namespace: Arc<()>,
    serial: u64,
    entries: Vec<Entry>,
}

impl Anchors {
    pub(super) fn set_selection(
        &self,
        terminal: &mut Terminal,
        start: &AnchorId,
        end: &AnchorId,
        rectangle: bool,
    ) -> Result<Result<(), super::selection::SelectionError>, Error> {
        use super::selection::{NativeSelection, SelectionError};
        let indices = match (self.index(start), self.index(end)) {
            (Ok(start), Ok(end)) => [start, end],
            (Err(error), _) | (_, Err(error)) => return Ok(Err(SelectionError::Anchor(error))),
        };
        let active = terminal.metadata()?.alternate_screen;
        for index in indices {
            let entry = &self.entries[index];
            if entry.alternate_screen != active {
                return Ok(Err(SelectionError::InactiveScreen));
            }
            if entry.tracked.point(terminal, Space::Screen)?.is_none() {
                return Ok(Err(SelectionError::Expired));
            }
        }
        let mut selection = NativeSelection {
            rectangle,
            ..Default::default()
        };
        for (index, out) in indices
            .into_iter()
            .zip([&mut selection.start, &mut selection.end])
        {
            // SAFETY: provenance, active-screen ownership and liveness checked
            // above. Snapshot and set run here without intervening mutation.
            check(unsafe {
                ghostty_tracked_grid_ref_snapshot(self.entries[index].tracked.raw.as_ptr(), out)
            })?;
        }
        // SAFETY: native set immediately copies and tracks both valid endpoints.
        check(unsafe {
            super::ghostty_terminal_set(terminal.raw.as_ptr(), 22, ptr::from_ref(&selection).cast())
        })?;
        Ok(Ok(()))
    }

    pub fn new() -> Self {
        Self {
            namespace: Arc::new(()),
            serial: 0,
            entries: Vec::new(),
        }
    }

    fn index(&self, id: &AnchorId) -> Result<usize, AnchorError> {
        if !Arc::ptr_eq(&self.namespace, &id.namespace) {
            return Err(AnchorError::Foreign);
        }
        self.entries
            .iter()
            .position(|entry| entry.serial == id.serial)
            .ok_or(AnchorError::Released)
    }

    /// Inner errors are local request rejections, not terminal-state failures.
    pub fn execute(
        &mut self,
        terminal: &mut Terminal,
        command: Command,
    ) -> Result<Result<Reply, AnchorError>, Error> {
        match command {
            Command::Create(point) => self.create(terminal, point),
            Command::Resolve(id, space) => {
                let index = match self.index(&id) {
                    Ok(index) => index,
                    Err(error) => return Ok(Err(error)),
                };
                let entry = &self.entries[index];
                Ok(Ok(Reply::Resolved(
                    entry
                        .tracked
                        .point(terminal, space)?
                        .map(|coordinate| Position {
                            coordinate,
                            alternate_screen: entry.alternate_screen,
                        }),
                )))
            }
            Command::Release(id) => {
                let index = match self.index(&id) {
                    Ok(index) => index,
                    Err(error) => return Ok(Err(error)),
                };
                self.entries.swap_remove(index);
                Ok(Ok(Reply::Released))
            }
        }
    }

    fn create(
        &mut self,
        terminal: &mut Terminal,
        point: Point,
    ) -> Result<Result<Reply, AnchorError>, Error> {
        if self.entries.len() == MAX_ANCHORS || self.serial == u64::MAX {
            return Ok(Err(AnchorError::Limit));
        }
        self.entries.try_reserve(1).map_err(|_| Error::Allocation)?;
        let alternate_screen = terminal.metadata()?.alternate_screen;
        let mut native = NativePoint {
            tag: point.space.tag(),
            value: PointValue { padding: [0; 2] },
        };
        native.value.coordinate = point.coordinate;
        let mut raw = ptr::null_mut();
        // SAFETY: the live terminal and new tracked handle stay on this worker.
        // The by-value tagged point matches the C probe's pinned layout/values.
        let status =
            unsafe { ghostty_terminal_grid_ref_track(terminal.raw.as_ptr(), native, &mut raw) };
        if status == -2 {
            return Ok(Err(AnchorError::InvalidPoint));
        }
        check(status)?;
        let tracked = Tracked {
            raw: NonNull::new(raw).ok_or(Error::NullHandle)?,
            _thread_confined: PhantomData,
        };
        // The pinned native pin lookup can accept history coordinates that are
        // actually in the active area. Require a native round trip in the exact
        // requested space, rather than inventing history bounds or clamping.
        if tracked.point(terminal, point.space)? != Some(point.coordinate) {
            return Ok(Err(AnchorError::InvalidPoint));
        }
        self.serial += 1;
        self.entries.push(Entry {
            serial: self.serial,
            alternate_screen,
            tracked,
        });
        Ok(Ok(Reply::Created(AnchorId {
            namespace: self.namespace.clone(),
            serial: self.serial,
        })))
    }
}

#[cfg(test)]
#[path = "anchor_tests.rs"]
mod tests;
