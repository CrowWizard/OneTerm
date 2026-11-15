use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=GHOSTTY_VT_PREFIX");
    if env::var_os("CARGO_FEATURE_GHOSTTY_NATIVE").is_none() {
        return;
    }
    assert_eq!(
        env::var("TARGET").expect("Cargo supplies TARGET"),
        "x86_64-pc-windows-msvc",
        "ghostty-native currently validates only the Windows x86_64 MSVC artifact"
    );
    let root =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo supplies manifest dir"))
            .join("../..")
            .canonicalize()
            .expect("workspace root exists");
    let prefix = PathBuf::from(env::var_os("GHOSTTY_VT_PREFIX").expect(
        "ghostty-native requires GHOSTTY_VT_PREFIX; first run scripts/ghostty-vt/build.ps1",
    ))
    .canonicalize()
    .expect("GHOSTTY_VT_PREFIX must point to an existing native install");
    assert!(
        prefix.starts_with(&root),
        "Ghostty artifacts must stay inside the project"
    );
    let revision = prefix.join("oneterm-revision.txt");
    let library = prefix.join("lib/ghostty-vt-static.lib");
    println!("cargo:rerun-if-changed={}", revision.display());
    println!("cargo:rerun-if-changed={}", library.display());
    assert_eq!(
        fs::read_to_string(&revision)
            .expect("native install must include OneTerm build provenance")
            .trim(),
        "fbb6dbe7ea5c1ce6df14ef82e00779a568cdb2a2",
        "Ghostty library revision does not match the staged bindings"
    );
    assert!(
        library.is_file(),
        "native install must include ghostty-vt-static.lib"
    );
    println!(
        "cargo:rustc-link-search=native={}",
        prefix.join("lib").display()
    );
    println!("cargo:rustc-link-lib=static=ghostty-vt-static");
    println!("cargo:rustc-link-lib=ntdll");
    println!("cargo:rustc-link-lib=kernel32");
}
