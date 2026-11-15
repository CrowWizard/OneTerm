//! Native viewport regression fixtures, with no legacy terminal state.

use super::render::*;
use super::{Error, Terminal};

#[test]
fn graphemes_wide_cells_and_owned_lifetime() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Frame>();
    let mut terminal = Terminal::new(16, 3).unwrap();
    let mut render = RenderBuffer::new().unwrap();
    terminal
        .write("\x1b[?2027he\u{301}中👩\u{200d}💻".as_bytes())
        .unwrap();
    let frame = render.capture(&mut terminal).unwrap();
    let cells = &frame.rows[0].cells;
    assert_eq!(cells[0].text, "e\u{301}");
    assert_eq!(cells[1].text, "中");
    assert_eq!(cells[1].width, Width::Wide);
    assert_eq!(cells[2].width, Width::SpacerTail);
    assert_eq!(cells[3].text, "👩\u{200d}💻");
    assert_eq!(frame.cursor.position, Some((5, 0)));
    let expected = frame.clone();
    terminal.write(b"\x1b[2Jchanged").unwrap();
    terminal.resize(8, 2, 9, 18).unwrap();
    assert_eq!(render.capture(&mut terminal).unwrap().cols, 8);
    drop(terminal);
    drop(render);
    assert_eq!(frame, expected);
}

#[test]
fn styles_preserve_color_kinds_and_all_flags() {
    let mut terminal = Terminal::new(16, 2).unwrap();
    terminal
        .write(b"\x1b[1;2;3;4:3;5;7;8;9;53;31;48;2;4;5;6;58;2;7;8;9mX\x1b[0mY")
        .unwrap();
    let frame = RenderBuffer::new().unwrap().capture(&mut terminal).unwrap();
    let style = frame.rows[0].cells[0].style;
    assert_eq!(style.foreground, Color::Palette(1));
    assert_eq!(style.background, Color::Rgb(Rgb { r: 4, g: 5, b: 6 }));
    assert_eq!(style.underline, Color::Rgb(Rgb { r: 7, g: 8, b: 9 }));
    assert_eq!(style.flags, 0xff | (3 << 8));
    assert_eq!(frame.rows[0].cells[1].style.flags, 0);
    assert_eq!(frame.rows[0].cells[1].style.foreground, Color::Default);
}

#[test]
fn background_only_erasure_and_palette_are_projected() {
    let mut terminal = Terminal::new(8, 2).unwrap();
    terminal
        .write(b"\x1b]4;4;rgb:11/22/33\x07\x1b[44m\x1b[2K")
        .unwrap();
    let frame = RenderBuffer::new().unwrap().capture(&mut terminal).unwrap();
    let blue = Rgb {
        r: 0x11,
        g: 0x22,
        b: 0x33,
    };
    assert_eq!(frame.palette[4], blue);
    for cell in &frame.rows[0].cells {
        assert!(cell.text.is_empty());
        assert_eq!(cell.background, Some(blue));
    }
}

#[test]
fn independent_consumers_keep_wrap_identity_and_cursor() {
    let mut terminal = Terminal::new(4, 3).unwrap();
    let mut first = RenderBuffer::new().unwrap();
    let mut second = RenderBuffer::new().unwrap();
    terminal.write(b"abcde\x1b[5 q\x1b[?25l").unwrap();
    let a = first.capture(&mut terminal).unwrap();
    let b = second.capture(&mut terminal).unwrap();
    assert_eq!(a, b);
    assert!(a.rows[0].wrapped);
    assert!(a.rows[1].wrap_continuation);
    assert_eq!(a.cursor.shape, CursorShape::Bar);
    assert!(a.cursor.blinking);
    assert!(!a.cursor.visible);
    assert_eq!(a.cursor.position, Some((1, 1)));
    assert_eq!(a, first.capture(&mut terminal).unwrap());
    terminal.write(b"f").unwrap();
    let changed = second.capture(&mut terminal).unwrap();
    assert_eq!(a.rows[1].identity, changed.rows[1].identity);
    assert_eq!(changed, first.capture(&mut terminal).unwrap());
    assert_ne!(a, changed);
}

