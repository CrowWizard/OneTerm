//! Explicit native workspace acceptance entry, never the production default.

#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

fn main() -> anyhow::Result<()> {
    oneterm_app::run_native_desktop()
}
