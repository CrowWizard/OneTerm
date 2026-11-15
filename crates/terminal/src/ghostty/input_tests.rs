use super::*;
use crate::ghostty::owner::{Damage, EngineOwner, State};
use crate::ghostty::render::RenderBuffer;

fn ctrl_c() -> KeyEvent {
    let mut key = KeyEvent::new(KeyCode::from_code(22).unwrap());
    key.text = "c".into();
    key.unshifted = Some('c');
    key.modifiers.control = true;
    key
}

fn mouse() -> MouseEvent {
    MouseEvent {
        action: MouseAction::Press,
        button: Some(MouseButton::Left),
        modifiers: Modifiers::default(),
        x: 25.0,
        y: 21.0,
        any_button_pressed: true,
        geometry: Geometry {
            width: 120,
            height: 60,
            cell_width: 10,
            cell_height: 20,
            padding_top: 0,
            padding_bottom: 0,
            padding_left: 0,
            padding_right: 0,
        },
    }
}

#[test]
fn named_function_keys_follow_native_cursor_mode() {
    let mut terminal = Terminal::new(12, 3).unwrap();
    for (code, normal, application) in [
        (KeyCode::ENTER, b"\r".as_slice(), b"\r".as_slice()),
        (KeyCode::TAB, b"\t", b"\t"),
        (KeyCode::ESCAPE, b"\x1b", b"\x1b"),
        (KeyCode::ARROW_DOWN, b"\x1b[B", b"\x1bOB"),
        (KeyCode::ARROW_LEFT, b"\x1b[D", b"\x1bOD"),
        (KeyCode::ARROW_RIGHT, b"\x1b[C", b"\x1bOC"),
    ] {
        let key = KeyEvent::new(code);
        terminal.write(b"\x1b[?1l").unwrap();
        assert_eq!(terminal.encode_key(&key).unwrap(), normal);
        terminal.write(b"\x1b[?1h").unwrap();
        assert_eq!(terminal.encode_key(&key).unwrap(), application);
    }
}

#[test]
fn paste_mode_reads_unpublished_state_and_reset() {
    let mut owner = EngineOwner::new(12, 3).unwrap();
    assert!(!owner.bracketed_paste().unwrap());
    owner.feed(b"\x1b[?2004h").unwrap();
    assert!(owner.bracketed_paste().unwrap());
    owner.feed(b"\x1b[?2004l").unwrap();
    assert!(!owner.bracketed_paste().unwrap());
    owner.feed(b"\x1b[?2004h\x1bc").unwrap();
    assert!(!owner.bracketed_paste().unwrap());
    owner.close().unwrap();
}

#[test]
fn sgr_buttons_wheels_modifiers_and_padding_use_native_encoding() {
    let mut terminal = Terminal::new(12, 3).unwrap();
    terminal.write(b"\x1b[?1000;1006h").unwrap();
    for (button, code) in [
        (MouseButton::Left, Some(0)),
        (MouseButton::Right, Some(2)),
        (MouseButton::Middle, Some(1)),
        (MouseButton::Four, Some(64)),
        (MouseButton::Five, Some(65)),
        (MouseButton::Six, Some(66)),
        (MouseButton::Seven, Some(67)),
        (MouseButton::Eight, Some(128)),
        (MouseButton::Nine, Some(129)),
        // The pinned encoder accepts these event identities but has no wire
        // representation for them; do not invent extra-button escape codes.
        (MouseButton::Ten, None),
        (MouseButton::Eleven, None),
    ] {
        let mut event = mouse();
        event.button = Some(button);
        event.geometry.padding_left = 5;
        event.geometry.padding_top = 1;
        event.modifiers.shift = true;
        event.modifiers.alt = true;
        event.modifiers.control = true;
        let expected = code.map_or_else(Vec::new, |code| {
            format!("\x1b[<{};3;2M", code + 28).into_bytes()
        });
        assert_eq!(terminal.encode_mouse(&event).unwrap(), expected);
    }
    terminal.write(b"\x1b[?1016h").unwrap();
    let mut event = mouse();
    event.geometry.padding_left = 5;
    event.geometry.padding_top = 1;
    assert_eq!(terminal.encode_mouse(&event).unwrap(), b"\x1b[<0;20;20M");
}

