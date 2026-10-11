//! Argument surface and parsing.
//!
//! `clap` handles usage errors (prints to stderr, exits non-zero) and `--help`
//! (prints, exits 0) automatically via the derive macro, so `parse()` returns
//! `Args` rather than `Result`.

use std::path::PathBuf;

use clap::Parser;

use crate::cli::help;

/// The parsed command-line arguments.
#[derive(Parser, Debug, PartialEq, Eq)]
#[command(version, about = help::LONG_ABOUT)]
pub struct Args {
    /// Reconfigure from scratch, bypassing `.linim` and any existing `LICENSE`.
    #[arg(long, help = help::NEW_HELP)]
    pub new: bool,

    /// Write or update `LICENSE` only; do not touch source file headers.
    #[arg(long, help = help::NO_HEADERS_HELP)]
    pub no_headers: bool,

    /// Emit one line per updated file to stderr.
    #[arg(long, help = help::VERBOSE_HELP)]
    pub verbose: bool,

    /// The directory to operate on.
    #[arg(default_value = ".", help = help::PATH_HELP)]
    pub path: PathBuf,
}

/// Parse the process arguments.
pub fn parse() -> Args {
    Args::parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_from(args: &[&str]) -> Result<Args, clap::Error> {
        Args::try_parse_from(args)
    }

    #[test]
    fn defaults() {
        let a = parse_from(&["linim"]).unwrap();
        assert!(!a.new);
        assert!(!a.no_headers);
        assert_eq!(a.path, PathBuf::from("."));
    }

    #[test]
    fn new_flag() {
        let a = parse_from(&["linim", "--new"]).unwrap();
        assert!(a.new);
        assert!(!a.no_headers);
    }

    #[test]
    fn no_headers_flag() {
        let a = parse_from(&["linim", "--no-headers"]).unwrap();
        assert!(a.no_headers);
        assert!(!a.new);
    }

    #[test]
    fn both_flags_combine() {
        let a = parse_from(&["linim", "--new", "--no-headers"]).unwrap();
        assert!(a.new);
        assert!(a.no_headers);
    }

    #[test]
    fn positional_path() {
        let a = parse_from(&["linim", "src"]).unwrap();
        assert_eq!(a.path, PathBuf::from("src"));
    }

    #[test]
    fn flag_and_path() {
        let a = parse_from(&["linim", "--new", "src"]).unwrap();
        assert!(a.new);
        assert_eq!(a.path, PathBuf::from("src"));
    }

    #[test]
    fn unknown_flag_errors() {
        assert!(parse_from(&["linim", "--unknown"]).is_err());
    }

    #[test]
    fn duplicate_flag_rejected() {
        // clap's default `SetTrue` action rejects repeated occurrences of a
        // bool flag with an `ArgumentConflict`. Documented behavior.
        assert!(parse_from(&["linim", "--new", "--new"]).is_err());
    }

    #[test]
    fn verbose_flag() {
        let a = parse_from(&["linim", "--verbose"]).unwrap();
        assert!(a.verbose);
    }

    #[test]
    fn version_flag() {
        use clap::CommandFactory;
        let cmd = Args::command();
        let version = cmd.render_version().to_string();
        assert!(version.contains("0.1.0"), "{version}");
    }

    #[test]
    fn help_renders() {
        use clap::CommandFactory;
        let mut cmd = Args::command();
        let out = cmd.render_help();
        let text = out.to_string();
        assert!(text.contains("manage a project's license"));
        assert!(text.contains("--new"));
        assert!(text.contains("--no-headers"));
    }
}
