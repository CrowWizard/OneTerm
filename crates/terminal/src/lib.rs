//! The terminal adapter: `oneterm-vt` behind `TerminalSession`.
//!
//! Since `US-0082` this crate speaks the engine's vocabulary — `RowId`,
//! `SnapshotState`, `EventBatch`, `ColorKey`, `ResizePolicy` — and owns the two
//! things the engine deliberately does not: the lock ([`TerminalHandle`]) and
//! the event-delivery policy ([`backend`]).
//!
//! Since `US-0085` the compatibility surface is gone with it: nothing this
//! crate publishes names a forked-engine type any more, and `US-0087` deleted
//! the fork itself along with the old-versus-new differential that was its
//! last consumer. No GPUI here either way.

// Modules nothing outside names by path are the crate's own business; the items
// other crates use are re-exported below (`US-0090`).
pub mod backend;
pub(crate) mod color_classification;
pub(crate) mod content;
pub(crate) mod factory;
#[cfg(feature = "ghostty-native")]
#[doc(hidden)]
pub mod ghostty;
#[cfg(feature = "ghostty-native")]
#[path = "ghostty/bridge.rs"]
pub mod ghostty_bridge;
#[cfg(feature = "ghostty-native")]
#[path = "ghostty/content.rs"]
pub mod ghostty_content;
#[cfg(feature = "ghostty-native")]
#[path = "ghostty/ime.rs"]
pub mod ghostty_ime;
#[cfg(feature = "ghostty-native")]
#[path = "ghostty/shared_model.rs"]
pub mod ghostty_model;
#[cfg(feature = "ghostty-native")]
#[path = "ghostty/project.rs"]
pub mod ghostty_projection;
#[cfg(feature = "ghostty-native")]
#[path = "ghostty/session.rs"]
pub mod ghostty_session;
pub mod handle;
pub mod logging;
pub mod model;
pub(crate) mod osc;
pub(crate) mod osc_color;
pub(crate) mod palette;
pub(crate) mod paste;
pub(crate) mod program_status;
pub(crate) mod render_row;
pub mod security_policy;
pub mod session;
#[cfg(test)]
pub(crate) mod test_engine;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod url_policy;

pub use backend::{
    ByteBudget, GridSize, OscRouter, PtyTransport, SessionEventSink, SharedSessionState,
    SharedState, TerminalPump,
};
pub use color_classification::is_decorative_character;
pub use content::{ContentCell, LineRangeCells, TerminalContent, last_content_row};
pub use factory::{CreatedSession, PtySize, SessionFactory};
pub use handle::{DEFAULT_SCROLLBACK_LINES, SharedTerminal, TerminalHandle, new_shared_terminal};
pub use logging::{
    TerminalLogController, TerminalLogError, TerminalLogState, local_log_identity, ssh_log_identity,
};
/// The input encoders are the engine's (`US-0099`): the bytes a key press or a
/// mouse click sends depend on DECCKM and on `? 1005` / `? 1006`, which only
/// the engine knows. Re-exported here **for one release**, so the move did not
/// have to touch every consumer at once; name them from `oneterm_vt::input`.
pub use oneterm_vt::input::{
    KeyEvent, KeyEventKind, KeyMods, KeySpec, MouseModifiers, NamedKey, TerminalMouseButton,
    encode_key, encode_key_event, encode_mouse_move, encode_mouse_press, encode_mouse_release,
    encode_wheel_event,
};
pub use oneterm_vt::search::{SearchMatch, SearchOptions};
/// The engine vocabulary this crate's own API speaks, re-exported so a consumer
/// can name what [`TerminalContent`]'s native accessors return without taking a
/// direct dependency on `oneterm-vt` first (`US-0085`).
pub use oneterm_vt::{
    Attrs, CellWidth, Color, CursorShape, GraphicData, GraphicId, HyperlinkId, KeyboardFlags,
    ModeSnapshot, MouseEncoding, MouseProtocol, MouseReporting, NamedColor,
    Progress as TerminalProgress, ResizePolicy, Rgb, RowId, SelectionKind, SelectionRange,
    Semantic, SeqNo, ShellMark, Size, SnapshotCell, SnapshotContent, SnapshotCursor,
    SnapshotPlacement, SnapshotRow, SnapshotUpdate, Style, StyleRun, Terminal,
};
pub use osc::encode_osc52;
pub use osc_color::DynamicColors;
pub use palette::{TerminalPalette, resolve_color};
pub use program_status::ProgramState;
pub use render_row::{RenderRow, RenderRowKey};
pub use security_policy::{ClipboardOrigin, TerminalSecurityPolicy};
pub use session::{
    NetStats, PtyOwner, PtySession, SessionEvent, SessionKind, TerminalCapabilities, TerminalError,
    TerminalIme, TerminalInfo, TerminalInput, TerminalLifecycle, TerminalQueryState,
    TerminalRender, TerminalSession, report_generated_input,
};
pub use url_policy::TargetDecision;
