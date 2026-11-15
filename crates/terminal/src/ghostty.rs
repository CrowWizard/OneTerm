//! Staged native ownership boundary for the pinned libghostty-vt library.
//!
//! Enabled explicitly by `ghostty-native`; neither backend uses it yet. The
//! owner is thread-confined until native thread migration is audited. It never
//! keeps an oneterm-vt terminal alongside its native state.

use std::ffi::{c_int, c_void};
use std::fmt;
use std::marker::PhantomData;
use std::ptr::{self, NonNull};
use std::rc::Rc;

#[path = "ghostty/render.rs"]
pub mod render;

#[path = "ghostty/owner.rs"]
pub mod owner;

#[path = "ghostty/events.rs"]
pub mod events;

#[path = "ghostty/clipboard.rs"]
mod clipboard;

#[path = "ghostty/state.rs"]
pub mod state;

#[path = "ghostty/input.rs"]
pub mod input;

#[path = "ghostty/wheel.rs"]
pub mod wheel;

#[path = "ghostty/anchors.rs"]
pub mod anchors;

#[path = "ghostty/selection.rs"]
pub mod selection;

#[path = "ghostty/search.rs"]
pub mod search;

#[path = "ghostty/colors.rs"]
pub mod colors;

#[cfg(test)]
#[path = "ghostty/render_tests.rs"]
mod render_tests;

const REPLY_CAPACITY: usize = 4096;
// Checked against the pinned C headers by probe.c's static assertions.
const USERDATA: c_int = 1;
const WRITE_PTY: c_int = 2;
const CURSOR_X: c_int = 3;
const CURSOR_Y: c_int = 4;
const PROCESSING_ERROR: c_int = 33;

