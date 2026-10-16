// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

//! Extension-based policy: which files linim knows a comment syntax for.
//!
//! Pure and deterministic: no filesystem access, no traversal. This is *our*
//! policy (does the file have a comment style?), kept separate from `ignore`'s
//! policy (gitignore, hidden files, symlinks).

use std::path::Path;

use crate::header::comment_map;

/// Whether `path`'s extension maps to a known `CommentStyle`.
///
/// Returns `false` for paths with no extension, a non-UTF8 extension, or an
/// extension not in the comment map. `comment_map::comment_style` already
/// normalizes case and leading dots.
pub fn should_process(path: &Path) -> bool {
    let Some(ext) = path.extension() else {
        return false;
    };
    let Some(ext) = ext.to_str() else {
        // A non-UTF8 extension can never be in the comment map.
        return false;
    };
    comment_map::comment_style(ext).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_file() {
        assert!(should_process(Path::new("main.rs")));
    }

    #[test]
    fn python_file() {
        assert!(should_process(Path::new("script.py")));
    }

    #[test]
    fn html_file() {
        assert!(should_process(Path::new("page.html")));
    }

    #[test]
    fn json_not_supported() {
        assert!(!should_process(Path::new("data.json")));
    }

    #[test]
    fn markdown_not_supported() {
        assert!(!should_process(Path::new("README.md")));
    }

    #[test]
    fn no_extension() {
        assert!(!should_process(Path::new("Makefile")));
    }

    #[test]
    fn trailing_dot_empty_extension() {
        assert!(!should_process(Path::new("weird.")));
    }

    #[test]
    fn dotfile_with_no_stem() {
        // `Path::extension()` returns `None` for `.gitignore`.
        assert!(!should_process(Path::new(".gitignore")));
    }

    #[test]
    fn uppercase_extension_normalized() {
        assert!(should_process(Path::new("file.RS")));
    }

    #[test]
    fn path_with_directory_component() {
        assert!(should_process(Path::new("src/lib.rs")));
    }
}
