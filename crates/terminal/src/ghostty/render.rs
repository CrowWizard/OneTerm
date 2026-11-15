//! Owned viewport projection. No native iterator, pointer, or grid reference is
//! published. This does not yet implement the application's damage/graphics API.

use std::ffi::{c_int, c_void};
use std::marker::PhantomData;
use std::ptr::{self, NonNull};
use std::rc::Rc;

use super::{Error, Terminal, check};

const MAX_CELLS: usize = 1_048_576;
pub(crate) const MAX_TEXT_BYTES: usize = 16 * 1024 * 1024;
const OUT_OF_SPACE: c_int = -3;
const NO_VALUE: c_int = -4;

type Getter = unsafe extern "C" fn(*mut c_void, c_int, *mut c_void) -> c_int;
type NewHandle = unsafe extern "C" fn(*const c_void, *mut *mut c_void) -> c_int;
type FreeHandle = unsafe extern "C" fn(*mut c_void);

unsafe extern "C" {
    fn ghostty_render_state_new(allocator: *const c_void, out: *mut *mut c_void) -> c_int;
    fn ghostty_render_state_free(state: *mut c_void);
    fn ghostty_render_state_update(state: *mut c_void, terminal: *mut c_void) -> c_int;
    fn ghostty_render_state_get(state: *mut c_void, key: c_int, out: *mut c_void) -> c_int;
    fn ghostty_render_state_row_iterator_new(
        allocator: *const c_void,
        out: *mut *mut c_void,
    ) -> c_int;
    fn ghostty_render_state_row_iterator_free(rows: *mut c_void);
    fn ghostty_render_state_row_iterator_next(rows: *mut c_void) -> bool;
    fn ghostty_render_state_row_get(rows: *mut c_void, key: c_int, out: *mut c_void) -> c_int;
    fn ghostty_render_state_row_cells_new(allocator: *const c_void, out: *mut *mut c_void)
    -> c_int;
    fn ghostty_render_state_row_cells_free(cells: *mut c_void);
    fn ghostty_render_state_row_cells_next(cells: *mut c_void) -> bool;
    fn ghostty_render_state_row_cells_get(
        cells: *mut c_void,
        key: c_int,
        out: *mut c_void,
    ) -> c_int;
    fn ghostty_cell_get(cell: u64, key: c_int, out: *mut c_void) -> c_int;
    fn ghostty_row_get(row: u64, key: c_int, out: *mut c_void) -> c_int;
}

/// RGB bytes with the same fixed layout as GhosttyColorRgb.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Style color identity, before palette/default resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Default,
    Palette(u8),
    Rgb(Rgb),
}

#[derive(Default)]
#[repr(C)]
struct NativeStyle {
    foreground: [u8; 4],
    background: [u8; 4],
    underline: [u8; 4],
    flags: u16,
    reserved: u16,
}

/// All GhosttyStyle flags are retained, including underline style bits 8..10.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub foreground: Color,
    pub background: Color,
    pub underline: Color,
    pub flags: u16,
}

impl TryFrom<NativeStyle> for Style {
    type Error = Error;

    fn try_from(raw: NativeStyle) -> Result<Self, Error> {
        fn color(bytes: [u8; 4]) -> Result<Color, Error> {
            match bytes {
                [0, ..] => Ok(Color::Default),
                [1, index, ..] => Ok(Color::Palette(index)),
                [2, r, g, b] => Ok(Color::Rgb(Rgb { r, g, b })),
                _ => Err(Error::InvalidSnapshot("unknown style color tag")),
            }
        }
        Ok(Self {
            foreground: color(raw.foreground)?,
            background: color(raw.background)?,
            underline: color(raw.underline)?,
            flags: raw.flags,
        })
    }
}

/// Opaque equality-only identity, not an absolute or ordered application RowId.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct RowIdentity {
    bits: [u64; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    Narrow,
    Wide,
    SpacerTail,
    SpacerHead,
}

