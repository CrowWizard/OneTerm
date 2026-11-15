//! Fixed callback capture. Borrowed native strings are copied before return;
//! allocation and UTF-8 decoding happen only during the owned drain.

use std::ffi::c_void;
use std::ptr;

use super::{Error, Replies, Terminal, check, ghostty_terminal_get, ghostty_terminal_set};

pub const MAX_EVENTS: usize = 256;
pub const MAX_EVENT_BYTES: usize = 384 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Title(String),
    WorkingDirectory(String),
    Bell,
    Clipboard(String),
    ClipboardRead,
}

impl Event {
    pub fn text_len(&self) -> usize {
        match self {
            Self::Title(text) | Self::WorkingDirectory(text) | Self::Clipboard(text) => text.len(),
            Self::Bell | Self::ClipboardRead => 0,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Title,
    Directory,
    Bell,
    Clipboard,
    ClipboardRead,
}

#[derive(Clone, Copy)]
struct Record {
    kind: Kind,
    start: usize,
    len: usize,
}

#[repr(C)]
struct NativeString {
    data: *const u8,
    len: usize,
}

pub(super) struct Capture {
    records: [Record; MAX_EVENTS],
    count: usize,
    bytes: [u8; MAX_EVENT_BYTES],
    len: usize,
    failure: Option<Error>,
}

impl Capture {
    pub(super) fn new() -> Self {
        Self {
            records: [Record {
                kind: Kind::Bell,
                start: 0,
                len: 0,
            }; MAX_EVENTS],
            count: 0,
            bytes: [0; MAX_EVENT_BYTES],
            len: 0,
            failure: None,
        }
    }

    pub(super) fn check(&self) -> Result<(), Error> {
        match &self.failure {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    pub(super) unsafe fn push(&mut self, kind: Kind, data: *const u8, len: usize) {
        if self.failure.is_some() {
            return;
        }
        if self.count == MAX_EVENTS || len > MAX_EVENT_BYTES - self.len {
            self.failure = Some(Error::EventOverflow);
            return;
        }
        if len != 0 && data.is_null() {
            self.failure = Some(Error::InvalidEvent);
            return;
        }
        if len != 0 {
            // SAFETY: callback lends len bytes; destination capacity was checked.
            unsafe {
                ptr::copy_nonoverlapping(data, self.bytes.as_mut_ptr().add(self.len), len);
            }
        }
        // SAFETY: count is below the fixed capacity, checked above.
        unsafe {
            self.records.as_mut_ptr().add(self.count).write(Record {
                kind,
                start: self.len,
                len,
            });
        }
        self.count += 1;
        self.len += len;
    }

    pub(super) fn drain(&mut self) -> Result<Vec<Event>, Error> {
        self.check()?;
        let mut events = Vec::new();
        events
            .try_reserve_exact(self.count)
            .map_err(|_| Error::Allocation)?;
        for record in &self.records[..self.count] {
            let event = match record.kind {
                Kind::Bell => Event::Bell,
                Kind::ClipboardRead => Event::ClipboardRead,
                kind => {
                    let bytes = &self.bytes[record.start..record.start + record.len];
                    let text = std::str::from_utf8(bytes).map_err(|_| Error::InvalidEvent)?;
                    let mut owned = String::new();
                    owned
                        .try_reserve_exact(text.len())
                        .map_err(|_| Error::Allocation)?;
                    owned.push_str(text);
                    match kind {
                        Kind::Title => Event::Title(owned),
                        Kind::Clipboard => Event::Clipboard(owned),
                        _ => Event::WorkingDirectory(owned),
                    }
                }
            };
            events.push(event);
        }
        self.count = 0;
        self.len = 0;
        Ok(events)
    }
}

unsafe fn capture_string(terminal: *mut c_void, userdata: *mut c_void, key: i32, kind: Kind) {
    // SAFETY: stable boxed userdata is registered until terminal destruction.
    let capture = unsafe { &mut (*userdata.cast::<Replies>()).events };
    if capture.failure.is_some() {
        return;
    }
    let mut value = NativeString {
        data: ptr::null(),
        len: 0,
    };
    // SAFETY: the pinned title/PWD keys write GhosttyString. Upstream explicitly
    // permits these read-only getters inside their respective callbacks.
    let status = unsafe { ghostty_terminal_get(terminal, key, ptr::from_mut(&mut value).cast()) };
    if status != 0 {
        capture.failure = Some(Error::InvalidEvent);
        return;
    }
    // SAFETY: getter borrows bytes through this synchronous callback.
    unsafe {
        capture.push(kind, value.data, value.len);
    }
}

unsafe extern "C" fn title(terminal: *mut c_void, userdata: *mut c_void) {
    // SAFETY: exact callback ABI; title getter is permitted here.
    unsafe {
        capture_string(terminal, userdata, 12, Kind::Title);
    }
}

unsafe extern "C" fn directory(terminal: *mut c_void, userdata: *mut c_void) {
    // SAFETY: exact callback ABI; PWD getter is permitted here.
    unsafe {
        capture_string(terminal, userdata, 13, Kind::Directory);
    }
}

unsafe extern "C" fn bell(_terminal: *mut c_void, userdata: *mut c_void) {
    // SAFETY: stable callback context, empty bell payload.
    unsafe {
        (*userdata.cast::<Replies>())
            .events
            .push(Kind::Bell, ptr::null(), 0);
    }
}

pub(super) fn install(terminal: &mut Terminal) -> Result<(), Error> {
    for (key, callback) in [
        (3, bell as *const ()),
        (6, title as *const ()),
        (26, directory as *const ()),
    ] {
        // SAFETY: exact pinned callback signatures, stable owner context. No
        // clipboard or graphics callbacks or local-file media are enabled.
        check(unsafe { ghostty_terminal_set(terminal.raw.as_ptr(), key, callback.cast()) })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_string_layout_matches_the_c_probe() {
        assert_eq!(std::mem::size_of::<NativeString>(), 16);
        assert_eq!(std::mem::align_of::<NativeString>(), 8);
        assert_eq!(std::mem::offset_of!(NativeString, data), 0);
        assert_eq!(std::mem::offset_of!(NativeString, len), 8);
    }

    #[test]
    fn callback_strings_and_order_survive_splits_mutation_and_drop() {
        let stream = "\x1b]2;标题\x07\x1b]7;file://host/C:/a%20b\x1b\\\x07".as_bytes();
        for split in 0..=stream.len() {
            let mut terminal = Terminal::new(16, 3).unwrap();
            terminal.write(&stream[..split]).unwrap();
            let mut events = terminal.take_events().unwrap();
            terminal.write(&stream[split..]).unwrap();
            events.extend(terminal.take_events().unwrap());
            terminal.write(b"\x1b]2;other\x07").unwrap();
            drop(terminal);
            assert_eq!(
                events,
                [
                    Event::Title("标题".into()),
                    Event::WorkingDirectory("file://host/C:/a%20b".into()),
                    Event::Bell
                ]
            );
        }
    }

    #[test]
    fn event_count_overflow_is_sticky_and_drain_does_not_truncate() {
        let mut terminal = Terminal::new(16, 3).unwrap();
        terminal.write(&vec![7; MAX_EVENTS]).unwrap();
        assert_eq!(terminal.take_events().unwrap().len(), MAX_EVENTS);
        assert_eq!(
            terminal.write(&vec![7; MAX_EVENTS + 1]),
            Err(Error::EventOverflow)
        );
        assert_eq!(terminal.take_events(), Err(Error::EventOverflow));
        assert_eq!(terminal.write(b"later"), Err(Error::EventOverflow));
    }

    #[test]
    fn fixed_arena_rejects_text_overflow_and_invalid_data() {
        let mut capture = Capture::new();
        let text = vec![b'x'; MAX_EVENT_BYTES];
        // SAFETY: fixture lends valid bytes for each synchronous copy.
        unsafe {
            capture.push(Kind::Title, text.as_ptr(), text.len());
        }
        assert_eq!(
            capture.drain().unwrap(),
            [Event::Title("x".repeat(MAX_EVENT_BYTES))]
        );
        unsafe {
            capture.push(Kind::Title, text.as_ptr(), text.len());
            capture.push(Kind::Title, text.as_ptr(), 1);
        }
        assert_eq!(capture.drain(), Err(Error::EventOverflow));
        let mut invalid = Capture::new();
        unsafe {
            invalid.push(Kind::Title, ptr::null(), 1);
        }
        assert_eq!(invalid.drain(), Err(Error::InvalidEvent));
    }
}
