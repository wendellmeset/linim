// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

//! Aggregate per-file results into a human-readable report.
//!
//! Pure aggregation and formatting: no file access, no parallel logic. The
//! caller pairs results with paths; we just count and render.

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::header::detect::HeaderBlock;
use crate::helpers;
use crate::process::single::FileOutcome;

/// Counts and report details for a run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    /// Total files examined.
    pub scanned: usize,
    /// Files rewritten.
    pub updated: usize,
    /// Files already correct (ambiguous files are NOT counted here).
    pub skipped: usize,
    /// Files with multiple headers: path and starting line numbers.
    pub ambiguous: Vec<(PathBuf, Vec<usize>)>,
    /// Files that failed: path and formatted error message.
    pub errors: Vec<(PathBuf, String)>,
    /// Paths of files rewritten, in input order (for `--verbose`).
    pub updated_paths: Vec<PathBuf>,
    /// Whether per-file update lines should be reported.
    pub verbose: bool,
}

/// Build a [`Summary`] from results paired with their paths.
///
/// `results[i]` corresponds to `paths[i]` (as produced by `process_all`).
pub fn summarize(results: &[Result<FileOutcome>], paths: &[PathBuf], verbose: bool) -> Summary {
    debug_assert_eq!(results.len(), paths.len());
    let mut summary = Summary {
        verbose,
        ..Summary::default()
    };
    for (path, result) in paths.iter().zip(results) {
        summary.scanned += 1;
        match result {
            Ok(FileOutcome::Skipped) => summary.skipped += 1,
            Ok(FileOutcome::Updated) => {
                summary.updated += 1;
                summary.updated_paths.push(path.clone());
            }
            Ok(FileOutcome::Ambiguous(blocks)) => {
                summary.ambiguous.push((path.clone(), lines_of(blocks)));
            }
            Err(e) => summary.errors.push((path.clone(), e.to_string())),
        }
    }
    summary
}

impl Summary {
    /// Render the report, showing paths relative to `root`.
    pub fn render(&self, root: &Path) -> String {
        let mut out = format!(
            "Scanned {}, updated {}, skipped {}.\n",
            self.scanned, self.updated, self.skipped
        );

        if !self.ambiguous.is_empty() {
            out.push('\n');
            out.push_str("Ambiguous header (skipped):\n");
            for (path, lines) in &self.ambiguous {
                let rel = helpers::relative_display(root, path);
                let line_str = lines
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!(
                    "  {rel} — {} license headers at lines {line_str}\n",
                    lines.len()
                ));
            }
        }

        if !self.errors.is_empty() {
            out.push('\n');
            out.push_str("Errors:\n");
            for (path, msg) in &self.errors {
                let rel = helpers::relative_display(root, path);
                out.push_str(&format!("  {rel} — {msg}\n"));
            }
        }

        out
    }

    /// Render one `updated: <relative path>` line per rewritten file, for stderr.
    ///
    /// Empty unless `verbose` is set. Paths are relative to `root`, matching the
    /// summary's conventions.
    pub fn render_verbose(&self, root: &Path) -> String {
        if !self.verbose {
            return String::new();
        }
        let mut out = String::new();
        for path in &self.updated_paths {
            out.push_str(&format!(
                "updated: {}\n",
                helpers::relative_display(root, path)
            ));
        }
        out
    }
}

