// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

//! Per-file orchestration: read, detect, compare, and rewrite as needed.
//!
//! The fast path reads only the first 8 KB and skips writes for correct files;
//! the slow path performs a full read, strip, inject, and write. No threading,
//! no CLI, no summary, just one file, classified.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::error::{LinimError, Result};
use crate::header::comment_map;
use crate::header::detect::{self, Detection, HeaderBlock};
use crate::header::inject;
use crate::header::preamble;
use crate::header::strip;
use crate::header::template::HeaderTemplate;
use crate::helpers;

/// Bytes read on the fast path; files this size or smaller never need a second
/// read even when a rewrite is required.
pub const FAST_PATH_BYTES: u64 = 8192;

/// What happened to a file. Errors are not variants: they are `Err`s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOutcome {
    /// The file already had a correct header (or was not modifiable); no write.
    Skipped,
    /// The file was rewritten (header inserted or replaced).
    Updated,
    /// The file had two or more qualifying headers; not modified.
    Ambiguous(Vec<HeaderBlock>),
}

/// Everything `process_file` needs that is fixed across the whole run.
pub struct Context {
    /// The expected SPDX expression, e.g. `"MIT"`.
    pub expected_spdx: String,
    /// The expected copyright line, e.g. `"SPDX-FileCopyrightText: 2026 wendellmeset"`.
    pub expected_copyright: String,
    /// The header template, rendered per file for the file's comment style.
    pub template: HeaderTemplate,
}

/// Process a single file, returning its outcome.
pub fn process_file(path: &Path, ctx: &Context) -> Result<FileOutcome> {
    // 1. Comment style. Defensive: the walker already filtered these out.
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return Ok(FileOutcome::Skipped);
    };
    let Some(style) = comment_map::comment_style(ext) else {
        return Ok(FileOutcome::Skipped);
    };

    // 2. Open.
    let mut file = File::open(path).map_err(|e| LinimError::io(path, e))?;

    // 3. Fast-path read: at most FAST_PATH_BYTES.
    let mut buf = Vec::new();
    file.by_ref()
        .take(FAST_PATH_BYTES)
        .read_to_end(&mut buf)
        .map_err(|e| LinimError::io(path, e))?;

    // 4. Does the buffer already hold the whole file?
    let file_len = file
        .metadata()
        .map_err(|e| LinimError::io(path, e))?
        .len();
    let has_full = file_len <= buf.len() as u64;

    // 5. Preamble, then existing headers.
    let preamble = preamble::detect(&buf);
    let detection = detect::detect(&buf, &preamble, style);

    match detection {
        Detection::None => {
            // Rewrite path: no strip needed.
            let full = if has_full { buf } else { read_full(path)? };
            let rendered = ctx.template.render(style);
            let new_buf = inject::inject(&full, &preamble, &rendered);
            write_all(path, &new_buf)?;
            Ok(FileOutcome::Updated)
        }
        Detection::Single(block) => {
            if header_matches(&block, ctx) {
                return Ok(FileOutcome::Skipped);
            }
            let full = if has_full { buf } else { read_full(path)? };
            let stripped = strip::strip(&full, &block);
            let rendered = ctx.template.render(style);
            let new_buf = inject::inject(&stripped, &preamble, &rendered);
            write_all(path, &new_buf)?;
            Ok(FileOutcome::Updated)
        }
        Detection::Ambiguous(blocks) => Ok(FileOutcome::Ambiguous(blocks)),
    }
}

/// Whether an existing header matches the expected one.
///
/// SPDX must match exactly (case- and punctuation-sensitive). Copyright must
/// match after whitespace normalization. Both must be present: an old-style
/// header with no SPDX line is a mismatch and gets rewritten.
fn header_matches(block: &HeaderBlock, ctx: &Context) -> bool {
    let spdx_ok = block
        .spdx
        .as_deref()
        .map(|s| s.trim() == ctx.expected_spdx.trim())
        .unwrap_or(false);
    if !spdx_ok {
        return false;
    }
    block
        .copyright
        .as_deref()
        .map(|c| helpers::eq_normalized(c, &ctx.expected_copyright))
        .unwrap_or(false)
}

/// Re-open and read a file in full (slow-path only, for files over 8 KB).
fn read_full(path: &Path) -> Result<Vec<u8>> {
    let mut full = Vec::new();
    File::open(path)
        .and_then(|mut f| f.read_to_end(&mut full))
        .map_err(|e| LinimError::io(path, e))?;
    Ok(full)
}