#[test]
fn keyboard_uses_live_cursor_backarrow_and_modify_other_keys() {
    let mut owner = EngineOwner::new(12, 3).unwrap();
    let up = KeyEvent::new(KeyCode::ARROW_UP);
    assert_eq!(owner.encode_key(up.clone()).unwrap(), b"\x1b[A");
    owner.feed(b"\x1b[?1h").unwrap();
    assert_eq!(owner.encode_key(up.clone()).unwrap(), b"\x1bOA");
    let backspace = KeyEvent::new(KeyCode::BACKSPACE);
    assert_eq!(owner.encode_key(backspace.clone()).unwrap(), b"\x7f");
    owner.feed(b"\x1b[?67h").unwrap();
    assert_eq!(owner.encode_key(backspace).unwrap(), b"\x08");
    assert_eq!(owner.encode_key(ctrl_c()).unwrap(), b"\x03");
    owner.feed(b"\x1b[>4;2m").unwrap();
    assert_eq!(owner.encode_key(ctrl_c()).unwrap(), b"\x1b[27;5;99~");
    owner.feed(b"\x1bc").unwrap();
    assert_eq!(owner.encode_key(up).unwrap(), b"\x1b[A");
    assert_eq!(owner.encode_key(ctrl_c()).unwrap(), b"\x03");
    owner.close().unwrap();
}

#[test]
fn kitty_events_follow_active_screen_and_release_negotiation() {
    let mut owner = EngineOwner::new(12, 3).unwrap();
    let mut key = ctrl_c();
    key.action = KeyAction::Release;
    assert!(owner.encode_key(key.clone()).unwrap().is_empty());
    owner.feed(b"\x1b[>3u").unwrap();
    assert_eq!(owner.encode_key(ctrl_c()).unwrap(), b"\x1b[99;5u");
    assert_eq!(owner.encode_key(key.clone()).unwrap(), b"\x1b[99;5:3u");
    key.action = KeyAction::Repeat;
    assert_eq!(owner.encode_key(key.clone()).unwrap(), b"\x1b[99;5:2u");
    owner.feed(b"\x1b[?1049h").unwrap();
    assert_eq!(owner.encode_key(ctrl_c()).unwrap(), b"\x03");
    owner.feed(b"\x1b[?1049l").unwrap();
    assert_eq!(owner.encode_key(key).unwrap(), b"\x1b[99;5:2u");
    owner.close().unwrap();
}

#[test]
fn mouse_reads_native_tracking_and_pixel_format_without_legacy_modes() {
    let mut owner = EngineOwner::new(12, 3).unwrap();
    assert!(owner.encode_mouse(mouse()).unwrap().is_empty());
    owner.feed(b"\x1b[?1000;1006h").unwrap();
    assert_eq!(owner.encode_mouse(mouse()).unwrap(), b"\x1b[<0;3;2M");
    let mut released = mouse();
    released.action = MouseAction::Release;
    released.any_button_pressed = false;
    assert_eq!(owner.encode_mouse(released).unwrap(), b"\x1b[<0;3;2m");
    let mut motion = mouse();
    motion.action = MouseAction::Motion;
    motion.button = None;
    motion.any_button_pressed = false;
    assert!(owner.encode_mouse(motion).unwrap().is_empty());
    owner.feed(b"\x1b[?1003h").unwrap();
    assert_eq!(owner.encode_mouse(motion).unwrap(), b"\x1b[<35;3;2M");
    owner.feed(b"\x1b[?1016h").unwrap();
    // The pinned encoder emits terminal-space pixels without the cell mode's
    // one-based offset (src/input/mouse_encode.zig::posToPixels).
    assert_eq!(owner.encode_mouse(mouse()).unwrap(), b"\x1b[<0;25;21M");
    owner.feed(b"\x1bc").unwrap();
    assert!(owner.encode_mouse(mouse()).unwrap().is_empty());
    owner.close().unwrap();
}