unsafe extern "C" {
    fn ghostty_terminal_new(
        allocator: *const c_void,
        terminal: *mut *mut c_void,
        cols: u16,
        rows: u16,
    ) -> c_int;
    fn ghostty_terminal_free(terminal: *mut c_void);
    fn ghostty_terminal_set(terminal: *mut c_void, option: c_int, value: *const c_void) -> c_int;
    fn ghostty_terminal_get(terminal: *mut c_void, data: c_int, value: *mut c_void) -> c_int;
    fn ghostty_terminal_resize(
        terminal: *mut c_void,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> c_int;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Terminal dimensions must be nonzero.
    InvalidSize,
    /// The upstream ABI returned this non-success result.
    Native(c_int),
    /// Upstream returned success without a terminal handle.
    NullHandle,
    /// Replies exceeded the fixed capture bound; the session must stop.
    ReplyOverflow,
    /// Callback events exceeded their fixed count or text budget.
    EventOverflow,
    /// Native callback data could not be queried or decoded safely.
    InvalidEvent,
    /// Upstream reports an error while processing terminal output.
    Processing,
    /// The viewport or its text exceeds the bounded owned-frame budget.
    SnapshotLimit,
    /// Memory for an owned frame could not be reserved.
    Allocation,
    /// Native render data did not match the pinned ABI's invariants.
    InvalidSnapshot(&'static str),
    /// A local input request is malformed; it was rejected before encoding.
    InvalidInput(&'static str),
    /// Input text or encoded output exceeds the explicit input budget.
    InputLimit,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize => f.write_str("Ghostty terminal dimensions must be nonzero"),
            Self::Native(code) => write!(f, "Ghostty operation failed with native result {code}"),
            Self::NullHandle => f.write_str("Ghostty returned success without a terminal handle"),
            Self::ReplyOverflow => f.write_str("Ghostty reply capture exceeded its byte limit"),
            Self::EventOverflow => f.write_str("Ghostty callback events exceeded their budget"),
            Self::InvalidEvent => f.write_str("Ghostty callback returned invalid event data"),
            Self::Processing => f.write_str("Ghostty failed to process terminal output"),
            Self::SnapshotLimit => f.write_str("Ghostty snapshot exceeded the frame budget"),
            Self::Allocation => f.write_str("Could not reserve memory for Ghostty owned data"),
            Self::InvalidSnapshot(reason) => write!(f, "Invalid Ghostty snapshot: {reason}"),
            Self::InvalidInput(reason) => write!(f, "Invalid Ghostty input: {reason}"),
            Self::InputLimit => f.write_str("Ghostty input exceeded its byte budget"),
        }
    }
}

impl std::error::Error for Error {}

fn check(status: c_int) -> Result<(), Error> {
    match status {
        0 => Ok(()),
        other => Err(Error::Native(other)),
    }
}

struct Replies {
    bytes: [u8; REPLY_CAPACITY],
    len: usize,
    overflow: bool,
    events: events::Capture,
}

unsafe extern "C" fn write_reply(
    _terminal: *mut c_void,
    userdata: *mut c_void,
    data: *const u8,
    len: usize,
) {
    // SAFETY: the owner registers a stable boxed context before any writes and
    // frees the terminal before the box. Calls are synchronous and exclusive.
    let replies = unsafe { &mut *userdata.cast::<Replies>() };
    if replies.overflow || len > REPLY_CAPACITY - replies.len {
        replies.overflow = true;
        return;
    }
    if len != 0 {
        // SAFETY: Ghostty lends len readable bytes for this callback. The fixed
        // destination has enough room (checked above) and cannot overlap them.
        unsafe {
            ptr::copy_nonoverlapping(data, replies.bytes.as_mut_ptr().add(replies.len), len);
        }
        replies.len += len;
    }
    // No allocation, indexing, locks, user code, or panicking work crosses C.
}

pub struct Terminal {
    raw: NonNull<c_void>,
    replies: Box<Replies>,
    clipboard: clipboard::Observer,
    // Native thread migration is not yet audited: deliberately !Send + !Sync.
    _thread_confined: PhantomData<Rc<()>>,
}

impl Terminal {
    /// Configure physical history lines before feeding output. Positive limits
    /// are page-granular upstream; zero disables and erases retained history.
    pub fn set_scrollback_limit(&mut self, lines: usize) -> Result<(), Error> {
        let bytes = if lines == 0 { 0 } else { usize::MAX };
        // SAFETY: pinned options 28/29 borrow size_t pointers synchronously.
        // The terminal is exclusively owned on its confined worker.
        check(unsafe {
            ghostty_terminal_set(self.raw.as_ptr(), 28, ptr::from_ref(&bytes).cast())
        })?;
        // SAFETY: option 29 reads a size_t; lines lives through the call.
        check(unsafe { ghostty_terminal_set(self.raw.as_ptr(), 29, ptr::from_ref(&lines).cast()) })
    }

    /// Create a terminal with no process, transport, or UI side effects.
    pub fn new(cols: u16, rows: u16) -> Result<Self, Error> {
        if cols == 0 || rows == 0 {
            return Err(Error::InvalidSize);
        }
        let mut raw = ptr::null_mut();
        // SAFETY: default allocator, valid output slot and nonzero dimensions.
        check(unsafe { ghostty_terminal_new(ptr::null(), &mut raw, cols, rows) })?;
        let mut terminal = Self {
            raw: NonNull::new(raw).ok_or(Error::NullHandle)?,
            replies: Box::new(Replies {
                bytes: [0; REPLY_CAPACITY],
                len: 0,
                overflow: false,
                events: events::Capture::new(),
            }),
            _thread_confined: PhantomData,
            clipboard: clipboard::Observer::default(),
        };
        let context = ptr::from_mut(terminal.replies.as_mut()).cast();
        // SAFETY: options take pointer values directly, not pointer-to-pointer.
        // The context does not move with its owning Box. On setup failure Drop
        // frees the native handle while the context is still alive.
        check(unsafe { ghostty_terminal_set(terminal.raw.as_ptr(), USERDATA, context) })?;
        // SAFETY: callback has the exact upstream C ABI and remains executable
        // throughout the terminal's lifetime. No callback calls back into it.
        check(unsafe {
            ghostty_terminal_set(
                terminal.raw.as_ptr(),
                WRITE_PTY,
                write_reply as *const () as *const c_void,
            )
        })?;
        events::install(&mut terminal)?;
        Ok(terminal)
    }

    /// Feed incremental output. Overflow is sticky, never silent truncation.
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.replies.events.check()?;
        if self.replies.overflow {
            return Err(Error::ReplyOverflow);
        }
        // SAFETY: unique access to a live terminal. Input is valid for the
        // synchronous call; the registered callback only touches its context.
        self.write_observed(bytes)?;
        if self.replies.overflow {
            return Err(Error::ReplyOverflow);
        }
        self.replies.events.check()?;
        let mut processing_error = false;
        // SAFETY: this key writes a C bool, represented by Rust bool. The
        // output slot and exclusive native terminal remain live for the call.
        check(unsafe {
            ghostty_terminal_get(
                self.raw.as_ptr(),
                PROCESSING_ERROR,
                ptr::from_mut(&mut processing_error).cast(),
            )
        })?;
        if processing_error {
            return Err(Error::Processing);
        }
        Ok(())
    }

    /// Drain owned reply bytes for delivery to the originating transport.
    pub fn take_replies(&mut self) -> Result<Vec<u8>, Error> {
        self.replies.events.check()?;
        if self.replies.overflow {
            return Err(Error::ReplyOverflow);
        }
        let owned = self.replies.bytes[..self.replies.len].to_vec();
        self.replies.len = 0;
        Ok(owned)
    }

    /// Copy callback payloads into owned events; native pointers never escape.
    pub fn take_events(&mut self) -> Result<Vec<events::Event>, Error> {
        self.replies.events.drain()
    }

    /// Read the zero-based active-screen cursor column and row.
    pub fn cursor(&mut self) -> Result<(u16, u16), Error> {
        let mut x = 0_u16;
        let mut y = 0_u16;
        // SAFETY: each key's output is a uint16_t in the pinned header; both
        // outputs live through the call and the handle is exclusively owned.
        unsafe {
            check(ghostty_terminal_get(
                self.raw.as_ptr(),
                CURSOR_X,
                ptr::from_mut(&mut x).cast(),
            ))?;
            check(ghostty_terminal_get(
                self.raw.as_ptr(),
                CURSOR_Y,
                ptr::from_mut(&mut y).cast(),
            ))?;
        }
        Ok((x, y))
    }

    /// Resize the grid and set the renderer's cell dimensions in pixels.
    pub fn resize(
        &mut self,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result<(), Error> {
        self.replies.events.check()?;
        if self.replies.overflow {
            return Err(Error::ReplyOverflow);
        }
        if cols == 0 || rows == 0 {
            return Err(Error::InvalidSize);
        }
        // SAFETY: exclusive live handle and nonzero dimensions. Reply context
        // remains live if resize synchronously emits an in-band size report.
        check(unsafe {
            ghostty_terminal_resize(self.raw.as_ptr(), cols, rows, cell_width_px, cell_height_px)
        })?;
        if self.replies.overflow {
            return Err(Error::ReplyOverflow);
        }
        self.replies.events.check()?;
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // SAFETY: this non-Clone owner frees its handle exactly once. Field
        // destruction follows this call, so callback userdata is still live.
        unsafe { ghostty_terminal_free(self.raw.as_ptr()) };
    }
}

#[test]
fn split_input_replies_and_moves_keep_context_live() {
    let stream = b"\x1b[31mA\xe4\xb8\xad\x1b[0m\x1b[6n\x1b[3;5Hxy\x1b[6n";
    for split in 0..=stream.len() {
        let mut original = Terminal::new(80, 24).unwrap();
        original.write(&stream[..split]).unwrap();
        let mut moved = Box::new(original);
        moved.write(&stream[split..]).unwrap();
        assert_eq!(moved.cursor().unwrap(), (6, 2));
        let replies = moved.take_replies().unwrap();
        assert_eq!(replies, b"\x1b[1;4R\x1b[3;7R");
        moved.write(b"\x1b[6n").unwrap();
        assert_eq!(moved.take_replies().unwrap(), b"\x1b[3;7R");
        assert_eq!(replies, b"\x1b[1;4R\x1b[3;7R");
        moved.resize(100, 30, 9, 18).unwrap();
    }
}

#[test]
fn reply_overflow_is_explicit_and_sticky() {
    let mut terminal = Terminal::new(80, 24).unwrap();
    let flood = b"\x1b[6n".repeat(REPLY_CAPACITY);
    assert_eq!(terminal.write(&flood), Err(Error::ReplyOverflow));
    assert_eq!(terminal.take_replies(), Err(Error::ReplyOverflow));
    assert_eq!(terminal.write(b"more"), Err(Error::ReplyOverflow));
}

#[test]
fn invalid_resize_preserves_the_live_terminal() {
    assert!(matches!(Terminal::new(0, 24), Err(Error::InvalidSize)));
    let mut terminal = Terminal::new(80, 24).unwrap();
    assert_eq!(terminal.resize(80, 0, 9, 18), Err(Error::InvalidSize));
    terminal.write(b"hello\x1b[6n").unwrap();
    assert_eq!(terminal.take_replies().unwrap(), b"\x1b[1;6R");
}

#[test]
fn repeated_create_write_drop_has_no_shared_callback_state() {
    for _ in 0..128 {
        let mut terminal = Terminal::new(80, 24).unwrap();
        terminal.write(b"x\x1b[6n").unwrap();
        assert_eq!(terminal.take_replies().unwrap(), b"\x1b[1;2R");
    }
}
