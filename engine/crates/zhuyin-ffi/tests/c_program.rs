//! Builds `tests/c/smoke.c` against the checked-in header and the shared library cargo built for this test run,
//! and runs it.  Skipped (with a message) when no C compiler `cc` is on PATH.  Also checks that the header declares
//! every exported function, so a change to the ABI without regenerating it fails here.
#![allow(missing_docs)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn header_declares_every_function() {
    let header = std::fs::read_to_string(crate_dir().join("include/zhuyin.h")).unwrap();
    let mut missing = vec![];
    for file in std::fs::read_dir(crate_dir().join("src")).unwrap() {
        let source = std::fs::read_to_string(file.unwrap().path()).unwrap();
        for line in source.lines() {
            let line = line.trim_start();
            let Some(rest) =
                line.strip_prefix("pub unsafe extern \"C\" fn ").or(line.strip_prefix("pub extern \"C\" fn "))
            else {
                continue;
            };
            let name = rest.split('(').next().unwrap();
            let declared = header
                .match_indices(&format!("{name}("))
                .any(|(at, _)| header[..at].chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_')));
            if !declared {
                missing.push(name.to_owned());
            }
        }
    }
    assert!(missing.is_empty(), "include/zhuyin.h is out of date (run cbindgen, see cbindgen.toml): {missing:?}");
}

/// The directory with the library cargo built alongside this test (`target/<profile>/deps`).
fn library_dir() -> PathBuf {
    std::env::current_exe().unwrap().parent().unwrap().to_path_buf()
}

#[test]
fn c_program() {
    if Command::new("cc").arg("--version").output().is_err() {
        eprintln!("skipped: no C compiler");
        return;
    }
    let deps = library_dir();
    let library = ["libzhuyin.dylib", "libzhuyin.so"].iter().map(|n| deps.join(n)).find(|p| p.exists());
    let Some(library) = library else {
        eprintln!("skipped: no shared library in {}", deps.display());
        return;
    };
    let output = std::env::temp_dir().join(format!("zhuyin-smoke-{}", std::process::id()));
    let status = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-o"])
        .arg(&output)
        .arg(crate_dir().join("tests/c/smoke.c"))
        .arg("-I")
        .arg(crate_dir().join("include"))
        .arg(&library)
        .arg(format!("-Wl,-rpath,{}", deps.display()))
        .status()
        .unwrap();
    assert!(status.success(), "the C program does not compile");
    let run = Command::new(&output).output().unwrap();
    let _ = std::fs::remove_file(&output);
    assert!(run.status.success(), "{}{}", String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&run.stderr));
    assert!(Path::new(&library).exists());
}
