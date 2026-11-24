//! The release notes the app shows after an update (`release_notes.rs`): docs/releases/*.md as JSON, made by
//! tools/release_notes.py, which the macOS build uses too.

use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let root = manifest.join("../../../..");
    let script = root.join("tools/release_notes.py");
    let notes = root.join("docs/releases");
    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("release-notes.json");
    println!("cargo:rerun-if-changed={}", script.display());
    println!("cargo:rerun-if-changed={}", notes.display());
    for entry in fs::read_dir(&notes).expect("docs/releases").flatten() {
        println!("cargo:rerun-if-changed={}", entry.path().display());
    }
    let status = Command::new("python3").arg("-I").arg(&script).arg(&out).status().expect("python3 runs");
    assert!(status.success(), "{}: {status}", script.display());
}