#[test]
fn render_buffer_can_be_reused_after_terminal_destruction() {
    let mut render = RenderBuffer::new().unwrap();
    for _ in 0..32 {
        let mut terminal = Terminal::new(8, 2).unwrap();
        terminal.write(b"hello").unwrap();
        assert_eq!(
            render.capture(&mut terminal).unwrap().rows[0].cells[0].text,
            "h"
        );
    }
}

#[test]
fn every_input_split_projects_the_same_unicode_and_style_frame() {
    let stream = "\x1b[?2027h\x1b[38;2;12;34;56me\u{301}中👩\u{200d}💻\x1b[0m".as_bytes();
    let mut expected_terminal = Terminal::new(16, 2).unwrap();
    expected_terminal.write(stream).unwrap();
    let expected = RenderBuffer::new()
        .unwrap()
        .capture(&mut expected_terminal)
        .unwrap();
    for split in 0..=stream.len() {
        let mut terminal = Terminal::new(16, 2).unwrap();
        let mut render = RenderBuffer::new().unwrap();
        terminal.write(&stream[..split]).unwrap();
        // A partial UTF-8 or escape sequence must remain parser state, not
        // leak a borrowed or malformed string through the render boundary.
        render.capture(&mut terminal).unwrap();
        terminal.write(&stream[split..]).unwrap();
        let actual = render.capture(&mut terminal).unwrap();
        assert_eq!(
            actual.rows[0].cells, expected.rows[0].cells,
            "split {split}"
        );
        assert_eq!(actual.cursor, expected.cursor, "split {split}");
    }
}

#[test]
fn explicit_default_colors_and_cursor_color_survive_capture() {
    let mut terminal = Terminal::new(8, 2).unwrap();
    terminal
        .write(b"\x1b]10;rgb:11/22/33\x07\x1b]11;rgb:44/55/66\x07\x1b]12;rgb:77/88/99\x07")
        .unwrap();
    let frame = RenderBuffer::new().unwrap().capture(&mut terminal).unwrap();
    assert_eq!(
        frame.foreground,
        Rgb {
            r: 0x11,
            g: 0x22,
            b: 0x33
        }
    );
    assert_eq!(
        frame.background,
        Rgb {
            r: 0x44,
            g: 0x55,
            b: 0x66
        }
    );
    assert_eq!(
        frame.cursor_color,
        Some(Rgb {
            r: 0x77,
            g: 0x88,
            b: 0x99
        })
    );
}

#[test]
fn alternate_screen_does_not_overwrite_an_owned_primary_frame() {
    let mut terminal = Terminal::new(8, 2).unwrap();
    let mut render = RenderBuffer::new().unwrap();
    terminal.write(b"primary").unwrap();
    let primary = render.capture(&mut terminal).unwrap();
    terminal.write(b"\x1b[?1049h\x1b[Halt").unwrap();
    assert_eq!(
        render.capture(&mut terminal).unwrap().rows[0].cells[0].text,
        "a"
    );
    terminal.write(b"\x1b[?1049l").unwrap();
    let restored = render.capture(&mut terminal).unwrap();
    assert_eq!(primary.rows[0].cells, restored.rows[0].cells);
    assert_eq!(primary.cursor, restored.cursor);
}

#[test]
fn text_budget_rejection_does_not_consume_the_cell() {
    let mut terminal = Terminal::new(8, 2).unwrap();
    terminal.write(b"x").unwrap();
    let mut render = RenderBuffer::new().unwrap();
    assert!(render.capture(&mut terminal).is_ok());
    assert!(matches!(
        super::render::test_text_budget(&mut render, &mut terminal),
        Err(Error::SnapshotLimit)
    ));
    assert_eq!(
        render.capture(&mut terminal).unwrap().rows[0].cells[0].text,
        "x"
    );
}