/// One owned cell; an empty grapheme is not converted into a printable space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub width: Width,
    pub style: Style,
    /// Resolved cell colors also cover packed background-only erase cells.
    pub foreground: Option<Rgb>,
    pub background: Option<Rgb>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub identity: RowIdentity,
    pub wrapped: bool,
    pub wrap_continuation: bool,
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Bar,
    Block,
    Underline,
    HollowBlock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub position: Option<(u16, u16)>,
    pub shape: CursorShape,
    pub visible: bool,
    pub blinking: bool,
}

/// Full owned viewport. It can outlive both native render state and terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub metadata: super::state::Metadata,
    pub cols: u16,
    pub rows: Vec<Row>,
    pub cursor: Cursor,
    pub foreground: Rgb,
    pub background: Rgb,
    pub cursor_color: Option<Rgb>,
    pub palette: [Rgb; 256],
}

// Keys are private typed constants, not integer parameters supplied by callers.
// Their discriminants and output layouts are checked against C in probe.c.
struct Field<T>(c_int, PhantomData<T>);

const fn field<T>(key: c_int) -> Field<T> {
    Field(key, PhantomData)
}

const COLS: Field<u16> = field(1);
const ROWS: Field<u16> = field(2);
const BACKGROUND: Field<Rgb> = field(5);
const FOREGROUND: Field<Rgb> = field(6);
const CURSOR_COLOR: Field<Rgb> = field(7);
const PALETTE: Field<[Rgb; 256]> = field(9);
const CURSOR_SHAPE: Field<c_int> = field(10);
const CURSOR_VISIBLE: Field<bool> = field(11);
const CURSOR_BLINKING: Field<bool> = field(12);
const CURSOR_IN_VIEW: Field<bool> = field(14);
const CURSOR_X: Field<u16> = field(15);
const CURSOR_Y: Field<u16> = field(16);
const ROW_RAW: Field<u64> = field(2);
const ROW_ID: Field<RowIdentity> = field(7);
const CELL_RAW: Field<u64> = field(1);
const CELL_STYLE: Field<NativeStyle> = field(2);
const CELL_BG: Field<Rgb> = field(5);
const CELL_FG: Field<Rgb> = field(6);
const CELL_SELECTED: Field<bool> = field(7);

struct Handle {
    raw: NonNull<c_void>,
    free: FreeHandle,
    get: Getter,
}

impl Handle {
    fn new(new: NewHandle, free: FreeHandle, get: Getter) -> Result<Self, Error> {
        let mut raw = ptr::null_mut();
        // SAFETY: the caller pairs matching new/free/get functions from the
        // pinned API. The default allocator and output slot are valid.
        check(unsafe { new(ptr::null(), &mut raw) })?;
        Ok(Self {
            raw: NonNull::new(raw).ok_or(Error::NullHandle)?,
            free,
            get,
        })
    }

    fn read<T>(&self, field: Field<T>, mut value: T) -> Result<T, Error> {
        // SAFETY: private Field constants specify each key's actual output
        // type. The iterator is positioned and its render state remains live.
        check(unsafe { (self.get)(self.raw.as_ptr(), field.0, ptr::from_mut(&mut value).cast()) })?;
        Ok(value)
    }

