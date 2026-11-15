use super::*;
use crate::ghostty::owner::{Damage, EngineOwner, Error as OwnerError, State};

fn complete(owner: &mut EngineOwner, mut result: Snapshot) -> Snapshot {
    for _ in 0..1000 {
        if result.status == Status::Complete {
            return result;
        }
        result = owner.search_step().unwrap();
    }
    panic!("search fixture did not complete");
}

#[test]
fn native_search_returns_owned_unicode_and_ascii_insensitive_screen_matches() {
    let mut owner = EngineOwner::new(20, 3).unwrap();
    owner.feed("Abé abé\r\nlast abé".as_bytes()).unwrap();
    let first = owner.search("abé".into(), Options::default()).unwrap();
    let found = complete(&mut owner, first);
    assert_eq!(found.matches.len(), 3);
    assert!(!found.alternate_screen);
    assert_eq!(
        found.matches[0],
        Match {
            start: Coordinate { column: 5, row: 1 },
            end: Coordinate { column: 7, row: 1 }
        }
    );
    assert_eq!(owner.selection_text().unwrap(), None);
    owner.feed(b"\x1bc").unwrap();
    assert!(owner.search_step().unwrap().matches.is_empty());
    owner.close().unwrap();
    assert_eq!(found.matches.len(), 3);
}

#[test]
fn search_tracks_history_reflow_and_screen_switches_without_publication() {
    let mut owner = EngineOwner::new(12, 3).unwrap();
    owner.feed(b"needle\r\none\r\ntwo\r\nthree").unwrap();
    owner.capture().unwrap();
    let mut reader = owner.reader();
    reader.poll().unwrap();
    let first = owner.search("needle".into(), Options::default()).unwrap();
    let found = complete(&mut owner, first);
    assert_eq!(found.matches.len(), 1);
    assert_eq!(found.matches[0].start.row, 0);
    assert_eq!(reader.poll().unwrap().damage, Damage::None);
    owner.resize(4, 3, 10, 20).unwrap();
    let first = owner.search_step().unwrap();
    let reflow = complete(&mut owner, first);
    assert_eq!(reflow.matches.len(), 1);
    owner.feed(b"\x1b[?1049h\x1b[Hneedle").unwrap();
    let first = owner.search_step().unwrap();
    let alt = complete(&mut owner, first);
    assert!(alt.alternate_screen);
    assert_eq!(alt.matches.len(), 1);
    owner.feed(b"\x1b[?1049l\x1b[3J").unwrap();
    let first = owner.search_step().unwrap();
    assert!(complete(&mut owner, first).matches.is_empty());
    owner.close().unwrap();
}

#[test]
fn search_options_and_limits_are_recoverable_and_clear_releases_results() {
    let mut owner = EngineOwner::new(100, 50).unwrap();
    owner.feed(&vec![b'x'; 5000]).unwrap();
    assert!(matches!(
        owner.search("x".repeat(MAX_NEEDLE_BYTES + 1), Options::default()),
        Err(OwnerError::Search(SearchError::NeedleLimit))
    ));
    for options in [
        Options {
            case_sensitive: true,
            whole_word: false,
        },
        Options {
            case_sensitive: false,
            whole_word: true,
        },
    ] {
        assert!(matches!(
            owner.search("x".into(), options),
            Err(OwnerError::Search(SearchError::UnsupportedOptions))
        ));
    }
    assert!(matches!(
        owner.search("x".into(), Options::default()),
        Err(OwnerError::Search(SearchError::MatchLimit))
    ));
    let cleared = owner.search(String::new(), Options::default()).unwrap();
    assert_eq!(cleared.status, Status::Complete);
    assert!(cleared.matches.is_empty());
    assert_eq!(owner.reader().poll().unwrap().state, State::Running);
    owner.close().unwrap();
}
