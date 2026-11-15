//! Caller-owned native defaults, separate from program-controlled OSC overrides.

use std::ptr;

use super::render::Rgb;
use super::{Error, Terminal, check, ghostty_terminal_set};

/// Full palette intent. Never construct defaults from an effective native frame:
/// it may contain OSC overrides which must disappear when the program resets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultColors {
    pub foreground: Rgb,
    pub background: Rgb,
    /// None restores the native unset default, not an invented foreground value.
    pub cursor: Option<Rgb>,
    pub palette: [Rgb; 256],
}

impl Terminal {
    /// Update defaults without clearing OSC overrides or feeding parser bytes.
    /// A failed setter may follow earlier mutations: callers must fail-stop,
    /// retain last-good publication and never claim transactional rollback.
    pub fn set_default_colors(&mut self, colors: &DefaultColors) -> Result<(), Error> {
        let values = [
            (12, ptr::from_ref(&colors.foreground)),
            (13, ptr::from_ref(&colors.background)),
            (
                14,
                colors.cursor.as_ref().map_or(ptr::null(), ptr::from_ref),
            ),
        ];
        for (key, value) in values {
            // SAFETY: pinned keys 12-14 accept borrowed RGB pointers or null to
            // clear the default. Native code copies during this exclusive call.
            check(unsafe { ghostty_terminal_set(self.raw.as_ptr(), key, value.cast()) })?;
        }
        // SAFETY: pinned key 15 copies exactly 256 packed RGB entries. The
        // complete initialized array is live throughout this synchronous call.
        check(unsafe {
            ghostty_terminal_set(self.raw.as_ptr(), 15, colors.palette.as_ptr().cast())
        })
    }
}

#[cfg(test)]
#[path = "colors_tests.rs"]
mod tests;