    fn optional<T>(&self, field: Field<T>, mut value: T) -> Result<Option<T>, Error> {
        // SAFETY: same key/type and lifetime invariants as read. NO_VALUE does
        // not initialize output, so that branch never publishes the value.
        match unsafe { (self.get)(self.raw.as_ptr(), field.0, ptr::from_mut(&mut value).cast()) } {
            NO_VALUE => Ok(None),
            result => {
                check(result)?;
                Ok(Some(value))
            }
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: non-Clone owner, matched destructor, exactly one free.
        unsafe { (self.free)(self.raw.as_ptr()) };
    }
}

#[repr(C)]
struct Buffer {
    ptr: *mut u8,
    cap: usize,
    len: usize,
}

/// Snapshot owner for one consumer. Rebuilds native state for each full frame.
pub struct RenderBuffer {
    // Rust drops fields in this order: borrowed cell/row views before state.
    cells: Handle,
    rows: Handle,
    state: Handle,
    _thread_confined: PhantomData<Rc<()>>,
}

impl RenderBuffer {
    pub fn new() -> Result<Self, Error> {
        let state = Handle::new(
            ghostty_render_state_new,
            ghostty_render_state_free,
            ghostty_render_state_get,
        )?;
        let rows = Handle::new(
            ghostty_render_state_row_iterator_new,
            ghostty_render_state_row_iterator_free,
            ghostty_render_state_row_get,
        )?;
        let cells = Handle::new(
            ghostty_render_state_row_cells_new,
            ghostty_render_state_row_cells_free,
            ghostty_render_state_row_cells_get,
        )?;
        Ok(Self {
            cells,
            rows,
            state,
            _thread_confined: PhantomData,
        })
    }

    pub fn capture(&mut self, terminal: &mut Terminal) -> Result<Frame, Error> {
        // Native updates consume terminal-global row dirtiness. Rebuild all
        // handles so another consumer cannot leave this one stale. This
        // correctness-first staging path intentionally allocates per frame.
        let mut fresh = Self::new()?;
        let frame = fresh.capture_inner(terminal)?;
        *self = fresh;
        Ok(frame)
    }

