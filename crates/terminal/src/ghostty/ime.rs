//! Session-owned composition, independent of native terminal state and painting.

use parking_lot::Mutex;

use crate::TerminalError;

pub const MAX_TEXT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    TextLimit,
    Input(TerminalError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TextLimit => f.write_str("IME text exceeds 1 MiB UTF-8"),
            Self::Input(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

/// Preedit is not parser input. Callers arrange their own redraw after edits.
#[derive(Default)]
pub struct Composition {
    marked: Mutex<Option<String>>,
}

impl Composition {
    pub fn set_marked_text(&self, text: String) -> Result<(), Error> {
        validate(&text)?;
        *self.marked.lock() = Some(text);
        Ok(())
    }

    pub fn marked_text(&self) -> Option<String> {
        self.marked.lock().clone()
    }

    pub fn clear_marked_text(&self) {
        *self.marked.lock() = None;
    }

    /// Validate first, then clear before one original-FIFO write. Even rejected
    /// admission leaves preedit cleared; callers must not retry implicitly.
    /// The lock is released before calling transport code.
    pub fn commit_text(
        &self,
        text: &str,
        write: impl FnOnce(&[u8]) -> Result<(), TerminalError>,
    ) -> Result<(), Error> {
        validate(text)?;
        self.clear_marked_text();
        write(text.as_bytes()).map_err(Error::Input)
    }
}

fn validate(text: &str) -> Result<(), Error> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(Error::TextLimit);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_is_owned_replaceable_and_clearable() {
        let composition = Composition::default();
        assert_eq!(composition.marked_text(), None);
        composition
            .set_marked_text("e\u{301}日本語".into())
            .unwrap();
        let snapshot = composition.marked_text().unwrap();
        composition.set_marked_text(String::new()).unwrap();
        assert_eq!(composition.marked_text().as_deref(), Some(""));
        composition.clear_marked_text();
        assert_eq!(composition.marked_text(), None);
        assert_eq!(snapshot, "e\u{301}日本語");
        assert_eq!(Composition::default().marked_text(), None);
    }

    #[test]
    fn exact_commit_clears_before_one_write_without_holding_composition_lock() {
        let composition = Composition::default();
        composition.set_marked_text("preedit".into()).unwrap();
        let input = "e\u{301}日本語\n\r\x1b\x03";
        let mut calls = 0;
        composition
            .commit_text(input, |bytes| {
                calls += 1;
                assert_eq!(composition.marked_text(), None);
                assert_eq!(bytes, input.as_bytes());
                Ok(())
            })
            .unwrap();
        assert_eq!(calls, 1);
        composition
            .commit_text("", |bytes| {
                assert!(bytes.is_empty());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn byte_limit_rejects_before_mutation_or_transport() {
        let composition = Composition::default();
        let limit = "é".repeat(MAX_TEXT_BYTES / 2);
        composition.set_marked_text(limit.clone()).unwrap();
        let over = format!("{limit}a");
        assert_eq!(
            composition.set_marked_text(over.clone()),
            Err(Error::TextLimit)
        );
        assert_eq!(
            composition.commit_text(&over, |_| panic!("oversize write")),
            Err(Error::TextLimit)
        );
        assert_eq!(composition.marked_text().as_deref(), Some(limit.as_str()));
        composition
            .commit_text(&limit, |bytes| {
                assert_eq!(bytes.len(), MAX_TEXT_BYTES);
                Ok(())
            })
            .unwrap();
        assert_eq!(composition.marked_text(), None);
    }

    #[test]
    fn failed_admission_clears_without_retry_or_restoration() {
        let composition = Composition::default();
        for error in [
            TerminalError::QueueFull,
            TerminalError::Closed,
            TerminalError::Transport("failed".into()),
        ] {
            composition.set_marked_text("preedit".into()).unwrap();
            let mut calls = 0;
            assert_eq!(
                composition.commit_text("commit", |_| {
                    calls += 1;
                    assert_eq!(composition.marked_text(), None);
                    Err(error.clone())
                }),
                Err(Error::Input(error))
            );
            assert_eq!(calls, 1);
            assert_eq!(composition.marked_text(), None);
        }
    }
}
