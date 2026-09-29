//! Read and write the `.linim` project state file.
//!
//! Dumb storage: records the project's four licensing decisions and nothing
//! else. No license, copyright, header, CLI, or git logic lives here. The
//! on-disk format is plain `key = value` lines: flat schema, zero parsing
//! dependencies.

use std::path::Path;

use crate::error::{LinimError, Result};
use crate::helpers;

/// The name of the state file at the repo root, committed to git.
pub const STATE_FILE: &str = ".linim";

/// The comment line written at the top of the state file.
const HEADER_COMMENT: &str = "# linim project state";

/// The project's stored licensing decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// The active SPDX identifier, e.g. `"MIT"`.
    pub spdx: String,
    /// The copyright holder name.
    pub holder: String,
    /// The copyright year.
    pub year: u16,
    /// Whether `--no-headers` was in effect last run.
    pub headers_enabled: bool,
}

impl State {
    /// Construct a new state with headers enabled.
    pub fn new(spdx: String, holder: String, year: u16) -> Self {
        State {
            spdx,
            holder,
            year,
            headers_enabled: true,
        }
    }
}

/// Read the state file under `root`.
///
/// Returns `Ok(None)` when the file does not exist (the normal first-run case),
/// and `Err(StateFileInvalid)` when it exists but is malformed.
pub fn read(root: &Path) -> Result<Option<State>> {
    let path = root.join(STATE_FILE);
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(LinimError::io(path, e)),
    };

    let mut spdx = None;
    let mut holder = None;
    let mut year = None;
    let mut headers = None;

    for (idx, raw) in contents.lines().enumerate() {
        let lineno = idx + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(eq) = line.find('=') else {
            return Err(LinimError::StateFileInvalid {
                reason: format!("line {lineno}: expected `key = value`: {raw:?}"),
            });
        };
        let key = line[..eq].trim();
        let value = line[eq + 1..].trim();
        match key {
            "spdx" => spdx = Some(value.to_string()),
            "holder" => holder = Some(value.to_string()),
            "year" => {
                year = Some(helpers::parse_year(value).ok_or_else(|| {
                    LinimError::StateFileInvalid {
                        reason: format!("invalid year value: '{value}'"),
                    }
                })?);
            }
            "headers" => {
                headers = Some(match value {
                    "true" => true,
                    "false" => false,
                    _ => {
                        return Err(LinimError::StateFileInvalid {
                            reason: format!("invalid headers value: '{value}'"),
                        })
                    }
                });
            }
            // Unknown keys are ignored so newer state files stay readable.
            _ => {}
        }
    }

    let spdx = required(spdx, "spdx")?;
    let holder = required(holder, "holder")?;
    let year = required(year, "year")?;
    let headers = required(headers, "headers")?;

    Ok(Some(State {
        spdx,
        holder,
        year,
        headers_enabled: headers,
    }))
}

fn required<T>(value: Option<T>, key: &str) -> Result<T> {
    value.ok_or_else(|| LinimError::StateFileInvalid {
        reason: format!("missing required key: {key}"),
    })
}