fn write_all(path: &Path, contents: &[u8]) -> Result<()> {
    std::fs::write(path, contents).map_err(|e| LinimError::io(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::template;
    use tempfile::tempdir;

    fn ctx(spdx: &str, holder: &str, year: u16) -> Context {
        let template = template::build(spdx, holder, year);
        Context {
            expected_spdx: spdx.to_string(),
            expected_copyright: template.copyright_line.clone(),
            template,
        }
    }

    #[test]
    fn empty_file_gets_header() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("main.rs");
        std::fs::write(&path, b"").unwrap();
        assert_eq!(process_file(&path, &ctx("MIT", "wendellmeset", 2026)).unwrap(), FileOutcome::Updated);
        let out = std::fs::read(&path).unwrap();
        assert!(out.starts_with(b"// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n"));
    }

    #[test]
    fn correct_header_is_skipped() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("main.rs");
        let original =
            b"// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\nfn main() {}\n";
        std::fs::write(&path, original).unwrap();
        assert_eq!(process_file(&path, &ctx("MIT", "wendellmeset", 2026)).unwrap(), FileOutcome::Skipped);
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    #[test]
    fn different_spdx_is_replaced() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("main.rs");
        std::fs::write(
            &path,
            b"// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: Apache-2.0\n\ncode\n",
        )
        .unwrap();
        assert_eq!(process_file(&path, &ctx("MIT", "wendellmeset", 2026)).unwrap(), FileOutcome::Updated);
        let out = std::fs::read(&path).unwrap();
        assert!(out.starts_with(b"// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n"));
        assert!(out.windows(b"Apache-2.0".len()).all(|w| w != b"Apache-2.0"));
    }

    #[test]
    fn old_style_mit_gets_spdx_line() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("main.rs");
        std::fs::write(
            &path,
            b"// Copyright (c) 2026 wendellmeset\n//\n// Permission is hereby granted, free of charge, to any person\n// obtaining a copy of this software...\n\nfn main() {}\n",
        )
        .unwrap();
        assert_eq!(process_file(&path, &ctx("MIT", "wendellmeset", 2026)).unwrap(), FileOutcome::Updated);
        let out = std::fs::read(&path).unwrap();
        assert!(out.starts_with(b"// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n"));
    }

    #[test]
    fn two_headers_are_ambiguous_and_unchanged() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("main.rs");
        let original =
            b"// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\n// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: Apache-2.0\n\ncode\n";
        std::fs::write(&path, original).unwrap();
        match process_file(&path, &ctx("MIT", "wendellmeset", 2026)).unwrap() {
            FileOutcome::Ambiguous(blocks) => assert_eq!(blocks.len(), 2),
            other => panic!("expected Ambiguous, got {other:?}"),
        }
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    #[test]
    fn shebang_without_header_gets_header_after() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("script.py");
        std::fs::write(&path, b"#!/usr/bin/env python3\nprint('hi')\n").unwrap();
        assert_eq!(process_file(&path, &ctx("MIT", "wendellmeset", 2026)).unwrap(), FileOutcome::Updated);
        let out = String::from_utf8(std::fs::read(&path).unwrap()).unwrap();
        assert!(out.starts_with("#!/usr/bin/env python3\n# SPDX-FileCopyrightText: 2026 wendellmeset\n# SPDX-License-Identifier: MIT\n"));
    }

    #[test]
    fn no_comment_syntax_skipped() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        std::fs::write(&path, b"{}\n").unwrap();
        assert_eq!(process_file(&path, &ctx("MIT", "wendellmeset", 2026)).unwrap(), FileOutcome::Skipped);
    }

    #[test]
    fn large_file_correct_header_skipped() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("big.rs");
        let mut original =
            b"// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\n".to_vec();
        original.extend(std::iter::repeat_n(b'x', 9000));
        original.push(b'\n');
        std::fs::write(&path, &original).unwrap();
        assert!(original.len() as u64 > FAST_PATH_BYTES);
        assert_eq!(process_file(&path, &ctx("MIT", "wendellmeset", 2026)).unwrap(), FileOutcome::Skipped);
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    #[test]
    fn large_file_missing_header_updated_full_preserved() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("big.rs");
        let mut original = b"fn main() {}\n".to_vec();
        original.extend(std::iter::repeat_n(b'x', 9000));
        original.push(b'\n');
        std::fs::write(&path, &original).unwrap();
        assert_eq!(process_file(&path, &ctx("MIT", "wendellmeset", 2026)).unwrap(), FileOutcome::Updated);
        let out = std::fs::read(&path).unwrap();
        assert!(out.starts_with(b"// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n"));
        // The original tail is preserved after the inserted header.
        let header = b"// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\n";
        assert_eq!(&out[header.len()..], original.as_slice());
    }

    #[test]
    fn nonexistent_file_errors() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nope.rs");
        assert!(process_file(&path, &ctx("MIT", "wendellmeset", 2026)).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_file_errors() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("a.rs");
        std::fs::write(&path, b"// x\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        // Skip when running as root (mode 000 is still readable).
        if std::fs::read(&path).is_ok() {
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644));
            return;
        }
        assert!(process_file(&path, &ctx("MIT", "wendellmeset", 2026)).is_err());
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644));
    }
}
