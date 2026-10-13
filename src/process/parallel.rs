//! Run per-file processing across rayon's thread pool, preserving input order.
//!
//! Pure concurrency policy: no walking, no summary, no progress output. Uses
//! the global pool (no construction), and `par_iter().collect()` preserves the
//! sorted input order and keeps every file's `Result` independent.

use std::path::PathBuf;

use rayon::prelude::*;

use crate::process::single::{process_file, Context, FileOutcome};
use crate::error::Result;

/// Process every path and return one `Result` per input, in input order.
///
/// Errors are per-file: a failure on one path does not abort the rest.
pub fn process_all(paths: &[PathBuf], ctx: &Context) -> Vec<Result<FileOutcome>> {
    paths.par_iter().map(|p| process_file(p, ctx)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::single::FileOutcome;
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

    /// Classify results into comparable strings (avoids needing `PartialEq` on
    /// `LinimError`).
    fn classify(results: &[Result<FileOutcome>]) -> Vec<&'static str> {
        results
            .iter()
            .map(|r| match r {
                Ok(FileOutcome::Skipped) => "skipped",
                Ok(FileOutcome::Updated) => "updated",
                Ok(FileOutcome::Ambiguous(_)) => "ambiguous",
                Err(_) => "err",
            })
            .collect()
    }

    #[test]
    fn empty_input() {
        assert!(process_all(&[], &ctx("MIT", "wendellmeset", 2026)).is_empty());
    }

    #[test]
    fn single_empty_file_updated() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("a.rs");
        std::fs::write(&path, b"").unwrap();
        let results = process_all(std::slice::from_ref(&path), &ctx("MIT", "wendellmeset", 2026));
        assert_eq!(classify(&results), vec!["updated"]);
        assert!(std::fs::read(&path).unwrap().starts_with(b"// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n"));
    }

    #[test]
    fn three_files_all_need_headers() {
        let dir = tempdir().unwrap();
        let paths: Vec<PathBuf> = ["a.rs", "b.rs", "c.rs"]
            .iter()
            .map(|f| dir.path().join(f))
            .collect();
        for p in &paths {
            std::fs::write(p, b"fn main() {}\n").unwrap();
        }
        let results = process_all(&paths, &ctx("MIT", "wendellmeset", 2026));
        assert_eq!(classify(&results), vec!["updated"; 3]);
    }

    #[test]
    fn three_files_all_correct() {
        let dir = tempdir().unwrap();
        let paths: Vec<PathBuf> = ["a.rs", "b.rs", "c.rs"]
            .iter()
            .map(|f| dir.path().join(f))
            .collect();
        let header = b"// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\nfn main() {}\n";
        for p in &paths {
            std::fs::write(p, header).unwrap();
        }
        let results = process_all(&paths, &ctx("MIT", "wendellmeset", 2026));
        assert_eq!(classify(&results), vec!["skipped"; 3]);
    }

    #[test]
    fn mixed_outcomes_in_order() {
        let dir = tempdir().unwrap();
        // a.rs: needs a header (Updated)
        let a = dir.path().join("a.rs");
        std::fs::write(&a, b"fn main() {}\n").unwrap();
        // b.rs: already correct (Skipped)
        let b = dir.path().join("b.rs");
        std::fs::write(&b, b"// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\nfn main() {}\n")
            .unwrap();
        // c.rs: two headers (Ambiguous)
        let c = dir.path().join("c.rs");
        std::fs::write(
            &c,
            b"// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\n// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: Apache-2.0\n\nx\n",
        )
        .unwrap();

        let paths = vec![a.clone(), b.clone(), c.clone()];
        let results = process_all(&paths, &ctx("MIT", "wendellmeset", 2026));

        assert_eq!(classify(&results), vec!["updated", "skipped", "ambiguous"]);
        if let Ok(FileOutcome::Ambiguous(blocks)) = &results[2] {
            assert_eq!(blocks.len(), 2);
        } else {
            panic!("expected Ambiguous");
        }
    }

    #[test]
    fn missing_path_errors_at_right_index() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.rs");
        let missing = dir.path().join("missing.rs");
        let c = dir.path().join("c.rs");
        std::fs::write(&a, b"").unwrap();
        std::fs::write(&c, b"").unwrap();
        let paths = vec![a.clone(), missing.clone(), c.clone()];
        let results = process_all(&paths, &ctx("MIT", "wendellmeset", 2026));
        assert_eq!(classify(&results), vec!["updated", "err", "updated"]);
    }

    #[test]
    fn order_preserved_across_reordering() {
        let dir = tempdir().unwrap();
        let header =
            b"// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\nfn main() {}\n";

        // Pair 1: a correct (Skipped), b missing (Updated).
        let a1 = dir.path().join("a1.rs");
        let b1 = dir.path().join("b1.rs");
        std::fs::write(&a1, header).unwrap();
        std::fs::write(&b1, b"fn main() {}\n").unwrap();
        let fwd = process_all(&[a1, b1], &ctx("MIT", "wendellmeset", 2026));
        assert_eq!(classify(&fwd), vec!["skipped", "updated"]);

        // Pair 2 (fresh files): reversed order, b missing (Updated) first.
        let a2 = dir.path().join("a2.rs");
        let b2 = dir.path().join("b2.rs");
        std::fs::write(&a2, header).unwrap();
        std::fs::write(&b2, b"fn main() {}\n").unwrap();
        let rev = process_all(&[b2, a2], &ctx("MIT", "wendellmeset", 2026));
        assert_eq!(classify(&rev), vec!["updated", "skipped"]);
    }
}
