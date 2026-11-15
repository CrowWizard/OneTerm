//! Observe OSC boundaries reported by the native parser; never parse VT twice.

use std::ffi::c_void;

use base64::Engine;

use super::events::Kind;
use super::{Error, Terminal};

const START: u32 = 1 << 5;
const END: u32 = 1 << 6;
const MAX_TEXT: usize = 256 * 1024;
const MAX_SEQUENCE: usize = MAX_TEXT.div_ceil(3) * 4 + 32;

#[repr(C)]
struct Stop {
    size: usize,
    consumed: usize,
    reached: u32,
    kind: i32,
    introducer: i32,
    end: i32,
    byte: u8,
}

unsafe extern "C" {
    fn ghostty_terminal_vt_write_until(
        terminal: *mut c_void,
        bytes: *const u8,
        len: usize,
        boundaries: u32,
        stop: *mut Stop,
    ) -> i32;
}

#[derive(Default)]
pub(super) struct Observer {
    active: bool,
    oversized: bool,
    bytes: Vec<u8>,
}

impl Terminal {
    pub(super) fn write_observed(&mut self, mut bytes: &[u8]) -> Result<(), Error> {
        while !bytes.is_empty() {
            let mut stop = Stop {
                size: size_of::<Stop>(),
                consumed: 0,
                reached: 0,
                kind: 0,
                introducer: 0,
                end: 0,
                byte: 0,
            };
            // SAFETY: pinned sized ABI, live exclusive terminal and borrowed input.
            let result = unsafe {
                ghostty_terminal_vt_write_until(
                    self.raw.as_ptr(),
                    bytes.as_ptr(),
                    bytes.len(),
                    START | END,
                    &mut stop,
                )
            };
            if result != 0 && result != -4 {
                return Err(Error::Native(result));
            }
            if stop.consumed == 0 || stop.consumed > bytes.len() {
                return Err(Error::InvalidEvent);
            }
            let observer = &mut self.clipboard;
            if observer.active && !observer.oversized {
                let len = stop.consumed - usize::from(stop.reached & END != 0);
                if len > MAX_SEQUENCE - observer.bytes.len() {
                    observer.oversized = true;
                    observer.bytes.clear();
                } else {
                    observer
                        .bytes
                        .try_reserve(len)
                        .map_err(|_| Error::Allocation)?;
                    observer.bytes.extend_from_slice(&bytes[..len]);
                }
            }
            if stop.reached & END != 0 {
                if observer.active && !observer.oversized && matches!(stop.end, 0 | 4) {
                    if let Some(payload) = observer.bytes.strip_prefix(b"52;") {
                        if let Some(separator) = payload.iter().position(|byte| *byte == b';') {
                            let payload = &payload[separator + 1..];
                            if payload == b"?" {
                                // SAFETY: an empty event has no borrowed payload.
                                unsafe {
                                    self.replies.events.push(
                                        Kind::ClipboardRead,
                                        std::ptr::null(),
                                        0,
                                    )
                                };
                            } else if let Ok(decoded) =
                                base64::engine::general_purpose::STANDARD.decode(payload)
                            {
                                if decoded.len() <= MAX_TEXT
                                    && std::str::from_utf8(&decoded).is_ok()
                                {
                                    // SAFETY: copy the owned decoded bytes before they are dropped.
                                    unsafe {
                                        self.replies.events.push(
                                            Kind::Clipboard,
                                            decoded.as_ptr(),
                                            decoded.len(),
                                        )
                                    };
                                }
                            }
                        }
                    }
                }
                observer.active = false;
                observer.bytes.clear();
            }
            if stop.reached & START != 0 {
                observer.active = true;
                observer.oversized = false;
                observer.bytes.clear();
            }
            bytes = &bytes[stop.consumed..];
            self.replies.events.check()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::events::Event;
    use super::*;

    #[test]
    fn native_boundaries_preserve_fragmented_clipboard_and_never_reply_early() {
        let stream = b"\x1b]52;c;aGk=\x07\x1b]52;c;?\x1b\\";
        for split in 0..=stream.len() {
            let mut terminal = Terminal::new(16, 3).unwrap();
            terminal.write(&stream[..split]).unwrap();
            let mut events = terminal.take_events().unwrap();
            terminal.write(&stream[split..]).unwrap();
            events.extend(terminal.take_events().unwrap());
            assert_eq!(
                events,
                [Event::Clipboard("hi".into()), Event::ClipboardRead]
            );
            assert!(terminal.take_replies().unwrap().is_empty());
        }
    }

    #[test]
    fn clipboard_st_termination_bounds_and_event_order() {
        let stream = b"\x1b]2;before\x07\x1b]52;c;aGk=\x1b\\\x07\x1b]2;after\x07";
        for split in 0..=stream.len() {
            let mut terminal = Terminal::new(16, 3).unwrap();
            terminal.write(&stream[..split]).unwrap();
            let mut events = terminal.take_events().unwrap();
            terminal.write(&stream[split..]).unwrap();
            events.extend(terminal.take_events().unwrap());
            assert_eq!(
                events,
                [
                    Event::Title("before".into()),
                    Event::Clipboard("hi".into()),
                    Event::Bell,
                    Event::Title("after".into())
                ]
            );
        }
        let mut terminal = Terminal::new(16, 3).unwrap();
        let text = "x".repeat(MAX_TEXT);
        let encoded = base64::engine::general_purpose::STANDARD.encode(&text);
        terminal
            .write(format!("\x1b]52;c;{encoded}\x1b\\").as_bytes())
            .unwrap();
        assert_eq!(terminal.take_events().unwrap(), [Event::Clipboard(text)]);
    }

    #[test]
    fn invalid_cancelled_and_oversized_clipboard_are_inert() {
        let mut terminal = Terminal::new(16, 3).unwrap();
        terminal
            .write(b"\x1b]52;c;!bad!\x07\x1b]52;c;/w==\x07\x1b]52;c;aGk=\x18")
            .unwrap();
        let text = "x".repeat(MAX_TEXT + 1);
        terminal
            .write(
                format!(
                    "\x1b]52;c;{}\x07",
                    base64::engine::general_purpose::STANDARD.encode(text)
                )
                .as_bytes(),
            )
            .unwrap();
        assert!(terminal.take_events().unwrap().is_empty());
        terminal.write(b"\x1b]52;c;\x07").unwrap();
        assert_eq!(
            terminal.take_events().unwrap(),
            [Event::Clipboard(String::new())]
        );
    }
}
