//! Directory traversal: walks a repo with `ignore`, applying our extension
//! policy to each file.
//!
//! Discovery only: no file opening, no processing, no threading. Collects the
//! full sorted list of candidate files, then hands it to `process/parallel.rs`.

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::error::{LinimError, Result};

pub mod filter;

/// Traverse `root`, pruning ignored paths, and return the sorted list of files
/// that pass both `ignore`'s filtering and our comment-syntax filter.
///
/// Only a hard failure of the root itself returns `Err`; individual entry
/// errors (e.g. a permission-denied subdirectory) are swallowed so one bad file
/// doesn't abort the whole run.
pub fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    // Hard failure: the root must exist and be a directory.
    match std::fs::metadata(root) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => {
            return Err(LinimError::WalkFailed {
                path: root.to_path_buf(),
                source: ignore::Error::from(std::io::Error::new(
                    std::io::ErrorKind::NotADirectory,
                    "root is not a directory",
                )),
            });
        }
        Err(e) => {
            return Err(LinimError::WalkFailed {
                path: root.to_path_buf(),
                source: ignore::Error::from(e),
            });
        }
    }

    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(true) // skip dotfiles, matching git's default
        .git_ignore(true) // respect .gitignore
        .git_global(true) // respect the user's global gitignore
        .git_exclude(true) // respect .git/info/exclude
        .ignore(true) // also read .ignore files
        .parents(true) // respect .gitignore in parent directories
        .require_git(false) // apply gitignore rules even outside a git repo
        .follow_links(false); // never follow symlinks

    let mut paths = Vec::new();
    for entry in builder.build() {
        let Ok(entry) = entry else {
            continue; // entry-level errors are non-fatal
        };
        let Some(ft) = entry.file_type() else {
            continue;
        };
        if !ft.is_file() {
            continue; // dirs descend automatically; symlinks are not followed
        }
        if filter::should_process(entry.path()) {
            paths.push(entry.into_path());
        }
    }

    // Byte-stable, locale-independent ordering for deterministic output.
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(root: &Path, rel: &str, contents: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn empty_directory() {
        let dir = tempdir().unwrap();
        assert_eq!(collect_files(dir.path()).unwrap(), Vec::<PathBuf>::new());
    }

    #[test]
    fn single_rust_file() {
        let dir = tempdir().unwrap();
        write(dir.path(), "main.rs", "fn main() {}\n");
        let files = collect_files(dir.path()).unwrap();
        assert_eq!(files, vec![dir.path().join("main.rs")]);
    }

    #[test]
    fn only_comment_capable_files_survive() {
        let dir = tempdir().unwrap();
        write(dir.path(), "main.rs", "fn main() {}\n");
        write(dir.path(), "data.json", "{}\n");
        write(dir.path(), "README.md", "# hi\n");
        let files = collect_files(dir.path()).unwrap();
        assert_eq!(files, vec![dir.path().join("main.rs")]);
    }

    #[test]
    fn gitignore_respected() {
        let dir = tempdir().unwrap();
        write(dir.path(), ".gitignore", "foo.rs\n");
        write(dir.path(), "foo.rs", "// ignored\n");
        write(dir.path(), "bar.rs", "// kept\n");
        let files = collect_files(dir.path()).unwrap();
        assert_eq!(files, vec![dir.path().join("bar.rs")]);
    }

    #[test]
    fn nested_gitignore_respected() {
        let dir = tempdir().unwrap();
        write(dir.path(), ".gitignore", "*.log\n");
        write(dir.path(), "sub/.gitignore", "secret.rs\n");
        write(dir.path(), "a.rs", "// a\n");
        write(dir.path(), "b.log", "log\n");
        write(dir.path(), "sub/c.rs", "// c\n");
        write(dir.path(), "sub/secret.rs", "// secret\n");
        let files = collect_files(dir.path()).unwrap();
        assert_eq!(
            files,
            vec![dir.path().join("a.rs"), dir.path().join("sub/c.rs")]
        );
    }

    #[test]
    fn git_directory_not_walked() {
        let dir = tempdir().unwrap();
        write(dir.path(), ".git/config", "[core]\n");
        write(dir.path(), "a.rs", "// a\n");
        let files = collect_files(dir.path()).unwrap();
        assert_eq!(files, vec![dir.path().join("a.rs")]);
    }

    #[test]
    fn hidden_files_skipped() {
        let dir = tempdir().unwrap();
        write(dir.path(), ".hidden.rs", "// hidden\n");
        write(dir.path(), "a.rs", "// a\n");
        let files = collect_files(dir.path()).unwrap();
        assert_eq!(files, vec![dir.path().join("a.rs")]);
    }

    #[test]
    fn result_is_sorted() {
        let dir = tempdir().unwrap();
        write(dir.path(), "zeta.rs", "// z\n");
        write(dir.path(), "alpha.rs", "// a\n");
        write(dir.path(), "mid.rs", "// m\n");
        let files = collect_files(dir.path()).unwrap();
        let expected: Vec<PathBuf> = ["alpha.rs", "mid.rs", "zeta.rs"]
            .iter()
            .map(|f| dir.path().join(f))
            .collect();
        assert_eq!(files, expected);
    }

    #[test]
    fn root_does_not_exist() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("nope");
        assert!(collect_files(&missing).is_err());
    }

    #[test]
    fn root_is_a_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("file.rs");
        fs::write(&file, "// x\n").unwrap();
        assert!(collect_files(&file).is_err());
    }
}
