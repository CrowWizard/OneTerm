//! Native render/input contract. Not the legacy history/query session facade.

use crate::TerminalError;
use crate::ghostty::{
    Error as NativeError,
    input::{KeyEvent, MouseEvent},
};
use crate::ghostty_content::{Query, RenderSource};

/// Native destruction observation, not an OS-thread join.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Completion {
    pub exit_code: Option<i32>,
    pub lifecycle_delivered: bool,
}

/// Owned completion can be awaited away from the foreground executor.
pub type CompletionFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<Completion, TerminalError>> + Send>>;

/// Native panes retain this contract instead of the legacy render facade.
/// The caller owns event draining and must close, then release every other
/// control/view reference before consuming the sole completion reference.
pub trait NativeSession: SurfaceSession + crate::TerminalLifecycle + 'static {
    /// Optional services without constructing a legacy terminal facade.
    fn capabilities(&self) -> crate::TerminalCapabilities {
        crate::TerminalCapabilities::default()
    }

    /// Request orderly pane shutdown where supported. Implementations may seal
    /// admission and finish accepted work; hard close/cancellation stays separate.
    /// This is not completion and the caller must retain its existing deadline.
    fn request_shutdown(&self) -> Result<(), TerminalError> {
        self.close()
    }

    /// Reply only to this session's admitted OSC 52 request. No paste or fan-out.
    fn reply_clipboard(&self, text: &str) -> Result<(), TerminalError>;
    fn complete(self: std::sync::Arc<Self>) -> CompletionFuture;
}

/// Bound clipboard data before allocating the encoded protocol response.
pub fn clipboard_reply(text: &str) -> Result<Vec<u8>, TerminalError> {
    crate::TerminalSecurityPolicy::default()
        .validate_clipboard_write(text, crate::security_policy::ClipboardOrigin::Local)
        .ok_or_else(|| {
            TerminalError::Transport("Native clipboard reply exceeds policy limit".into())
        })?;
    Ok(format!("\x1b]52;c;{}\x07", crate::osc::encode_osc52(text)).into_bytes())
}

#[cfg(test)]
mod clipboard_tests {
    use super::clipboard_reply;

    #[test]
    fn native_clipboard_reply_encodes_without_paste_and_bounds_allocation() {
        assert_eq!(clipboard_reply("hi").unwrap(), b"\x1b]52;c;aGk=\x07");
        assert_eq!(clipboard_reply("").unwrap(), b"\x1b]52;c;\x07");
        assert!(clipboard_reply(&"x".repeat(256 * 1024)).is_ok());
        assert!(clipboard_reply(&"x".repeat(256 * 1024 + 1)).is_err());
    }
}

/// Borrowed object-safe operation; polling never blocks on the native worker.
pub type PasteFuture<'a> = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<(), crate::backend::AsyncGhosttyPumpError>> + 'a>,
>;
/// Palette publication must finish while the foreground executor is shutting down.
pub type ColorFuture<'a> = std::pin::Pin<
    Box<
        dyn std::future::Future<Output = Result<(), crate::backend::AsyncGhosttyPumpError>>
            + Send
            + 'a,
    >,
>;
/// Origin-encoded bytes, acknowledged only after the original FIFO accepts them.
pub type KeyReceiptFuture<'a> = std::pin::Pin<
    Box<
        dyn std::future::Future<Output = Result<Vec<u8>, crate::backend::AsyncGhosttyPumpError>>
            + 'a,
    >,
>;
pub type SearchFuture<'a> = std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<
                    crate::ghostty::search::Snapshot,
                    crate::backend::AsyncGhosttyPumpError,
                >,
            > + Send
            + 'a,
    >,
>;

/// Policy-only check for UI and endpoint admission; no native call or copy.
pub fn validate_paste(text: &str) -> Result<(), crate::session::PasteError> {
    validate_input_size(text.len())
}

