//! Owned native metadata and viewport commands. No application RowId is inferred.

use std::ffi::{c_int, c_void};
use std::ptr;

use super::{Error, Terminal, check, ghostty_terminal_get};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Scrollbar {
    pub total: u64,
    pub offset: u64,
    pub len: u64,
}

/// Raw supported modes retained without choosing application input fallbacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub alternate_screen: bool,
    pub kitty_keyboard_flags: u8,
    pub scrollbar: Scrollbar,
    pub modes: Modes,
}

impl Metadata {
    pub(super) fn global_damage_changed(&self, after: &Self) -> bool {
        // Cursor visibility already damages its old/new rows through Cursor.
        // Do not turn that existing narrow update into a full repaint.
        let mut before_modes = self.modes.clone();
        before_modes.cursor_visible = after.modes.cursor_visible;
        self.alternate_screen != after.alternate_screen
            || self.kitty_keyboard_flags != after.kitty_keyboard_flags
            || self.scrollbar != after.scrollbar
            || before_modes != after.modes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modes {
    pub cursor_keys: bool,
    pub keypad_keys: bool,
    pub bracketed_paste: bool,
    pub cursor_visible: bool,
    pub insert: bool,
    pub reverse_colors: bool,
    pub alternate_scroll: bool,
    pub synchronized_output: bool,
    pub focus_events: bool,
    pub mouse_x10: bool,
    pub mouse_normal: bool,
    pub mouse_button: bool,
    pub mouse_any: bool,
    pub mouse_utf8: bool,
    pub mouse_sgr: bool,
    pub mouse_urxvt: bool,
    pub mouse_sgr_pixels: bool,
}

#[repr(C)]
struct ModeConfig {
    mode: u16,
    value: bool,
}

/// Native scroll direction: negative moves toward older history.
#[derive(Debug, Clone, Copy)]
pub enum Scroll {
    Top,
    Bottom,
    Delta(i32),
    Row(u64),
}

#[repr(C)]
union ScrollValue {
    delta: isize,
    row: usize,
    padding: [u64; 2],
}

#[repr(C)]
struct NativeScroll {
    tag: c_int,
    value: ScrollValue,
}

unsafe extern "C" {
    fn ghostty_terminal_scroll_viewport(terminal: *mut c_void, scroll: NativeScroll) -> c_int;
}

impl Terminal {
    fn read_state<T>(&self, key: c_int, mut value: T) -> Result<T, Error> {
        // SAFETY: private callers match each pinned key to its C output type.
        check(unsafe {
            ghostty_terminal_get(self.raw.as_ptr(), key, ptr::from_mut(&mut value).cast())
        })?;
        Ok(value)
    }

    fn mode(&self, number: u16, ansi: bool) -> Result<bool, Error> {
        let config = ModeConfig {
            mode: number | (u16::from(ansi) << 15),
            value: false,
        };
        Ok(self.read_state(37, config)?.value)
    }

    pub fn metadata(&self) -> Result<Metadata, Error> {
        let alternate_screen = match self.read_state(6, 0 as c_int)? {
            0 => false,
            1 => true,
            _ => return Err(Error::InvalidSnapshot("unknown active screen")),
        };
        let scrollbar = self.read_state(9, Scrollbar::default())?;
        if scrollbar.len > scrollbar.total || scrollbar.offset > scrollbar.total - scrollbar.len {
            return Err(Error::InvalidSnapshot("invalid native scrollbar range"));
        }
        Ok(Metadata {
            alternate_screen,
            kitty_keyboard_flags: self.read_state(8, 0u8)?,
            scrollbar,
            modes: Modes {
                cursor_keys: self.mode(1, false)?,
                keypad_keys: self.mode(66, false)?,
                bracketed_paste: self.mode(2004, false)?,
                cursor_visible: self.mode(25, false)?,
                insert: self.mode(4, true)?,
                reverse_colors: self.mode(5, false)?,
                alternate_scroll: self.mode(1007, false)?,
                synchronized_output: self.mode(2026, false)?,
                focus_events: self.mode(1004, false)?,
                mouse_x10: self.mode(9, false)?,
                mouse_normal: self.mode(1000, false)?,
                mouse_button: self.mode(1002, false)?,
                mouse_any: self.mode(1003, false)?,
                mouse_utf8: self.mode(1005, false)?,
                mouse_sgr: self.mode(1006, false)?,
                mouse_urxvt: self.mode(1015, false)?,
                mouse_sgr_pixels: self.mode(1016, false)?,
            },
        })
    }

    pub fn scroll(&mut self, scroll: Scroll) -> Result<(), Error> {
        let mut native = NativeScroll {
            tag: 0,
            value: ScrollValue { padding: [0; 2] },
        };
        match scroll {
            Scroll::Top => {}
            Scroll::Bottom => native.tag = 1,
            Scroll::Delta(delta) => {
                native.tag = 2;
                native.value.delta = delta as isize;
            }
            Scroll::Row(row) => {
                native.tag = 3;
                native.value.row = usize::try_from(row).map_err(|_| Error::SnapshotLimit)?;
            }
        }
        // SAFETY: pinned by-value tagged union, initialized padding and active
        // member; exclusive native owner borrow, no callback pointer escapes.
        check(unsafe { ghostty_terminal_scroll_viewport(self.raw.as_ptr(), native) })
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
