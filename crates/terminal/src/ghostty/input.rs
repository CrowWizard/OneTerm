//! State-derived input encoding. All native handles and UTF-8 borrows stay on
//! the terminal owner thread; only owned requests and bytes cross its boundary.

use std::ffi::{c_int, c_void};
use std::marker::PhantomData;
use std::ptr::{self, NonNull};
use std::rc::Rc;

use super::{Error, Terminal, check};

pub const MAX_KEY_TEXT_BYTES: usize = 4096;
pub const MAX_ENCODED_BYTES: usize = 64 * 1024;

/// Physical key code from the pinned Ghostty/W3C key table, not a Unicode value.
/// A platform adapter must supply the real code or UNIDENTIFIED, not guess the
/// physical location from layout text. The pinned table is contiguous 0..=175.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyCode(u16);

impl KeyCode {
    pub const UNIDENTIFIED: Self = Self(0);
    pub const BACKSPACE: Self = Self(53);
    pub const ENTER: Self = Self(58);
    pub const TAB: Self = Self(64);
    pub const ARROW_DOWN: Self = Self(75);
    pub const ARROW_LEFT: Self = Self(76);
    pub const ARROW_RIGHT: Self = Self(77);
    pub const ARROW_UP: Self = Self(78);
    pub const ESCAPE: Self = Self(120);

    pub fn from_code(code: u16) -> Result<Self, Error> {
        if code > 175 {
            return Err(Error::InvalidInput("unknown physical key code"));
        }
        Ok(Self(code))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub super_key: bool,
    pub caps_lock: bool,
    pub num_lock: bool,
}

impl Modifiers {
    fn bits(self) -> u16 {
        u16::from(self.shift)
            | (u16::from(self.control) << 1)
            | (u16::from(self.alt) << 2)
            | (u16::from(self.super_key) << 3)
            | (u16::from(self.caps_lock) << 4)
            | (u16::from(self.num_lock) << 5)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub enum KeyAction {
    Release,
    #[default]
    Press,
    Repeat,
}

#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub action: KeyAction,
    pub modifiers: Modifiers,
    pub consumed_modifiers: Modifiers,
    pub composing: bool,
    /// Layout text before Ctrl/Meta transformations; empty for functional keys.
    pub text: String,
    pub unshifted: Option<char>,
    /// Platform preference, applied after set_from_terminal resets this option.
    pub option_as_alt: bool,
}

impl KeyEvent {
    pub fn new(code: KeyCode) -> Self {
        Self {
            code,
            action: KeyAction::Press,
            modifiers: Modifiers::default(),
            consumed_modifiers: Modifiers::default(),
            composing: false,
            text: String::new(),
            unshifted: None,
            option_as_alt: false,
        }
    }

