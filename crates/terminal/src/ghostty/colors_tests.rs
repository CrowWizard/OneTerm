use super::*;
use crate::ghostty::owner::{Damage, EngineOwner};

fn defaults(seed: u8) -> Box<DefaultColors> {
    Box::new(DefaultColors {
        foreground: Rgb {
            r: seed,
            g: 34,
            b: 51,
        },
        background: Rgb {
            r: 4,
            g: seed,
            b: 6,
        },
        cursor: Some(Rgb {
            r: 7,
            g: 8,
            b: seed,
        }),
        palette: std::array::from_fn(|i| Rgb {
            r: i as u8,
            g: seed,
            b: 9,
        }),
    })
}

#[test]
fn defaults_preserve_osc_overrides_until_reset_and_retain_owned_frames() {
    let mut owner = EngineOwner::new(16, 3).unwrap();
    let first = defaults(17);
    owner.set_default_colors(first.clone()).unwrap();
    owner.feed(b"A\x1b[31mB\x1b[0mC").unwrap();
    let initial = owner.capture().unwrap();
    assert_eq!(initial.foreground, first.foreground);
    assert_eq!(initial.background, first.background);
    assert_eq!(initial.cursor_color, first.cursor);
    assert_eq!(initial.palette, first.palette);
    let mut reader = owner.reader();
    reader.poll().unwrap();
    owner
        .feed(b"\x1b]10;#aabbcc\x07\x1b]11;#112233\x07\x1b]12;#445566\x07\x1b]4;1;#778899\x07")
        .unwrap();
    let second = defaults(68);
    owner.set_default_colors(second.clone()).unwrap();
    let overridden = owner.capture().unwrap();
    assert_eq!(
        overridden.foreground,
        Rgb {
            r: 170,
            g: 187,
            b: 204
        }
    );
    assert_eq!(
        overridden.background,
        Rgb {
            r: 17,
            g: 34,
            b: 51
        }
    );
    assert_eq!(
        overridden.cursor_color,
        Some(Rgb {
            r: 68,
            g: 85,
            b: 102
        })
    );
    assert_eq!(
        overridden.palette[1],
        Rgb {
            r: 119,
            g: 136,
            b: 153
        }
    );
    assert_eq!(overridden.palette[2], second.palette[2]);
    assert_eq!(reader.poll().unwrap().damage, Damage::Full);
    owner
        .feed(b"\x1b]110\x07\x1b]111\x07\x1b]112\x07\x1b]104;1\x07")
        .unwrap();
    let reset = owner.capture().unwrap();
    assert_eq!(reset.foreground, second.foreground);
    assert_eq!(reset.background, second.background);
    assert_eq!(reset.cursor_color, second.cursor);
    assert_eq!(reset.palette, second.palette);
    assert_eq!(initial.foreground, first.foreground);
    assert_eq!(initial.palette, first.palette);
    let mut no_cursor = defaults(85);
    no_cursor.cursor = None;
    owner.set_default_colors(no_cursor).unwrap();
    assert_eq!(owner.capture().unwrap().cursor_color, None);
    owner.close().unwrap();
    assert_eq!(reset.palette, second.palette);
}

#[test]
fn native_color_queries_report_effective_defaults_and_override_then_reset() {
    let mut owner = EngineOwner::new(16, 3).unwrap();
    owner.set_default_colors(defaults(17)).unwrap();
    let reply = owner
        .feed(b"\x1b]10;?\x07\x1b]11;?\x07\x1b]12;?\x07\x1b]4;1;?\x07")
        .unwrap();
    assert_eq!(reply, b"\x1b]10;rgb:1111/2222/3333\x07\x1b]11;rgb:0404/1111/0606\x07\x1b]12;rgb:0707/0808/1111\x07\x1b]4;1;rgb:0101/1111/0909\x07");
    owner.feed(b"\x1b]10;#aabbcc\x07").unwrap();
    owner.set_default_colors(defaults(68)).unwrap();
    assert_eq!(
        owner.feed(b"\x1b]10;?\x1b\\").unwrap(),
        b"\x1b]10;rgb:aaaa/bbbb/cccc\x1b\\"
    );
    assert_eq!(
        owner.feed(b"\x1b]110\x07\x1b]10;?\x07").unwrap(),
        b"\x1b]10;rgb:4444/2222/3333\x07"
    );
    owner.close().unwrap();
}