#[test]
fn invalid_input_is_rejected_before_worker_mutation_and_publication() {
    let mut owner = EngineOwner::new(12, 3).unwrap();
    let mut reader = owner.reader();
    let before = owner.capture().unwrap();
    reader.poll().unwrap();
    let mut key = ctrl_c();
    key.text = "x".repeat(MAX_KEY_TEXT_BYTES + 1);
    assert!(matches!(
        owner.encode_key(key),
        Err(crate::ghostty::owner::Error::Native(Error::InputLimit))
    ));
    let mut key = ctrl_c();
    key.text = "\x1b[201~".into();
    assert!(owner.encode_key(key).is_err());
    assert!(KeyCode::from_code(176).is_err());
    for value in [f32::NAN, f32::INFINITY, -1.0, 121.0] {
        let mut event = mouse();
        event.x = value;
        assert!(owner.encode_mouse(event).is_err());
    }
    let mut event = mouse();
    event.geometry.cell_width = 0;
    assert!(owner.encode_mouse(event).is_err());
    event = mouse();
    event.geometry.padding_left = u32::MAX;
    assert!(owner.encode_mouse(event).is_err());
    assert_eq!(reader.poll().unwrap().state, State::Running);
    assert_eq!(reader.poll().unwrap().damage, Damage::None);
    assert_eq!(*before, *owner.capture().unwrap());
    assert_eq!(owner.encode_key(ctrl_c()).unwrap(), b"\x03");
    owner.close().unwrap();
}

#[test]
fn unicode_and_composition_borrows_end_without_changing_terminal_state() {
    let mut terminal = Terminal::new(12, 3).unwrap();
    let before = RenderBuffer::new().unwrap().capture(&mut terminal).unwrap();
    let mut key = KeyEvent::new(KeyCode::UNIDENTIFIED);
    key.text = "日e\u{301}".into();
    let bytes = terminal.encode_key(&key).unwrap();
    assert_eq!(bytes, key.text.as_bytes());
    key.text.clear();
    assert_eq!(bytes, "日e\u{301}".as_bytes());
    key.composing = true;
    assert!(terminal.encode_key(&key).unwrap().is_empty());
    assert_eq!(
        before,
        RenderBuffer::new().unwrap().capture(&mut terminal).unwrap()
    );
}

#[test]
fn input_layouts_match_the_pinned_c_guards() {
    assert_eq!(size_of::<Text>(), 16);
    assert_eq!(align_of::<Text>(), 8);
    assert_eq!(std::mem::offset_of!(Text, len), 8);
    assert_eq!(size_of::<Position>(), 8);
    assert_eq!(align_of::<Position>(), 4);
    assert_eq!(size_of::<NativeGeometry>(), 40);
    assert_eq!(align_of::<NativeGeometry>(), 8);
    assert_eq!(std::mem::offset_of!(NativeGeometry, width), 8);
    assert_eq!(std::mem::offset_of!(NativeGeometry, padding_left), 36);
}

#[test]
fn geometry_bounds_prevent_native_float_to_integer_overflow() {
    let mut terminal = Terminal::new(12, 3).unwrap();
    terminal.write(b"\x1b[?1000;1016h").unwrap();
    for width in [65536, i32::MAX as u32, u32::MAX] {
        let mut event = mouse();
        event.geometry.width = width;
        event.geometry.cell_width = 1;
        assert!(matches!(
            terminal.encode_mouse(&event),
            Err(Error::InvalidInput(_))
        ));
    }
}

#[test]
fn bounded_encoder_discards_partial_or_impossible_native_results() {
    unsafe extern "C" fn out_of_space(
        _: *mut c_void,
        _: *mut c_void,
        out: *mut u8,
        _: usize,
        written: *mut usize,
    ) -> c_int {
        // SAFETY: encode supplies a nonempty writable buffer and length slot.
        unsafe {
            *out = 0x1b;
            *written = MAX_ENCODED_BYTES + 1;
        }
        -3
    }
    unsafe extern "C" fn invalid_length(
        _: *mut c_void,
        _: *mut c_void,
        _: *mut u8,
        _: usize,
        written: *mut usize,
    ) -> c_int {
        // SAFETY: encode supplies a writable length slot.
        unsafe {
            *written = MAX_ENCODED_BYTES + 1;
        }
        0
    }
    let encoder = Handle::new(ghostty_key_encoder_new, ghostty_key_encoder_free).unwrap();
    let event = Handle::new(ghostty_key_event_new, ghostty_key_event_free).unwrap();
    assert!(matches!(
        encode(&encoder, &event, out_of_space),
        Err(Error::InputLimit)
    ));
    assert!(matches!(
        encode(&encoder, &event, invalid_length),
        Err(Error::InputLimit)
    ));
}
