//! Explicit process-lifetime storage selection for isolated acceptance entries.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static ROOT: Mutex<Option<PathBuf>> = Mutex::new(None);
static DEFAULT: OnceLock<PathBuf> = OnceLock::new();

/// Prepare one sibling directory without following a linked parent, then select
/// it before any configuration access. Never falls back to the default root.
pub fn prepare_root(path: &Path) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "isolated root has no parent")
    })?;
    validate_root(parent)?;
    match std::fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    select_root(path)
}

/// Select an absolute existing directory before any configuration is read.
/// Repeated or late selection fails, including selection of the same path.
/// The caller prepares the directory; this function never imports user data.
pub fn select_root(path: &Path) -> io::Result<()> {
    let mut root = ROOT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if root.is_some() || DEFAULT.get().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "configuration root already selected or used",
        ));
    }
    *root = Some(validate_root(path)?);
    Ok(())
}

fn validate_root(path: &Path) -> io::Result<PathBuf> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "isolated configuration root must be absolute",
        ));
    }
    for ancestor in path.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor)?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "isolated configuration root cannot traverse links",
            ));
        }
    }
    if !path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            "isolated configuration root is not a directory",
        ));
    }
    path.canonicalize()
}

fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

pub(super) fn config_dir() -> PathBuf {
    let root = ROOT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    root.clone().unwrap_or_else(|| {
        DEFAULT
            .get_or_init(super::shell::default_config_dir)
            .clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_root_rejects_relative_missing_and_file_paths() {
        assert_eq!(
            validate_root(Path::new("relative")).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(validate_root(&manifest.join("missing-isolated-config-root")).is_err());
        assert!(validate_root(&manifest.join("Cargo.toml")).is_err());
        assert_eq!(
            validate_root(manifest).unwrap(),
            manifest.canonicalize().unwrap()
        );
    }

    #[test]
    fn isolated_root_process_proof() {
        const MODE: &str = "ONETERM_ISOLATED_ROOT_PROOF";
        if let Ok(mode) = std::env::var(MODE) {
            let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
            if mode == "selected" {
                select_root(manifest).unwrap();
                assert_eq!(crate::config_dir(), manifest.canonicalize().unwrap());
                assert_eq!(
                    select_root(manifest).unwrap_err().kind(),
                    io::ErrorKind::AlreadyExists
                );
            } else {
                assert_eq!(
                    crate::config_dir(),
                    super::super::shell::default_config_dir()
                );
                assert_eq!(
                    select_root(manifest).unwrap_err().kind(),
                    io::ErrorKind::AlreadyExists
                );
            }
            return;
        }
        for mode in ["selected", "default"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "config::isolated::tests::isolated_root_process_proof",
                ])
                .env(MODE, mode)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{mode}: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