/// Write the state file under `root` in the fixed key order.
///
/// A direct write is fine: the file is four lines and replaced every run, so a
/// temp-file-and-rename dance buys nothing.
pub fn write(root: &Path, state: &State) -> Result<()> {
    let path = root.join(STATE_FILE);
    let mut out = String::with_capacity(64);
    out.push_str(HEADER_COMMENT);
    out.push('\n');
    out.push_str(&format!("spdx = {}\n", state.spdx));
    out.push_str(&format!("holder = {}\n", state.holder));
    out.push_str(&format!("year = {}\n", state.year));
    out.push_str(&format!(
        "headers = {}\n",
        if state.headers_enabled { "true" } else { "false" }
    ));
    std::fs::write(&path, out).map_err(|e| LinimError::io(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Create a fresh, empty temp directory for a test.
    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "linim-state-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_raw(root: &Path, contents: &str) {
        std::fs::write(root.join(STATE_FILE), contents).unwrap();
    }

    fn sample() -> State {
        State::new("MIT".into(), "wendellmeset".into(), 2026)
    }

    #[test]
    fn round_trip() {
        let dir = tempdir("roundtrip");
        let s = sample();
        write(&dir, &s).unwrap();
        let got = read(&dir).unwrap().expect("state read back");
        assert_eq!(got, s);
    }

    #[test]
    fn headers_false_round_trips() {
        let dir = tempdir("headers-false");
        let mut s = sample();
        s.headers_enabled = false;
        write(&dir, &s).unwrap();
        let got = read(&dir).unwrap().unwrap();
        assert!(!got.headers_enabled);
        assert_eq!(got, s);
    }

    #[test]
    fn missing_file_returns_none() {
        let dir = tempdir("missing");
        assert!(read(&dir).unwrap().is_none());
    }

    #[test]
    fn malformed_line_errors() {
        let dir = tempdir("malformed");
        write_raw(&dir, "spdx = MIT\nno equals here\n");
        let err = read(&dir).unwrap_err();
        assert!(err.to_string().contains("expected `key = value`"), "{err}");
    }

    #[test]
    fn missing_required_key_errors() {
        let dir = tempdir("missing-key");
        write_raw(&dir, "spdx = MIT\nholder = wendellmeset\nheaders = true\n");
        let err = read(&dir).unwrap_err();
        assert!(err.to_string().contains("year"), "{err}");
    }

    #[test]
    fn invalid_year_errors() {
        let dir = tempdir("bad-year");
        write_raw(&dir, "spdx = MIT\nholder = wendellmeset\nyear = abc\nheaders = true\n");
        let err = read(&dir).unwrap_err();
        assert!(err.to_string().contains("invalid year value"), "{err}");
    }

    #[test]
    fn invalid_headers_errors() {
        let dir = tempdir("bad-headers");
        write_raw(&dir, "spdx = MIT\nholder = wendellmeset\nyear = 2026\nheaders = maybe\n");
        let err = read(&dir).unwrap_err();
        assert!(err.to_string().contains("invalid headers value"), "{err}");
    }

    #[test]
    fn unknown_key_ignored() {
        let dir = tempdir("unknown");
        write_raw(
            &dir,
            "spdx = MIT\nholder = wendellmeset\nyear = 2026\nheaders = true\nfuture = 1\n",
        );
        let got = read(&dir).unwrap().unwrap();
        assert_eq!(got, sample());
    }

    #[test]
    fn blank_and_comments_skipped() {
        let dir = tempdir("comments");
        write_raw(
            &dir,
            "\n# a comment\nspdx = MIT\n\n# another\nholder = wendellmeset\nyear = 2026\nheaders = true\n",
        );
        let got = read(&dir).unwrap().unwrap();
        assert_eq!(got, sample());
    }

    #[test]
    fn duplicate_key_last_wins() {
        let dir = tempdir("duplicate");
        write_raw(
            &dir,
            "spdx = MIT\nspdx = GPL-3.0-only\nholder = wendellmeset\nyear = 2026\nheaders = true\n",
        );
        let got = read(&dir).unwrap().unwrap();
        assert_eq!(got.spdx, "GPL-3.0-only");
    }

    #[test]
    fn output_ends_with_single_newline() {
        let dir = tempdir("newline");
        write(&dir, &sample()).unwrap();
        let raw = std::fs::read_to_string(dir.join(STATE_FILE)).unwrap();
        assert!(raw.ends_with('\n'));
        assert!(!raw.ends_with("\n\n"));
        assert!(raw.ends_with("headers = true\n"));
    }

    #[test]
    fn whitespace_in_values_trimmed() {
        let dir = tempdir("trim");
        write_raw(
            &dir,
            "spdx =  MIT  \nholder =  wendellmeset \nyear = 2026\nheaders =  true \n",
        );
        let got = read(&dir).unwrap().unwrap();
        assert_eq!(got, sample());
    }
}
