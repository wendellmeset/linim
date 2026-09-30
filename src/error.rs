//! The crate-wide error type.
//!
//! A single [`LinimError`] enum covers every failure the tool can hit. There is
//! deliberately no nested hierarchy and no `thiserror`/`anyhow`: a handwritten
//! [`Display`] and [`std::error::Error`] impl keep the dependency surface small.
//! Every variant that involves a path or key carries it so messages are useful.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

/// The error type returned by every fallible operation in linim.
#[derive(Debug)]
pub enum LinimError {
    /// A file read or write failure, with the path that was being accessed.
    Io { path: PathBuf, source: io::Error },

    /// The user asked for a license identifier not present in the registry.
    LicenseNotFound { spdx: String },

    /// No root `LICENSE` exists and none could be resolved.
    LicenseFileMissing,

    /// A root `LICENSE` exists but has no recognizable SPDX identifier.
    LicenseFileInvalid { reason: String },

    /// The `.linim` state file is malformed or unreadable.
    StateFileInvalid { reason: String },

    /// A file contains multiple qualifying license headers.
    AmbiguousHeader { path: PathBuf, lines: Vec<usize> },

    /// A file's extension has no known comment style (usually unreachable).
    NoCommentStyle { ext: String },

    /// An interactive prompt failed (non-tty stdin, etc.).
    PromptFailed { source: io::Error },

    /// The user cancelled an interactive prompt with Ctrl-C.
    Cancelled,

    /// A directory traversal error.
    WalkFailed { path: PathBuf, source: ignore::Error },

    /// Catch-all for cases not yet decided.
    Unsupported,
}

impl LinimError {
    /// Construct an [`LinimError::Io`] for the common `map_err` case.
    pub fn io(path: impl Into<PathBuf>, source: io::Error) -> LinimError {
        LinimError::Io {
            path: path.into(),
            source,
        }
    }

    /// Map a `dialoguer` error, distinguishing Ctrl-C from real failures.
    ///
    /// `dialoguer` wraps its underlying `io::Error`; a Ctrl-C surfaces as
    /// `io::ErrorKind::Interrupted`. Anything else is a genuine prompt failure
    /// (non-tty stdin, broken pipe, etc.).
    pub fn dialoguer(source: dialoguer::Error) -> LinimError {
        let source: io::Error = source.into();
        if source.kind() == io::ErrorKind::Interrupted {
            LinimError::Cancelled
        } else {
            LinimError::PromptFailed { source }
        }
    }
}

impl fmt::Display for LinimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinimError::Io { path, source } => {
                write!(f, "failed to access {}: {source}", path.display())
            }
            LinimError::LicenseNotFound { spdx } => {
                write!(f, "license not found: {spdx}")
            }
            LinimError::LicenseFileMissing => write!(f, "no LICENSE file found"),
            LinimError::LicenseFileInvalid { reason } => {
                write!(f, "LICENSE is not a recognized license: {reason}")
            }
            LinimError::StateFileInvalid { reason } => {
                write!(f, "invalid .linim state file: {reason}")
            }
            LinimError::AmbiguousHeader { path, lines } => {
                write!(
                    f,
                    "multiple license headers found in {} at lines {lines:?}",
                    path.display()
                )
            }
            LinimError::NoCommentStyle { ext } => {
                write!(f, "no comment style known for extension: {ext}")
            }
            LinimError::PromptFailed { source } => {
                write!(f, "interactive prompt failed: {source}")
            }
            LinimError::Cancelled => write!(f, "cancelled"),
            LinimError::WalkFailed { path, source } => {
                write!(f, "failed to walk {}: {source}", path.display())
            }
            LinimError::Unsupported => write!(f, "operation not yet supported"),
        }
    }
}

impl Error for LinimError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            LinimError::Io { source, .. } => Some(source),
            LinimError::PromptFailed { source } => Some(source),
            LinimError::WalkFailed { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// The crate's standard result type.
pub type Result<T> = std::result::Result<T, LinimError>;

#[cfg(test)]
mod tests {
    use super::*;

    fn io_err() -> io::Error {
        io::Error::new(io::ErrorKind::NotFound, "No such file or directory")
    }

    #[test]
    fn display_io_includes_path_and_source() {
        let err = LinimError::io("LICENSE", io_err());
        let msg = err.to_string();
        assert!(msg.contains("LICENSE"), "msg: {msg}");
        assert!(msg.contains("No such file or directory"), "msg: {msg}");
    }

    #[test]
    fn display_license_not_found() {
        let err = LinimError::LicenseNotFound {
            spdx: "NOEXIST".into(),
        };
        assert!(err.to_string().contains("NOEXIST"));
    }

    #[test]
    fn display_license_file_missing() {
        let msg = LinimError::LicenseFileMissing.to_string();
        assert!(msg.contains("LICENSE"));
    }

    #[test]
    fn display_license_file_invalid() {
        let err = LinimError::LicenseFileInvalid {
            reason: "no SPDX identifier".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("LICENSE"));
        assert!(msg.contains("no SPDX identifier"));
    }

    #[test]
    fn display_state_file_invalid() {
        let err = LinimError::StateFileInvalid {
            reason: "bad format".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains(".linim"));
        assert!(msg.contains("bad format"));
    }

    #[test]
    fn display_ambiguous_header() {
        let err = LinimError::AmbiguousHeader {
            path: "src/main.rs".into(),
            lines: vec![1, 3],
        };
        let msg = err.to_string();
        assert!(msg.contains("src/main.rs"));
        assert!(msg.contains("1"));
        assert!(msg.contains("3"));
    }

    #[test]
    fn display_no_comment_style() {
        let err = LinimError::NoCommentStyle { ext: "xyz".into() };
        assert!(err.to_string().contains("xyz"));
    }

    #[test]
    fn display_prompt_failed() {
        let err = LinimError::PromptFailed { source: io_err() };
        assert!(err.to_string().contains("No such file or directory"));
    }

    #[test]
    fn display_walk_failed() {
        let err = LinimError::WalkFailed {
            path: "src".into(),
            source: ignore::Error::from(io_err()),
        };
        let msg = err.to_string();
        assert!(msg.contains("src"));
        assert!(msg.contains("No such file or directory"));
    }

    #[test]
    fn display_unsupported() {
        assert_eq!(LinimError::Unsupported.to_string(), "operation not yet supported");
    }

    #[test]
    fn source_returns_underlying_for_wrapped() {
        assert!(LinimError::io("x", io_err()).source().is_some());
        assert!(LinimError::PromptFailed { source: io_err() }.source().is_some());
        assert!(LinimError::WalkFailed {
            path: "x".into(),
            source: ignore::Error::from(io_err()),
        }
        .source()
        .is_some());
        assert!(LinimError::LicenseFileMissing.source().is_none());
    }

    #[test]
    fn result_alias_usable() {
        fn f() -> Result<()> {
            Err(LinimError::Unsupported)
        }
        assert!(f().is_err());
    }
}
