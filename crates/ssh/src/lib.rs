//! SSH + SFTP implementation for OneTerm.
//!
//! Implements a `russh` client on a shared tokio runtime. `SshSession`
//! implements `TerminalSession` — the UI uses it through the trait, unaware of
//! the internals; SFTP is reached through `TerminalCapabilities::sftp`.
//! Terminal parsing / OSC routing / event delivery come from the shared pump
//! layer in `oneterm_terminal::backend`.
//!
//! Default builds expose [`SshSession`] and [`connect`]. The opt-in
//! `ghostty-native` feature also exposes an experimental direct-channel owner,
//! not a shipping session selector. See `docs/terminal-backend.md` §7.

mod agent;
mod counting_stream;
#[cfg(feature = "ghostty-native")]
#[doc(hidden)]
pub mod ghostty_channel;
#[cfg(feature = "ghostty-native")]
mod ghostty_session;
mod handler;
#[cfg(feature = "ghostty-native")]
mod native_connect;
#[cfg(feature = "ghostty-native")]
mod native_sftp;
mod route;
mod session;
mod session_terminal;
mod sftp;
mod sftp_task;
mod task;
#[cfg(test)]
mod test_support;
mod transport;
mod tunnel;
#[cfg(test)]
mod us0095_verify_tests;

#[cfg(feature = "ghostty-native")]
pub use native_connect::connect_native;
pub use session::{SshSession, connect};
