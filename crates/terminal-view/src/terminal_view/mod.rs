//! [`TerminalView`] — the GPUI entity that shows one terminal session (local
//! or SSH) through the `render/` engine and the `input/` decision layer.
//!
//! - [`view`] — the entity, its dependencies, the events pump and lifecycle
//! - [`render`] — `Render`/`Focusable`: inputs refresh, wrapper div, overlays
//! - [`input`] — the wrapper's key / mouse / wheel listeners
//! - [`ime`] — `EntityInputHandler`
//! - [`search`], [`scrollbar`], [`gutter_timestamps`], [`completion`] — the
//!   sub-states the view owns

mod completion;
mod gutter_timestamps;
mod ime;
mod input;
mod render;
mod scrollbar;
mod search;
mod view;

#[cfg(feature = "ghostty-native")]
pub(crate) use render::cursor_shape_from_setting;
#[cfg(feature = "ghostty-native")]
pub(crate) use render::shell_kind_to_profile;
#[cfg(feature = "ghostty-native")]
pub(crate) use render::terminal_font;
#[cfg(feature = "ghostty-native")]
pub(crate) use render::{OverlayColors, bell_badge, progress_overlay};

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod view_tests;

pub(crate) use view::{BroadcastOrigin, TerminalDeps, TerminalView, TerminalViewEvent};
