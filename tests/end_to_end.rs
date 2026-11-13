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
fn crlf_file_keeps_crlf_header() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    fs::write(dir.path().join("main.rs"), b"fn main() {}\r\n").unwrap();

    let out = run(dir.path(), &[]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let bytes = fs::read(dir.path().join("main.rs")).unwrap();
    assert_eq!(
        bytes,
        b"// SPDX-FileCopyrightText: 2026 wendellmeset\r\n// SPDX-License-Identifier: MIT\r\n\r\nfn main() {}\r\n"
    );

    let again = run(dir.path(), &[]);
    assert!(again.status.success());
    assert!(stdout(&again).contains("updated 0, skipped 1"));
    assert_eq!(fs::read(dir.path().join("main.rs")).unwrap(), bytes);
}

#[test]
fn check_passes_when_headers_are_correct_and_writes_nothing() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    let header =
        "// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\nfn main() {}\n";
    fs::write(dir.path().join("main.rs"), header).unwrap();

    let out = run(dir.path(), &["--check"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("Scanned 1, up to date 1, need update 0."), "{text}");
    assert_eq!(fs::read_to_string(dir.path().join("main.rs")).unwrap(), header);
    assert_eq!(fs::read_to_string(dir.path().join(".linim")).unwrap(), STATE);
    // Check is read-only: it must not materialize LICENSE.
    assert!(!dir.path().join("LICENSE").exists());
}

#[test]
fn check_fails_on_missing_header_without_writing() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    let out = run(dir.path(), &["--check"]);
    assert!(!out.status.success());
    let text = stdout(&out);
    assert!(text.contains("need update 1"), "{text}");
    assert!(text.contains("Missing or incorrect header:"), "{text}");
    assert!(text.contains("main.rs"), "{text}");
    assert_eq!(
        fs::read_to_string(dir.path().join("main.rs")).unwrap(),
        "fn main() {}\n"
    );
    assert!(!dir.path().join("LICENSE").exists());
}

#[test]
fn check_fails_on_wrong_header_without_writing() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    let wrong =
        "// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: Apache-2.0\n\nfn main() {}\n";
    fs::write(dir.path().join("main.rs"), wrong).unwrap();

    let out = run(dir.path(), &["--check"]);
    assert!(!out.status.success());
    assert!(stdout(&out).contains("need update 1"), "{}", stdout(&out));
    assert_eq!(fs::read_to_string(dir.path().join("main.rs")).unwrap(), wrong);
}

#[test]
fn check_fails_on_ambiguous_header_without_writing() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    let ambiguous = "// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\n// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: Apache-2.0\n\nfn main() {}\n";
    fs::write(dir.path().join("main.rs"), ambiguous).unwrap();

    let out = run(dir.path(), &["--check"]);
    assert!(!out.status.success());
    let text = stdout(&out);
    assert!(text.contains("Ambiguous header (skipped):"), "{text}");
    assert_eq!(
        fs::read_to_string(dir.path().join("main.rs")).unwrap(),
        ambiguous
    );
}

#[test]
fn check_requires_state_file() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    let out = run(dir.path(), &["--check"]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("no .linim state file found"),
        "stderr: {}",
        stderr(&out)
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("main.rs")).unwrap(),
        "fn main() {}\n"
    );
    assert!(!dir.path().join("LICENSE").exists());
}

#[test]
fn check_license_override_sets_expected_license() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    let mit = "// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\nfn main() {}\n";
    fs::write(dir.path().join("main.rs"), mit).unwrap();

    let out = run(dir.path(), &["--check", "--license", "Apache-2.0"]);
    assert!(!out.status.success());
    assert!(stdout(&out).contains("need update 1"), "{}", stdout(&out));
    assert_eq!(fs::read_to_string(dir.path().join("main.rs")).unwrap(), mit);
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
fn license_flag_switches_license_without_prompt() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    fs::write(
        dir.path().join("main.rs"),
        "// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\nfn main() {}\n",
    )
    .unwrap();

    let out = run(dir.path(), &["--license", "Apache-2.0"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("Scanned 1, updated 1, skipped 0."));

    let rs = fs::read_to_string(dir.path().join("main.rs")).unwrap();
    assert!(rs.contains("SPDX-License-Identifier: Apache-2.0"), "{rs}");
    let state = fs::read_to_string(dir.path().join(".linim")).unwrap();
    assert!(state.contains("spdx = Apache-2.0"), "{state}");
    let license = fs::read_to_string(dir.path().join("LICENSE")).unwrap();
    assert!(license.contains("Apache License"), "{license}");
}

#[test]
fn unknown_license_flag_fails_before_writing() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    let before =
        "// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\nfn main() {}\n";
    fs::write(dir.path().join("main.rs"), before).unwrap();

    let out = run(dir.path(), &["--license", "NOPE"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("license not found: NOPE"), "stderr: {}", stderr(&out));
    assert_eq!(
        fs::read_to_string(dir.path().join("main.rs")).unwrap(),
        before
    );
    assert!(!dir.path().join("LICENSE").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join(".linim")).unwrap(),
        STATE
    );
}

#[test]
fn new_with_license_still_prompts_for_copyright() {
    let dir = tempdir().unwrap();
    write_state(dir.path());
    fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    let out = run(dir.path(), &["--new", "--license", "MIT"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("stdin is not a terminal"), "stderr: {}", stderr(&out));
    assert_eq!(
        fs::read_to_string(dir.path().join("main.rs")).unwrap(),
        "fn main() {}\n"
    );
    assert!(!dir.path().join("LICENSE").exists());
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