/// Shared allocation bound for exact bytes and clipboard payloads.
pub fn validate_input_size(bytes: usize) -> Result<(), crate::session::PasteError> {
    let max_bytes = crate::paste::PastePolicy::default().max_bytes;
    if bytes > max_bytes {
        Err(crate::session::PasteError::TooLarge { bytes, max_bytes })
    } else {
        Ok(())
    }
}

#[derive(Debug)]
pub enum InputError {
    Invalid(NativeError),
    Admission(TerminalError),
}

impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(error) => error.fmt(f),
            Self::Admission(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for InputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Invalid(error) => Some(error),
            Self::Admission(error) => Some(error),
        }
    }
}

/// Cell/grid metrics are one FIFO control, including DPI-only changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
    pub cell_width_px: u16,
    pub cell_height_px: u16,
}

/// The embeddable native surface needs neither legacy RowId/modes nor a second
/// parser. The caller owns lifecycle and the reliable SessionEvent consumer.
/// Keys/resize acknowledge bounded control admission, not encoding or receipt.
pub trait SurfaceSession: RenderSource {
    /// Reveal and write exact bytes, without paste filtering or encoding.
    fn write_visible<'a>(&'a self, _bytes: &'a [u8]) -> PasteFuture<'a> {
        Box::pin(async {
            Err(crate::backend::AsyncGhosttyPumpError::Transport(
                TerminalError::Transport(
                    "Native raw input is not supported by this surface".into(),
                ),
            ))
        })
    }
    /// Send exactly clear CR to this shell, then publish cleared selection.
    /// Acknowledges input admission/publication, not shell execution or paint.
    fn clear_command(&self) -> PasteFuture<'_>;
    /// Acknowledge complete native publication; OSC overrides remain intact.
    /// An admitted future must be awaited even if its UI recipient is released.
    fn set_default_colors(&self, colors: crate::ghostty::colors::DefaultColors) -> ColorFuture<'_>;
    fn query(&self) -> Query {
        self.render_model().query()
    }
    fn resize(&self, size: Size) -> Result<(), InputError>;
    fn key(&self, key: KeyEvent) -> Result<(), InputError>;
    /// Encode and deliver once; unsupported surfaces never fabricate a receipt.
    fn key_receipt(&self, _key: KeyEvent) -> KeyReceiptFuture<'_> {
        Box::pin(async {
            Err(crate::backend::AsyncGhosttyPumpError::Transport(
                TerminalError::Transport(
                    "Native key receipts are not supported by this surface".into(),
                ),
            ))
        })
    }
    fn mouse(&self, mouse: MouseEvent) -> Result<(), InputError>;
    fn wheel(&self, wheel: crate::ghostty::wheel::WheelEvent) -> Result<(), InputError>;
    /// Filter, reveal the live viewport and enqueue through the original FIFO.
    /// Completion acknowledges admission, not physical delivery. Dropping an
    /// admitted operation retains the endpoint's fail-stop cancellation rule.
    fn paste_visible<'a>(&'a self, text: &'a str) -> PasteFuture<'a>;
    /// Native selection gesture, ordered with the endpoint's existing controls.
    fn selection(&self, update: crate::ghostty::selection::Update) -> PasteFuture<'_>;
    /// Incremental native search; response coordinates are tied to the returned grid revision.
    fn search(&self, needle: String, options: crate::ghostty::search::Options) -> SearchFuture<'_>;
    fn search_step(&self) -> SearchFuture<'_>;
    fn reveal_search(&self, navigation: crate::ghostty::search::Navigation) -> SearchFuture<'_>;
    /// Publish a viewport scroll through the backend's control FIFO.
    fn scroll_viewport(&self, scroll: crate::ghostty::state::Scroll) -> PasteFuture<'_>;
    fn release_repaint_hint(&self);
    fn set_marked_text(&self, text: String) -> Result<(), crate::ghostty_ime::Error>;
    fn marked_text(&self) -> Option<String>;
    fn clear_marked_text(&self);
    fn commit_text(&self, text: &str) -> Result<(), crate::ghostty_ime::Error>;
}
