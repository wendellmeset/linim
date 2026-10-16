// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

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
    assert!(rs.starts_with("// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n"));

    let license = fs::read_to_string(dir.path().join("LICENSE")).unwrap();
    assert!(license.contains("Copyright (c) 2026 wendellmeset"));
    assert!(license.contains("MIT License"));

    assert!(dir.path().join(".linim").exists());
}

#[test]
fn second_run_is_idempotent() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    let before = b"// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\nfn main() {}\n";
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
        "// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\n// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: Apache-2.0\n\nfn main() {}\n",
    )
    .unwrap();

    let out = run(dir.path(), &[]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("Ambiguous header (skipped):"));
    assert!(text.contains("main.rs"));
    assert!(text.contains("2 license headers at lines 0, 3"));
}

#[test]
fn nonexistent_path_errors() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("nope");
    let out = run(&missing, &[]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("path is not a directory"));
}

#[test]
fn verbose_lists_updated_files_to_stderr() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    fs::write(dir.path().join("b.rs"), "fn b() {}\n").unwrap();

    let out = run(dir.path(), &["--verbose"]);
    assert!(out.status.success());
    let err = stderr(&out);
    assert!(err.contains("updated: a.rs"), "stderr: {err}");
    assert!(err.contains("updated: b.rs"), "stderr: {err}");
    // In the same (sorted) order as the walker yields them.
    let pos_a = err.find("updated: a.rs").unwrap();
    let pos_b = err.find("updated: b.rs").unwrap();
    assert!(pos_a < pos_b, "stderr: {err}");

    // Without --verbose, no update lines at all.
    let quiet = run(dir.path(), &[]);
    assert!(!stderr(&quiet).contains("updated:"));
}

#[test]
fn existing_recognized_license_reaches_copyright_prompt() {
    // A repo with a recognized LICENSE but no `.linim` skips the license
    // picker (detect wiring) and proceeds to the copyright prompt, which
    // fails on non-tty stdin, proving the picker was never reached.
    let dir = tempdir().unwrap();
    let entry = linim::license::registry::by_spdx("GPL-3.0-only").unwrap();
    fs::write(dir.path().join("LICENSE"), entry.body).unwrap();
    fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    let out = run(dir.path(), &[]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("stdin is not a terminal"));
    // Nothing modified: no .linim, no header, LICENSE untouched.
    assert!(!dir.path().join(".linim").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("main.rs")).unwrap(),
        "fn main() {}\n"
    );
}