    /// Check owned input before bounded backend admission; does no native work.
    pub fn validate(&self) -> Result<(), Error> {
        if self.text.len() > MAX_KEY_TEXT_BYTES {
            return Err(Error::InputLimit);
        }
        if self
            .text
            .chars()
            .any(|c| c <= '\u{1f}' || c == '\u{7f}' || ('\u{f700}'..='\u{f8ff}').contains(&c))
        {
            return Err(Error::InvalidInput(
                "key text contains a control or platform function code",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MouseAction {
    Press,
    Release,
    Motion,
}

#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Eleven,
}

#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    pub width: u32,
    pub height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub padding_top: u32,
    pub padding_bottom: u32,
    pub padding_right: u32,
    pub padding_left: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    pub action: MouseAction,
    pub button: Option<MouseButton>,
    pub modifiers: Modifiers,
    pub x: f32,
    pub y: f32,
    pub any_button_pressed: bool,
    pub geometry: Geometry,
}

impl MouseEvent {
    /// Check owned input before bounded backend admission; does no native work.
    pub fn validate(&self) -> Result<(), Error> {
        let g = self.geometry;
        if g.cell_width == 0
            || g.cell_height == 0
            || g.width > i32::MAX as u32
            || g.height > i32::MAX as u32
            // Upstream renderer geometry converts these f32 ratios to u16.
            || g.width as f32 / g.cell_width as f32 >= 65536.0
            || g.height as f32 / g.cell_height as f32 >= 65536.0
            || u64::from(g.width)
                < u64::from(g.cell_width) + u64::from(g.padding_left) + u64::from(g.padding_right)
            || u64::from(g.height)
                < u64::from(g.cell_height) + u64::from(g.padding_top) + u64::from(g.padding_bottom)
            || !self.x.is_finite()
            || !self.y.is_finite()
            || self.x < 0.0
            || self.y < 0.0
            || f64::from(self.x) > f64::from(g.width)
            || f64::from(self.y) > f64::from(g.height)
        {
            return Err(Error::InvalidInput("invalid mouse position or geometry"));
        }
        if !matches!(self.action, MouseAction::Motion) && self.button.is_none() {
            return Err(Error::InvalidInput("mouse button event has no button"));
        }
        Ok(())
    }
}

#[repr(C)]
struct Text {
    ptr: *const u8,
    len: usize,
}

#[repr(C)]
struct Position {
    x: f32,
    y: f32,
}

#[repr(C)]
struct NativeGeometry {
    size: usize,
    width: u32,
    height: u32,
    cell_width: u32,
    cell_height: u32,
    padding_top: u32,
    padding_bottom: u32,
    padding_right: u32,
    padding_left: u32,
}

unsafe extern "C" {
    fn ghostty_key_encoder_new(allocator: *const c_void, out: *mut *mut c_void) -> c_int;
    fn ghostty_key_encoder_free(encoder: *mut c_void);
    fn ghostty_key_event_new(allocator: *const c_void, out: *mut *mut c_void) -> c_int;
    fn ghostty_key_event_free(event: *mut c_void);
    fn ghostty_key_encoder_set_from_terminal(encoder: *mut c_void, terminal: *mut c_void) -> c_int;
    fn ghostty_key_encoder_set(encoder: *mut c_void, option: c_int, value: *const c_void) -> c_int;
    fn ghostty_key_event_set_action(event: *mut c_void, action: c_int) -> c_int;
    fn ghostty_key_event_set_key(event: *mut c_void, key: c_int) -> c_int;
    fn ghostty_key_event_set_mods(event: *mut c_void, mods: u16) -> c_int;
    fn ghostty_key_event_set_consumed_mods(event: *mut c_void, mods: u16) -> c_int;
    fn ghostty_key_event_set_composing(event: *mut c_void, composing: bool) -> c_int;
    fn ghostty_key_event_set_utf8(event: *mut c_void, text: Text) -> c_int;
    fn ghostty_key_event_set_unshifted_codepoint(event: *mut c_void, codepoint: u32) -> c_int;
    fn ghostty_key_encoder_encode(
        encoder: *mut c_void,
        event: *mut c_void,
        out: *mut u8,
        len: usize,
        written: *mut usize,
    ) -> c_int;
    fn ghostty_mouse_encoder_new(allocator: *const c_void, out: *mut *mut c_void) -> c_int;
    fn ghostty_mouse_encoder_free(encoder: *mut c_void);
    fn ghostty_mouse_event_new(allocator: *const c_void, out: *mut *mut c_void) -> c_int;
    fn ghostty_mouse_event_free(event: *mut c_void);
    fn ghostty_mouse_encoder_set_from_terminal(
        encoder: *mut c_void,
        terminal: *mut c_void,
    ) -> c_int;
    fn ghostty_mouse_encoder_set(
        encoder: *mut c_void,
        option: c_int,
        value: *const c_void,
    ) -> c_int;
    fn ghostty_mouse_event_set_action(event: *mut c_void, action: c_int) -> c_int;
    fn ghostty_mouse_event_set_button(event: *mut c_void, button: c_int) -> c_int;
    fn ghostty_mouse_event_set_mods(event: *mut c_void, mods: u16) -> c_int;
    fn ghostty_mouse_event_set_position(event: *mut c_void, position: Position) -> c_int;
    fn ghostty_mouse_encoder_encode(
        encoder: *mut c_void,
        event: *mut c_void,
        out: *mut u8,
        len: usize,
        written: *mut usize,
    ) -> c_int;
}

type Create = unsafe extern "C" fn(*const c_void, *mut *mut c_void) -> c_int;
type Free = unsafe extern "C" fn(*mut c_void);
type Encode = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut u8, usize, *mut usize) -> c_int;

struct Handle {
    raw: NonNull<c_void>,
    free: Free,
    _thread_confined: PhantomData<Rc<()>>,
}

impl Handle {
    fn new(create: Create, free: Free) -> Result<Self, Error> {
        let mut raw = ptr::null_mut();
        // SAFETY: private call sites pair the pinned create/free functions;
        // the default allocator and output slot remain valid for this call.
        check(unsafe { create(ptr::null(), &mut raw) })?;
        Ok(Self {
            raw: NonNull::new(raw).ok_or(Error::NullHandle)?,
            free,
            _thread_confined: PhantomData,
        })
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: unique live owner; private construction pairs the destructor.
        unsafe { (self.free)(self.raw.as_ptr()) };
    }
}

fn encode(encoder: &Handle, event: &Handle, call: Encode) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_ENCODED_BYTES)
        .map_err(|_| Error::Allocation)?;
    bytes.resize(MAX_ENCODED_BYTES, 0);
    let mut written = 0;
    // SAFETY: paired private encoder/event owners, writable bounded output and
    // length slot. One call only: discard all output if native encoding fails.
    let status = unsafe {
        call(
            encoder.raw.as_ptr(),
            event.raw.as_ptr(),
            bytes.as_mut_ptr(),
            bytes.len(),
            &mut written,
        )
    };
    if status == -3 || written > bytes.len() {
        return Err(Error::InputLimit);
    }
    check(status)?;
    bytes.truncate(written);
    Ok(bytes)
}

impl Terminal {
    pub fn encode_key(&mut self, key: &KeyEvent) -> Result<Vec<u8>, Error> {
        key.validate()?;
        let encoder = Handle::new(ghostty_key_encoder_new, ghostty_key_encoder_free)?;
        let event = Handle::new(ghostty_key_event_new, ghostty_key_event_free)?;
        // SAFETY: all handles belong to this thread and remain live. The event
        // borrows key.text until its destructor, before this method returns.
        // Each option/value uses the exact pinned C type; no native borrow escapes.
        unsafe {
            check(ghostty_key_encoder_set_from_terminal(
                encoder.raw.as_ptr(),
                self.raw.as_ptr(),
            ))?;
            let option: c_int = i32::from(key.option_as_alt);
            check(ghostty_key_encoder_set(
                encoder.raw.as_ptr(),
                7,
                ptr::from_ref(&option).cast(),
            ))?;
            check(ghostty_key_event_set_key(
                event.raw.as_ptr(),
                c_int::from(key.code.0),
            ))?;
            check(ghostty_key_event_set_action(
                event.raw.as_ptr(),
                match key.action {
                    KeyAction::Release => 0,
                    KeyAction::Press => 1,
                    KeyAction::Repeat => 2,
                },
            ))?;
            check(ghostty_key_event_set_mods(
                event.raw.as_ptr(),
                key.modifiers.bits(),
            ))?;
            check(ghostty_key_event_set_consumed_mods(
                event.raw.as_ptr(),
                key.consumed_modifiers.bits(),
            ))?;
            check(ghostty_key_event_set_composing(
                event.raw.as_ptr(),
                key.composing,
            ))?;
            check(ghostty_key_event_set_utf8(
                event.raw.as_ptr(),
                Text {
                    ptr: key.text.as_ptr(),
                    len: key.text.len(),
                },
            ))?;
            check(ghostty_key_event_set_unshifted_codepoint(
                event.raw.as_ptr(),
                key.unshifted.map_or(0, u32::from),
            ))?;
        }
        encode(&encoder, &event, ghostty_key_encoder_encode)
    }

