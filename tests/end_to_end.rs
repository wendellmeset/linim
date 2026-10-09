//! End-to-end tests: run the actual `linim` binary against scratch repos.
//!
//! Interactive flows are avoided by pre-populating `.linim`, so no picker or
//! prompt runs. The non-tty stdin (a pipe) makes interactive-only flows fail
//! fast with `PromptFailed`, which is itself asserted in one test.

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::tempdir;

const STATE: &str = "# linim project state\nspdx = MIT\nholder = wendellmeset\nyear = 2026\nheaders = true\n";

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_linim")
}

fn write_state(dir: &Path) {
    fs::write(dir.join(".linim"), STATE).unwrap();
}

fn run(dir: &Path, flags: &[&str]) -> Output {
    Command::new(bin())
        .args(flags)
        .arg(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn fresh_repo_adds_header_and_license() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    let out = run(dir.path(), &[]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("Scanned 1, updated 1, skipped 0."));

    let rs = fs::read_to_string(dir.path().join("main.rs")).unwrap();
    assert!(rs.starts_with("// SPDX-License-Identifier: MIT\n"));

    let license = fs::read_to_string(dir.path().join("LICENSE")).unwrap();
    assert!(license.contains("Copyright (c) 2026 wendellmeset"));
    assert!(license.contains("MIT License"));

    assert!(dir.path().join(".linim").exists());
}

#[test]
fn second_run_is_idempotent() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    let before = b"// SPDX-License-Identifier: MIT\n// Copyright (c) 2026 wendellmeset\n\nfn main() {}\n";
    fs::write(dir.path().join("main.rs"), before).unwrap();

    let out = run(dir.path(), &[]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("Scanned 1, updated 0, skipped 1."));
    assert_eq!(fs::read(dir.path().join("main.rs")).unwrap(), before);
}

#[test]
fn no_headers_leaves_files_alone() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    let out = run(dir.path(), &["--no-headers"]);
    assert!(out.status.success());
    // No summary line at all: the run short-circuits before walking.
    assert_eq!(stdout(&out), "");

    assert_eq!(
        fs::read_to_string(dir.path().join("main.rs")).unwrap(),
        "fn main() {}\n"
    );
    assert!(dir.path().join("LICENSE").exists());
}

#[test]
fn new_with_no_tty_fails_cleanly_and_modifies_nothing() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    let out = run(dir.path(), &["--new"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("stdin is not a terminal"));

    // The picker fails before any writes happen.
    assert_eq!(
        fs::read_to_string(dir.path().join("main.rs")).unwrap(),
        "fn main() {}\n"
    );
    assert!(!dir.path().join("LICENSE").exists());
}

#[test]
fn ambiguous_header_reported() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    fs::write(
        dir.path().join("main.rs"),
        "// SPDX-License-Identifier: MIT\n\n// SPDX-License-Identifier: Apache-2.0\n\nfn main() {}\n",
    )
    .unwrap();

    let out = run(dir.path(), &[]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("Ambiguous header (skipped):"));
    assert!(text.contains("main.rs"));
    assert!(text.contains("2 license headers at lines 0, 2"));
}

#[test]
fn nonexistent_path_errors() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("nope");
    let out = run(&missing, &[]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("path is not a directory"));
}
