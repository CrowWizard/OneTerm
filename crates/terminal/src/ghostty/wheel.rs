//! Atomic live-owner wheel policy; no published frame participates in routing.

use super::input::{KeyCode, KeyEvent, MAX_ENCODED_BYTES, MouseAction, MouseButton, MouseEvent};
use super::{Error, Terminal, state::Scroll};

pub const MAX_STEPS: i32 = 10;

#[derive(Debug, Clone, Copy)]
pub struct WheelEvent {
    /// Positive is up/toward older history. Zero is not an input operation.
    pub steps: i32,
    /// Only position, geometry, modifiers and held-button state are used.
    pub pointer: MouseEvent,
}

impl WheelEvent {
    pub fn validate(&self) -> Result<(), Error> {
        if self.steps == 0 || !(-MAX_STEPS..=MAX_STEPS).contains(&self.steps) {
            return Err(Error::InvalidInput(
                "wheel steps must be nonzero and bounded to ten",
            ));
        }
        self.pointer.validate()
    }
}

#[derive(Debug)]
pub enum Outcome {
    Bytes(Vec<u8>),
    Scrolled,
    Ignored,
}

impl Terminal {
    pub(super) fn wheel(&mut self, wheel: WheelEvent) -> Result<Outcome, Error> {
        wheel.validate()?;
        let state = self.metadata()?;
        let history = state.scrollbar.offset < state.scrollbar.total - state.scrollbar.len;
        let local = wheel.pointer.modifiers.shift || (!state.alternate_screen && history);
        let tracking = state.modes.mouse_x10
            || state.modes.mouse_normal
            || state.modes.mouse_button
            || state.modes.mouse_any;
        let bytes = if !local && tracking {
            let mouse = MouseEvent {
                action: MouseAction::Press,
                button: Some(if wheel.steps > 0 {
                    MouseButton::Four
                } else {
                    MouseButton::Five
                }),
                ..wheel.pointer
            };
            Some(self.encode_mouse(&mouse)?)
        } else if !local && state.alternate_screen && state.modes.alternate_scroll {
            let mut key = KeyEvent::new(if wheel.steps > 0 {
                KeyCode::ARROW_UP
            } else {
                KeyCode::ARROW_DOWN
            });
            key.modifiers = wheel.pointer.modifiers;
            Some(self.encode_key(&key)?)
        } else {
            None
        };
        if let Some(bytes) = bytes {
            let count = wheel.steps.unsigned_abs() as usize;
            let len = bytes
                .len()
                .checked_mul(count)
                .filter(|len| *len <= MAX_ENCODED_BYTES)
                .ok_or(Error::InputLimit)?;
            let mut output = Vec::new();
            output
                .try_reserve_exact(len)
                .map_err(|_| Error::Allocation)?;
            for _ in 0..count {
                output.extend_from_slice(&bytes);
            }
            return Ok(Outcome::Bytes(output));
        }
        if state.alternate_screen {
            return Ok(Outcome::Ignored);
        }
        self.scroll(Scroll::Delta(-wheel.steps))?;
        Ok(Outcome::Scrolled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ghostty::{
        input::{Geometry, Modifiers},
        owner::EngineOwner,
    };

    fn event(steps: i32) -> WheelEvent {
        WheelEvent {
            steps,
            pointer: MouseEvent {
                action: MouseAction::Motion,
                button: None,
                modifiers: Modifiers::default(),
                x: 12.5,
                y: 25.0,
                any_button_pressed: false,
                geometry: Geometry {
                    width: 160,
                    height: 60,
                    cell_width: 10,
                    cell_height: 20,
                    padding_top: 0,
                    padding_bottom: 0,
                    padding_left: 0,
                    padding_right: 0,
                },
            },
        }
    }

    fn bytes(outcome: Outcome) -> Vec<u8> {
        match outcome {
            Outcome::Bytes(bytes) => bytes,
            other => panic!("expected bytes, got {other:?}"),
        }
    }

    #[test]
    fn wheel_native_owner_uses_live_modes_without_capture() {
        let mut owner = EngineOwner::new(16, 3).unwrap();
        owner.capture().unwrap();
        owner.feed(b"\x1b[?1000;1006h").unwrap();
        assert_eq!(
            bytes(owner.wheel(event(2)).unwrap()),
            b"\x1b[<64;2;2M".repeat(2)
        );
        assert_eq!(bytes(owner.wheel(event(-1)).unwrap()), b"\x1b[<65;2;2M");
        let mut modified = event(1);
        modified.pointer.modifiers.control = true;
        assert_eq!(bytes(owner.wheel(modified).unwrap()), b"\x1b[<80;2;2M");
        owner.feed(b"\x1b[?1000l\x1b[?1049;1007h").unwrap();
        assert_eq!(bytes(owner.wheel(event(2)).unwrap()), b"\x1b[A\x1b[A");
        owner.feed(b"\x1b[?1h").unwrap();
        assert_eq!(bytes(owner.wheel(event(-2)).unwrap()), b"\x1bOB\x1bOB");
        owner.feed(b"\x1b[?1007l").unwrap();
        assert!(matches!(owner.wheel(event(1)).unwrap(), Outcome::Ignored));
        owner.feed(b"\x1b[?1000h").unwrap();
        assert_eq!(bytes(owner.wheel(event(1)).unwrap()), b"\x1b[<64;2;2M");
        let mut shift = event(1);
        shift.pointer.modifiers.shift = true;
        assert!(matches!(owner.wheel(shift).unwrap(), Outcome::Ignored));
        owner.close().unwrap();
    }

    #[test]
    fn wheel_history_and_shift_precede_mouse_and_validate_before_owner() {
        let mut owner = EngineOwner::new(16, 3).unwrap();
        for steps in [0, 11, -11, i32::MIN, i32::MAX] {
            assert!(owner.wheel(event(steps)).is_err());
        }
        let mut invalid = event(1);
        invalid.pointer.x = f32::NAN;
        assert!(owner.wheel(invalid).is_err());
        owner
            .feed(b"zero\r\none\r\ntwo\r\nthree\r\nfour\x1b[?1000;1006h")
            .unwrap();
        let bottom = owner.capture().unwrap().metadata.scrollbar;
        let mut shift = event(1);
        shift.pointer.modifiers.shift = true;
        assert!(matches!(owner.wheel(shift).unwrap(), Outcome::Scrolled));
        assert_eq!(
            owner.capture().unwrap().metadata.scrollbar.offset,
            bottom.offset - 1
        );
        assert!(matches!(owner.wheel(event(10)).unwrap(), Outcome::Scrolled));
        assert_eq!(owner.capture().unwrap().metadata.scrollbar.offset, 0);
        assert!(matches!(
            owner.wheel(event(-10)).unwrap(),
            Outcome::Scrolled
        ));
        assert_eq!(owner.capture().unwrap().metadata.scrollbar, bottom);
        assert_eq!(bytes(owner.wheel(event(1)).unwrap()), b"\x1b[<64;2;2M");
        owner.close().unwrap();
    }
}