    pub fn encode_mouse(&mut self, mouse: &MouseEvent) -> Result<Vec<u8>, Error> {
        mouse.validate()?;
        let encoder = Handle::new(ghostty_mouse_encoder_new, ghostty_mouse_encoder_free)?;
        let event = Handle::new(ghostty_mouse_event_new, ghostty_mouse_event_free)?;
        let g = mouse.geometry;
        let geometry = NativeGeometry {
            size: size_of::<NativeGeometry>(),
            width: g.width,
            height: g.height,
            cell_width: g.cell_width,
            cell_height: g.cell_height,
            padding_top: g.padding_top,
            padding_bottom: g.padding_bottom,
            padding_right: g.padding_right,
            padding_left: g.padding_left,
        };
        // SAFETY: live thread-confined handles and pinned sized geometry. Native
        // setters copy values synchronously; output is copied before freeing.
        unsafe {
            check(ghostty_mouse_encoder_set_from_terminal(
                encoder.raw.as_ptr(),
                self.raw.as_ptr(),
            ))?;
            check(ghostty_mouse_encoder_set(
                encoder.raw.as_ptr(),
                3,
                ptr::from_ref(&geometry).cast(),
            ))?;
            check(ghostty_mouse_encoder_set(
                encoder.raw.as_ptr(),
                4,
                ptr::from_ref(&mouse.any_button_pressed).cast(),
            ))?;
            check(ghostty_mouse_event_set_action(
                event.raw.as_ptr(),
                match mouse.action {
                    MouseAction::Press => 0,
                    MouseAction::Release => 1,
                    MouseAction::Motion => 2,
                },
            ))?;
            if let Some(button) = mouse.button {
                let value = match button {
                    MouseButton::Left => 1,
                    MouseButton::Right => 2,
                    MouseButton::Middle => 3,
                    MouseButton::Four => 4,
                    MouseButton::Five => 5,
                    MouseButton::Six => 6,
                    MouseButton::Seven => 7,
                    MouseButton::Eight => 8,
                    MouseButton::Nine => 9,
                    MouseButton::Ten => 10,
                    MouseButton::Eleven => 11,
                };
                check(ghostty_mouse_event_set_button(event.raw.as_ptr(), value))?;
            }
            check(ghostty_mouse_event_set_mods(
                event.raw.as_ptr(),
                mouse.modifiers.bits(),
            ))?;
            check(ghostty_mouse_event_set_position(
                event.raw.as_ptr(),
                Position {
                    x: mouse.x,
                    y: mouse.y,
                },
            ))?;
        }
        encode(&encoder, &event, ghostty_mouse_encoder_encode)
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