fn lines_of(blocks: &[HeaderBlock]) -> Vec<usize> {
    blocks.iter().map(|b| b.start_line).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::detect::HeaderBlock;
    use crate::error::LinimError;
    use std::io;

    fn block(start_line: usize) -> HeaderBlock {
        HeaderBlock {
            start_line,
            end_line: start_line,
            start_byte: 0,
            end_byte: 0,
            spdx: None,
            copyright: None,
        }
    }

    fn io_err(msg: &str) -> LinimError {
        LinimError::io("/proj/src/blocked.rs", io::Error::new(io::ErrorKind::PermissionDenied, msg))
    }

    #[test]
    fn all_skipped_counts() {
        let paths = vec![
            PathBuf::from("/proj/a.rs"),
            PathBuf::from("/proj/b.rs"),
            PathBuf::from("/proj/c.rs"),
        ];
        let results: Vec<Result<FileOutcome>> =
            (0..3).map(|_| Ok(FileOutcome::Skipped)).collect();
        let s = summarize(&results, &paths, false);
        assert_eq!(s.scanned, 3);
        assert_eq!(s.skipped, 3);
        assert_eq!(s.updated, 0);
        assert!(s.ambiguous.is_empty());
        assert!(s.errors.is_empty());
    }

    #[test]
    fn all_updated_counts() {
        let paths = vec![PathBuf::from("/proj/a.rs")];
        let results: Vec<Result<FileOutcome>> = vec![Ok(FileOutcome::Updated)];
        let s = summarize(&results, &paths, false);
        assert_eq!(s.scanned, 1);
        assert_eq!(s.updated, 1);
        assert_eq!(s.skipped, 0);
    }

    #[test]
    fn mixed_counts_and_vectors() {
        let paths = vec![
            PathBuf::from("/proj/a.rs"),
            PathBuf::from("/proj/b.rs"),
            PathBuf::from("/proj/c.rs"),
        ];
        let results: Vec<Result<FileOutcome>> = vec![
            Ok(FileOutcome::Skipped),
            Ok(FileOutcome::Updated),
            Ok(FileOutcome::Ambiguous(vec![block(3), block(8)])),
        ];
        let s = summarize(&results, &paths, false);
        assert_eq!(s.scanned, 3);
        assert_eq!(s.skipped, 1);
        assert_eq!(s.updated, 1);
        assert_eq!(s.ambiguous, vec![(PathBuf::from("/proj/c.rs"), vec![3, 8])]);
        assert!(s.errors.is_empty());
    }

    #[test]
    fn render_minimal() {
        let paths = vec![PathBuf::from("/proj/a.rs")];
        let results: Vec<Result<FileOutcome>> = vec![Ok(FileOutcome::Skipped)];
        let s = summarize(&results, &paths, false);
        assert_eq!(s.render(Path::new("/proj")), "Scanned 1, updated 0, skipped 1.\n");
    }

    #[test]
    fn render_long_registry() {
        let paths: Vec<PathBuf> = (0..10).map(|i| PathBuf::from(format!("/proj/f{i}.rs"))).collect();
        let results: Vec<Result<FileOutcome>> =
            (0..10).map(|_| Ok(FileOutcome::Skipped)).collect();
        let s = summarize(&results, &paths, false);
        assert_eq!(s.render(Path::new("/proj")), "Scanned 10, updated 0, skipped 10.\n");
    }

    #[test]
    fn render_one_ambiguous() {
        let paths = vec![PathBuf::from("/proj/src/vendor/foo.go")];
        let results: Vec<Result<FileOutcome>> =
            vec![Ok(FileOutcome::Ambiguous(vec![block(3), block(8)]))];
        let s = summarize(&results, &paths, false);
        assert_eq!(
            s.render(Path::new("/proj")),
            "Scanned 1, updated 0, skipped 0.\n\nAmbiguous header (skipped):\n  src/vendor/foo.go — 2 license headers at lines 3, 8\n"
        );
    }

    #[test]
    fn render_one_error() {
        let paths = vec![PathBuf::from("/proj/src/blocked.rs")];
        let results: Vec<Result<FileOutcome>> = vec![Err(io_err("Permission denied (os error 13)"))];
        let s = summarize(&results, &paths, false);
        assert_eq!(
            s.render(Path::new("/proj")),
            "Scanned 1, updated 0, skipped 0.\n\nErrors:\n  src/blocked.rs — failed to access /proj/src/blocked.rs: Permission denied (os error 13)\n"
        );
    }

    #[test]
    fn render_both_ambiguous_and_error() {
        let paths = vec![
            PathBuf::from("/proj/src/amb.rs"),
            PathBuf::from("/proj/src/err.rs"),
        ];
        let results: Vec<Result<FileOutcome>> = vec![
            Ok(FileOutcome::Ambiguous(vec![block(1), block(5)])),
            Err(io_err("boom")),
        ];
        let s = summarize(&results, &paths, false);
        let out = s.render(Path::new("/proj"));
        assert!(out.starts_with("Scanned 2, updated 0, skipped 0.\n"));
        // Sections appear in order: counts, blank+ambiguous, blank+errors.
        let amb_pos = out.find("Ambiguous header (skipped)").unwrap();
        let err_pos = out.find("Errors:").unwrap();
        assert!(amb_pos < err_pos);
        assert!(out.contains("src/amb.rs — 2 license headers at lines 1, 5"));
        assert!(out.contains("src/err.rs —"));
        assert!(out.contains("boom"));
    }

    #[test]
    fn ordering_matches_input() {
        let paths = vec![
            PathBuf::from("/proj/x.rs"),
            PathBuf::from("/proj/y.rs"),
            PathBuf::from("/proj/z.rs"),
        ];
        let results: Vec<Result<FileOutcome>> = vec![
            Ok(FileOutcome::Ambiguous(vec![block(1)])),
            Err(io_err("e1")),
            Ok(FileOutcome::Ambiguous(vec![block(2)])),
        ];
        let s = summarize(&results, &paths, false);
        assert_eq!(s.ambiguous[0].0, PathBuf::from("/proj/x.rs"));
        assert_eq!(s.ambiguous[1].0, PathBuf::from("/proj/z.rs"));
        assert_eq!(s.errors[0].0, PathBuf::from("/proj/y.rs"));
    }

    #[test]
    fn paths_rendered_relative_to_root() {
        // A nested path renders relative; a path equal to root falls back to
        // the full path via `relative_display`.
        assert_eq!(
            helpers::relative_display(Path::new("/proj"), Path::new("/proj/src/lib.rs")),
            "src/lib.rs"
        );
        assert_eq!(
            helpers::relative_display(Path::new("/proj"), Path::new("/proj")),
            "/proj"
        );
    }

    #[test]
    fn render_verbose_lists_updated_paths() {
        let paths = vec![
            PathBuf::from("/proj/src/a.rs"),
            PathBuf::from("/proj/src/b.rs"),
            PathBuf::from("/proj/src/c.rs"),
        ];
        let results: Vec<Result<FileOutcome>> = vec![
            Ok(FileOutcome::Updated),
            Ok(FileOutcome::Skipped),
            Ok(FileOutcome::Updated),
        ];
        let s = summarize(&results, &paths, true);
        assert_eq!(
            s.render_verbose(Path::new("/proj")),
            "updated: src/a.rs\nupdated: src/c.rs\n"
        );
    }

    #[test]
    fn render_verbose_empty_when_not_verbose() {
        let paths = vec![PathBuf::from("/proj/a.rs")];
        let results: Vec<Result<FileOutcome>> = vec![Ok(FileOutcome::Updated)];
        let s = summarize(&results, &paths, false);
        assert_eq!(s.render_verbose(Path::new("/proj")), "");
    }
}
