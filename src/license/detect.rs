//! Identify an existing `LICENSE` file's SPDX identifier.
//!
//! Lets a first run on a repo that already has a recognized license skip the
//! picker. An `SPDX-License-Identifier:` line wins; otherwise the body is
//! byte-compared against the registry as a fingerprint (no fuzzy matching).

use std::path::Path;

use crate::error::{LinimError, Result};
use crate::helpers;
use crate::license::registry;

/// Candidate file names, tried in order.
const CANDIDATES: &[&str] = &["LICENSE", "LICENSE.md", "LICENSE.txt", "COPYING"];

/// Return the SPDX identifier of an existing license file, or `None` if absent
/// or unrecognizable.
pub fn detect(root: &Path) -> Result<Option<String>> {
    for name in CANDIDATES {
        let path = root.join(name);
        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(LinimError::io(path, e)),
        };

        // An SPDX line is authoritative: if present but names a license we
        // don't know, don't fall through to body matching.
        if let Some(spdx) = spdx_from_line(&contents) {
            return Ok(registry::by_spdx(&spdx).ok().map(|e| e.spdx.to_string()));
        }

        if let Some(spdx) = match_body(&contents) {
            return Ok(Some(spdx));
        }
    }
    Ok(None)
}

/// Extract the expression after an `SPDX-License-Identifier:` line.
fn spdx_from_line(contents: &str) -> Option<String> {
    for line in contents.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("SPDX-License-Identifier:") {
            let expr = rest.trim();
            if !expr.is_empty() {
                return Some(expr.to_string());
            }
        }
    }
    None
}

/// Fingerprint the body against registry entries, tolerating only trailing
/// newline differences.
fn match_body(contents: &str) -> Option<String> {
    let trimmed = helpers::trim_trailing_newlines(contents);
    for entry in registry::LICENSES {
        if helpers::trim_trailing_newlines(entry.body) == trimmed {
            return Some(entry.spdx.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn missing_license_is_none() {
        let dir = tempdir().unwrap();
        assert_eq!(detect(dir.path()).unwrap(), None);
    }

    #[test]
    fn spdx_line_matches() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("LICENSE"), "SPDX-License-Identifier: MIT\n").unwrap();
        assert_eq!(detect(dir.path()).unwrap(), Some("MIT".to_string()));
    }

    #[test]
    fn spdx_line_unknown_is_none() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("LICENSE"),
            "SPDX-License-Identifier: NOPE\n",
        )
        .unwrap();
        assert_eq!(detect(dir.path()).unwrap(), None);
    }

    #[test]
    fn body_byte_identical() {
        let dir = tempdir().unwrap();
        let entry = registry::by_spdx("GPL-3.0-only").unwrap();
        fs::write(dir.path().join("LICENSE"), entry.body).unwrap();
        assert_eq!(detect(dir.path()).unwrap(), Some("GPL-3.0-only".to_string()));
    }

    #[test]
    fn body_trailing_newline_tolerance() {
        let dir = tempdir().unwrap();
        let entry = registry::by_spdx("GPL-3.0-only").unwrap();
        // Drop the final newline; still recognized.
        let body = helpers::trim_trailing_newlines(entry.body);
        fs::write(dir.path().join("LICENSE"), body).unwrap();
        assert_eq!(detect(dir.path()).unwrap(), Some("GPL-3.0-only".to_string()));
    }

    #[test]
    fn spdx_line_beats_body() {
        let dir = tempdir().unwrap();
        // SPDX line says MIT even though the body is something else entirely.
        fs::write(
            dir.path().join("LICENSE"),
            "SPDX-License-Identifier: MIT\nsome other body\n",
        )
        .unwrap();
        assert_eq!(detect(dir.path()).unwrap(), Some("MIT".to_string()));
    }

    #[test]
    fn later_candidates_tried() {
        let dir = tempdir().unwrap();
        let entry = registry::by_spdx("Unlicense").unwrap();
        // No LICENSE, but a COPYING file carries the license.
        fs::write(dir.path().join("COPYING"), entry.body).unwrap();
        assert_eq!(detect(dir.path()).unwrap(), Some("Unlicense".to_string()));
    }
}
