//! Terminal configuration (local shell, …).
//!
//! Pure logic, no GPUI dependency.

pub mod elevation;
mod env;
#[cfg(feature = "isolated-config")]
pub mod isolated;
pub mod shell;

pub use shell::{LocalShellConfig, ShellKind, config_dir, home_dir, resolve_shell};
