//! Local shell via PTY for OneTerm.
//!
//! Uses `oneterm_vt::pty` + a custom poll loop (ConPTY on Windows) on
//! top of the shared pump layer in `oneterm_terminal::backend`. Supports
//! `cmd`/`powershell`/`pwsh`/custom shell config. See `docs/terminal-backend.md`.
//!
//! The shipping public entry point is [`LocalSession`]. A default-off native
//! staging endpoint is separately exposed for migration validation.

mod event_loop;
mod session;
mod session_terminal;
mod transport;

#[cfg(feature = "ghostty-native")]
mod ghostty_session;

#[cfg(feature = "ghostty-native")]
#[doc(hidden)]
pub use ghostty_session::NativeLocalSession;

#[cfg(test)]
mod spawn_guard;

pub use session::LocalSession;

/// Explicit experimental endpoint; not used by LocalSession or SessionFactory.
#[cfg(feature = "ghostty-native")]
#[doc(hidden)]
pub use event_loop::ghostty_local;