    fn capture_inner(&mut self, terminal: &mut Terminal) -> Result<Frame, Error> {
        let mut cols = 0_u16;
        let mut rows = 0_u16;
        // SAFETY: terminal dimensions are uint16_t outputs. Check the budget
        // before native snapshot allocation; terminal is exclusively borrowed.
        unsafe {
            check(super::ghostty_terminal_get(
                terminal.raw.as_ptr(),
                1,
                ptr::from_mut(&mut cols).cast(),
            ))?;
            check(super::ghostty_terminal_get(
                terminal.raw.as_ptr(),
                2,
                ptr::from_mut(&mut rows).cast(),
            ))?;
        }
        check_budget(cols, rows)?;
        let mut row_iterator = self.rows.raw.as_ptr();
        // SAFETY: all owners are live and thread-confined. No iterator view is
        // used until this update and the population calls below have finished.
        unsafe {
            check(ghostty_render_state_update(
                self.state.raw.as_ptr(),
                terminal.raw.as_ptr(),
            ))?;
            check(ghostty_render_state_get(
                self.state.raw.as_ptr(),
                4,
                ptr::from_mut(&mut row_iterator).cast(),
            ))?;
        }
        if self.state.read(COLS, 0)? != cols || self.state.read(ROWS, 0)? != rows {
            return Err(Error::InvalidSnapshot(
                "viewport dimensions changed during capture",
            ));
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(usize::from(rows))
            .map_err(|_| Error::Allocation)?;
        let mut text_bytes = 0;
        // SAFETY: rows was populated from state above and state is not updated
        // again until this loop and all cell reads finish.
        while unsafe { ghostty_render_state_row_iterator_next(self.rows.raw.as_ptr()) } {
            if output.len() >= usize::from(rows) {
                return Err(Error::InvalidSnapshot("too many viewport rows"));
            }
            let identity = self.rows.read(ROW_ID, RowIdentity::default())?;
            let raw = self.rows.read(ROW_RAW, 0)?;
            let mut wrapped = false;
            let mut wrap_continuation = false;
            let mut cell_iterator = self.cells.raw.as_ptr();
            // SAFETY: row keys 1/2 return bool; raw is the API's opaque u64.
            unsafe {
                check(ghostty_row_get(raw, 1, ptr::from_mut(&mut wrapped).cast()))?;
                check(ghostty_row_get(
                    raw,
                    2,
                    ptr::from_mut(&mut wrap_continuation).cast(),
                ))?;
                check(ghostty_render_state_row_get(
                    self.rows.raw.as_ptr(),
                    3,
                    ptr::from_mut(&mut cell_iterator).cast(),
                ))?;
            }
            let mut cells = Vec::new();
            cells
                .try_reserve_exact(usize::from(cols))
                .map_err(|_| Error::Allocation)?;
            // SAFETY: cells is populated from the current row and neither row
            // iterator nor underlying state advances during the inner loop.
            while unsafe { ghostty_render_state_row_cells_next(self.cells.raw.as_ptr()) } {
                if cells.len() >= usize::from(cols) {
                    return Err(Error::InvalidSnapshot("too many row cells"));
                }
                cells.push(self.cell(&mut text_bytes)?);
            }
            if cells.len() != usize::from(cols) {
                return Err(Error::InvalidSnapshot("incomplete row"));
            }
            output.push(Row {
                identity,
                wrapped,
                wrap_continuation,
                cells,
            });
        }
        if output.len() != usize::from(rows) {
            return Err(Error::InvalidSnapshot("incomplete viewport"));
        }
        let metadata = terminal.metadata()?;
        if metadata.scrollbar.len != u64::from(rows) {
            return Err(Error::InvalidSnapshot(
                "scrollbar height differs from frame",
            ));
        }
        Ok(Frame {
            metadata,
            cols,
            rows: output,
            cursor: self.cursor()?,
            foreground: self.state.read(FOREGROUND, Rgb::default())?,
            background: self.state.read(BACKGROUND, Rgb::default())?,
            cursor_color: self.state.optional(CURSOR_COLOR, Rgb::default())?,
            palette: self.state.read(PALETTE, [Rgb::default(); 256])?,
        })
    }

    fn cursor(&self) -> Result<Cursor, Error> {
        let position = if self.state.read(CURSOR_IN_VIEW, false)? {
            Some((self.state.read(CURSOR_X, 0)?, self.state.read(CURSOR_Y, 0)?))
        } else {
            None
        };
        let shape = match self.state.read(CURSOR_SHAPE, 0)? {
            0 => CursorShape::Bar,
            1 => CursorShape::Block,
            2 => CursorShape::Underline,
            3 => CursorShape::HollowBlock,
            _ => return Err(Error::InvalidSnapshot("unknown cursor shape")),
        };
        Ok(Cursor {
            position,
            shape,
            visible: self.state.read(CURSOR_VISIBLE, false)?,
            blinking: self.state.read(CURSOR_BLINKING, false)?,
        })
    }

    fn cell(&self, text_bytes: &mut usize) -> Result<Cell, Error> {
        let raw = self.cells.read(CELL_RAW, 0)?;
        let mut wide: c_int = 0;
        // SAFETY: key 3 writes GhosttyCellWide, an int-sized enum, from the
        // opaque raw cell returned by this positioned iterator.
        check(unsafe { ghostty_cell_get(raw, 3, ptr::from_mut(&mut wide).cast()) })?;
        let width = match wide {
            0 => Width::Narrow,
            1 => Width::Wide,
            2 => Width::SpacerTail,
            3 => Width::SpacerHead,
            _ => return Err(Error::InvalidSnapshot("unknown cell width")),
        };
        Ok(Cell {
            text: self.text(text_bytes)?,
            width,
            style: self
                .cells
                .read(CELL_STYLE, NativeStyle::default())?
                .try_into()?,
            foreground: self.cells.optional(CELL_FG, Rgb::default())?,
            background: self.cells.optional(CELL_BG, Rgb::default())?,
            selected: self.cells.read(CELL_SELECTED, false)?,
        })
    }

    fn text(&self, used: &mut usize) -> Result<String, Error> {
        let mut buffer = Buffer {
            ptr: ptr::null_mut(),
            cap: 0,
            len: 0,
        };
        // SAFETY: key 9 takes GhosttyBuffer; a null pointer/zero capacity is
        // the documented length query, and the native cell remains unchanged.
        let status = unsafe {
            ghostty_render_state_row_cells_get(
                self.cells.raw.as_ptr(),
                9,
                ptr::from_mut(&mut buffer).cast(),
            )
        };
        if status != OUT_OF_SPACE {
            check(status)?;
        }
        let len = buffer.len;
        let next = used
            .checked_add(len)
            .filter(|n| *n <= MAX_TEXT_BYTES)
            .ok_or(Error::SnapshotLimit)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(len)
            .map_err(|_| Error::Allocation)?;
        bytes.resize(len, 0);
        if len != 0 {
            buffer.ptr = bytes.as_mut_ptr();
            buffer.cap = len;
            // SAFETY: a fully initialized owned buffer has the requested
            // capacity. Cell/state are not changed between query and copy.
            check(unsafe {
                ghostty_render_state_row_cells_get(
                    self.cells.raw.as_ptr(),
                    9,
                    ptr::from_mut(&mut buffer).cast(),
                )
            })?;
            if buffer.len != len {
                return Err(Error::InvalidSnapshot("grapheme length changed"));
            }
        }
        *used = next;
        String::from_utf8(bytes).map_err(|_| Error::InvalidSnapshot("grapheme is not UTF-8"))
    }
}

/// Validate viewport dimensions before bounded backend resize admission.
/// This checks the capture cell budget without entering native code.
pub fn check_budget(cols: u16, rows: u16) -> Result<(), Error> {
    if cols == 0 || rows == 0 {
        return Err(Error::InvalidSnapshot("zero viewport dimensions"));
    }
    if usize::from(cols) * usize::from(rows) > MAX_CELLS {
        return Err(Error::SnapshotLimit);
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn test_text_budget(
    render: &mut RenderBuffer,
    terminal: &mut Terminal,
) -> Result<String, Error> {
    render.capture(terminal)?;
    let mut rows = render.rows.raw.as_ptr();
    let mut cells = render.cells.raw.as_ptr();
    // SAFETY: live matching handles; position both iterators at the first cell.
    unsafe {
        check(ghostty_render_state_get(
            render.state.raw.as_ptr(),
            4,
            ptr::from_mut(&mut rows).cast(),
        ))?;
        assert!(ghostty_render_state_row_iterator_next(rows));
        check(ghostty_render_state_row_get(
            rows,
            3,
            ptr::from_mut(&mut cells).cast(),
        ))?;
        assert!(ghostty_render_state_row_cells_next(cells));
    }
    let mut used = MAX_TEXT_BYTES;
    render.text(&mut used)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_cell_budget_is_checked_before_snapshot_allocation() {
        assert_eq!(check_budget(1024, 1024), Ok(()));
        assert_eq!(check_budget(1025, 1024), Err(Error::SnapshotLimit));
        assert!(matches!(check_budget(0, 1), Err(Error::InvalidSnapshot(_))));
        assert_eq!(check_budget(u16::MAX, u16::MAX), Err(Error::SnapshotLimit));
    }

    #[test]
    fn rust_layouts_match_the_c_probe() {
        use std::mem::{align_of, offset_of, size_of};
        assert_eq!((size_of::<Rgb>(), align_of::<Rgb>()), (3, 1));
        assert_eq!(
            (offset_of!(Rgb, r), offset_of!(Rgb, g), offset_of!(Rgb, b)),
            (0, 1, 2)
        );
        assert_eq!(
            (size_of::<NativeStyle>(), align_of::<NativeStyle>()),
            (16, 2)
        );
        assert_eq!(offset_of!(NativeStyle, foreground), 0);
        assert_eq!(offset_of!(NativeStyle, background), 4);
        assert_eq!(offset_of!(NativeStyle, underline), 8);
        assert_eq!(offset_of!(NativeStyle, flags), 12);
        assert_eq!(offset_of!(NativeStyle, reserved), 14);
        assert_eq!(
            (size_of::<RowIdentity>(), align_of::<RowIdentity>()),
            (16, 8)
        );
        assert_eq!((size_of::<Buffer>(), align_of::<Buffer>()), (24, 8));
        assert_eq!(offset_of!(Buffer, ptr), 0);
        assert_eq!(offset_of!(Buffer, cap), 8);
        assert_eq!(offset_of!(Buffer, len), 16);
    }
}
