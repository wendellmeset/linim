// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

//! Small, top-level pure utility functions shared by two or more domain modules.
//!
//! Everything here is `std`-only, has no I/O, holds no global state, and never
//! references any other module of this crate. Helpers that are only used by a
//! single module belong in that module instead; this file is reserved for
//! genuinely shared utilities.

use std::path::Path;

/// Iterate over the lines of a byte buffer, considering at most `limit` bytes.
///
/// Scanning stops once the cumulative region exceeds `limit`, so a line that
/// runs past the limit is truncated at that boundary. Each yielded `&str` is a
/// line *without* its trailing `\n` (a trailing `\r` from CRLF is kept). No
/// allocation is performed and no `Vec` of lines is built.
///
/// # Safety contract
///
/// The caller must pass buffer bytes that are valid UTF-8 in the scanned
/// region. Source files are text, so this holds for every intended use.
pub fn lines_upto(buf: &[u8], limit: usize) -> impl Iterator<Item = &str> + '_ {
    let end = buf.len().min(limit);
    LinesUpto {
        buf: &buf[..end],
        pos: 0,
    }
}

/// The iterator backing [`lines_upto`]. Scans for `\n` byte by byte, then
/// hands back the slice up to (but not including) the newline.
struct LinesUpto<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Iterator for LinesUpto<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.pos >= self.buf.len() {
            return None;
        }
        let rest = &self.buf[self.pos..];
        let nl = rest.iter().position(|&b| b == b'\n');
        let end = match nl {
            Some(i) => self.pos + i,
            None => self.buf.len(),
        };
        let line = &self.buf[self.pos..end];
        // `self.buf` is a prefix of `buf`, which the caller guaranteed is
        // valid UTF-8 (see `lines_upto`). Each `line` is a sub-slice of it, so
        // it is also valid UTF-8.
        self.pos = end + 1;
        Some(unsafe { std::str::from_utf8_unchecked(line) })
    }
}

/// Return the zero-based line index that `byte_offset` falls on.
///
/// Counting the `\n` bytes strictly before the offset gives the index of the
/// line containing it: line 0 starts before any newline. Offsets past the end
/// of the buffer are clamped to the last line.
pub fn line_index_at(buf: &[u8], byte_offset: usize) -> usize {
    let offset = byte_offset.min(buf.len());
    buf[..offset].iter().filter(|&&b| b == b'\n').count()
}

/// Collapse runs of whitespace to a single space and trim both ends.
///
/// This also folds `\r\n`, tabs, and any other Unicode whitespace into single
/// spaces, so line-ending and indentation differences never cause a false
/// mismatch when comparing headers.
pub fn normalize_ws(s: &str) -> String {
    let mut words = s.split_whitespace();
    let mut out = String::with_capacity(s.len());
    if let Some(first) = words.next() {
        out.push_str(first);
        for w in words {
            out.push(' ');
            out.push_str(w);
        }
    }
    out
}

/// Compare two strings after normalizing whitespace and trailing newlines.
///
/// Uses lazy, allocation-free iteration over whitespace-delimited words, so no
/// `String` is built even when the inputs differ only in whitespace. A fast
/// byte-for-byte equality check short-circuits the common case.
pub fn eq_normalized(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    a.split_whitespace().eq(b.split_whitespace())
}

/// Current calendar year from the system clock.
pub fn current_year() -> u16 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    civil_year((secs / 86_400) as i64)
}

/// Convert days since the Unix epoch to the calendar year (Hinnant's
/// civil-from-days algorithm). All arithmetic is on `i64` and correct for any
/// era.
fn civil_year(days_since_epoch: i64) -> u16 {
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (y + if m <= 2 { 1 } else { 0 }) as u16
}

/// Format a single year, e.g. `"2026"`.
pub fn format_year(year: u16) -> String {
    year.to_string()
}

/// Format a year, or a `start-end` range when `start != end`.
pub fn format_year_range(start: u16, end: u16) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}-{end}")
    }
}

/// Parse `"2026"` or the trailing end of a range like `"2020-2026"`.
///
/// Returns `None` for input that has no parseable 4-digit year segment.
pub fn parse_year(s: &str) -> Option<u16> {
    let seg = s.rsplit('-').next()?.trim();
    if seg.len() != 4 {
        return None;
    }
    let year = seg.parse::<u16>().ok()?;
    (1000..=9999).contains(&year).then_some(year)
}

/// Replace every `{key}` in `template` with its value from `vars`.
///
/// A single left-to-right pass with no regex and no intermediate allocations.
/// Placeholders with no matching key are left verbatim. A `{` with no closing
/// `}` is emitted literally.
pub fn substitute(template: &str, vars: &[(&str, &str)]) -> String {
    let bytes = template.as_bytes();
    let mut out = String::with_capacity(template.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{'
            && let Some(rel) = bytes[i + 1..].iter().position(|&b| b == b'}')
        {
            let key = &template[i + 1..i + 1 + rel];
            if let Some((_, val)) = vars.iter().find(|(k, _)| *k == key) {
                out.push_str(val);
                i += 1 + rel + 1;
                continue;
            }
        }
        let ch = template[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Whether `template` references the placeholder `{key}`.
///
/// No allocation is performed.
pub fn contains_placeholder(template: &str, key: &str) -> bool {
    let bytes = template.as_bytes();
    let kb = key.as_bytes();
    let mut i = 0;
    while i + 1 + kb.len() < bytes.len() {
        if bytes[i] == b'{'
            && bytes[i + 1..i + 1 + kb.len()] == *kb
            && bytes[i + 1 + kb.len()] == b'}'
        {
            return true;
        }
        i += 1;
    }
    false
}

/// Strip a leading dot and lowercase an extension.
pub fn normalize_extension(ext: &str) -> String {
    ext.strip_prefix('.').unwrap_or(ext).to_lowercase()
}

/// Prefix every line of `text` with `prefix`, leaving empty lines as a bare
/// `prefix` so the comment block stays contiguous.
///
/// Every produced line is terminated with `\n`, including the last.
pub fn prefix_lines(text: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(text.len() + prefix.len() * 8 + 1);
    for line in text.lines() {
        out.push_str(prefix);
        if !line.is_empty() {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Render `path` relative to `root` for human-readable output.
///
/// Falls back to the full path when `path` is not under `root`. A path equal to
/// `root` is shown as `root` itself.
pub fn relative_display(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) if !rel.as_os_str().is_empty() => rel.display().to_string(),
        _ => path.display().to_string(),
    }
}

/// Trim trailing line terminators (`\n`, `\r`) from a string.
///
/// Used for byte-exact comparisons that tolerate a missing or extra final
/// newline, and nothing else. Legal text is not whitespace-normalized away.
pub fn trim_trailing_newlines(s: &str) -> &str {
    s.trim_end_matches(['\n', '\r'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_empty_buffer() {
        assert_eq!(lines_upto(b"", 8).collect::<Vec<_>>(), Vec::<&str>::new());
    }

    #[test]
    fn lines_single_line() {
        assert_eq!(
            lines_upto(b"hello", 16).collect::<Vec<_>>(),
            vec!["hello"]
        );
    }

    #[test]
    fn lines_trailing_newline() {
        assert_eq!(lines_upto(b"a\nb\n", 16).collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn lines_no_trailing_newline() {
        assert_eq!(lines_upto(b"a\nb", 16).collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn lines_byte_limit_mid_line() {
        // "hello\nworld" with a limit of 7 bytes scans "hello\nw".
        assert_eq!(lines_upto(b"hello\nworld", 7).collect::<Vec<_>>(), vec!["hello", "w"]);
    }

    #[test]
    fn line_index_basic() {
        let buf = b"a\nbb\nccc";
        assert_eq!(line_index_at(buf, 0), 0);
        assert_eq!(line_index_at(buf, 1), 0);
        assert_eq!(line_index_at(buf, 2), 1);
        assert_eq!(line_index_at(buf, 5), 2);
        assert_eq!(line_index_at(buf, 99), 2); // clamped
    }

    #[test]
    fn normalize_crlf_vs_lf() {
        assert_eq!(normalize_ws("a\r\nb"), "a b");
        assert_eq!(normalize_ws("a\nb"), "a b");
    }

    #[test]
    fn normalize_tabs_vs_spaces() {
        assert_eq!(normalize_ws("a\t\tb"), "a b");
        assert_eq!(normalize_ws("a  b"), "a b");
    }

    #[test]
    fn normalize_trailing_whitespace() {
        assert_eq!(normalize_ws("a  \n  "), "a");
    }

    #[test]
    fn normalize_empty() {
        assert_eq!(normalize_ws(""), "");
        assert_eq!(normalize_ws("   \n\t "), "");
    }

    #[test]
    fn eq_normalized_whitespace() {
        assert!(eq_normalized("a  b", "a b"));
        assert!(eq_normalized("a\r\nb", "a\nb"));
        assert!(eq_normalized("a ", "a"));
        assert!(eq_normalized("", ""));
        assert!(!eq_normalized("a", "b"));
        assert!(!eq_normalized("a b", "a c"));
    }

    #[test]
    fn current_year_is_sane() {
        let y = current_year();
        assert!((2000..=2100).contains(&y));
    }

    #[test]
    fn format_year_single() {
        assert_eq!(format_year(2026), "2026");
    }

    #[test]
    fn format_year_range_basic() {
        assert_eq!(format_year_range(2020, 2026), "2020-2026");
        assert_eq!(format_year_range(2020, 2020), "2020");
    }

    #[test]
    fn parse_year_single() {
        assert_eq!(parse_year("2026"), Some(2026));
    }

    #[test]
    fn parse_year_range_end() {
        assert_eq!(parse_year("2020-2026"), Some(2026));
    }

    #[test]
    fn parse_year_malformed() {
        assert_eq!(parse_year("garbage"), None);
        assert_eq!(parse_year("20"), None);
        assert_eq!(parse_year(""), None);
        // A range's start may be arbitrary; only the end matters.
        assert_eq!(parse_year("999-2026"), Some(2026));
    }

    #[test]
    fn substitute_single() {
        assert_eq!(substitute("Copyright {year}", &[("year", "2026")]), "Copyright 2026");
    }

    #[test]
    fn substitute_multiple() {
        assert_eq!(
            substitute("{spdx} {year}", &[("spdx", "MIT"), ("year", "2026")]),
            "MIT 2026"
        );
    }

    #[test]
    fn substitute_missing_key_left_intact() {
        assert_eq!(substitute("a {missing} b", &[("x", "y")]), "a {missing} b");
    }

    #[test]
    fn substitute_adjacent() {
        assert_eq!(substitute("{a}{b}", &[("a", "1"), ("b", "2")]), "12");
    }

    #[test]
    fn contains_placeholder_finds() {
        assert!(contains_placeholder("Copyright {year}", "year"));
        assert!(contains_placeholder("{year}{holder}", "year"));
        assert!(!contains_placeholder("no placeholder", "year"));
        assert!(!contains_placeholder("{spdx}", "year"));
    }

    #[test]
    fn extension_uppercase() {
        assert_eq!(normalize_extension("RS"), "rs");
    }

    #[test]
    fn extension_leading_dot() {
        assert_eq!(normalize_extension(".rs"), "rs");
        assert_eq!(normalize_extension(".RS"), "rs");
    }

    #[test]
    fn extension_empty() {
        assert_eq!(normalize_extension(""), "");
        assert_eq!(normalize_extension("."), "");
    }

    #[test]
    fn prefix_empty_lines() {
        // Empty lines become the bare prefix, keeping the block contiguous.
        assert_eq!(prefix_lines("a\n\nb", "// "), "// a\n// \n// b\n");
    }

    #[test]
    fn prefix_no_trailing_newline() {
        assert_eq!(prefix_lines("a", "// "), "// a\n");
    }

    #[test]
    fn prefix_multiline() {
        assert_eq!(prefix_lines("a\nb", "// "), "// a\n// b\n");
        assert_eq!(prefix_lines("", "// "), "");
    }

    #[test]
    fn relative_under_root() {
        assert_eq!(
            relative_display(Path::new("/proj"), Path::new("/proj/src/main.rs")),
            "src/main.rs"
        );
    }

    #[test]
    fn relative_equal_to_root() {
        assert_eq!(relative_display(Path::new("/proj"), Path::new("/proj")), "/proj");
    }

    #[test]
    fn relative_outside_root() {
        assert_eq!(
            relative_display(Path::new("/proj"), Path::new("/other/x")),
            "/other/x"
        );
    }

    #[test]
    fn trim_trailing_newlines_helpers() {
        assert_eq!(trim_trailing_newlines("a\n"), "a");
        assert_eq!(trim_trailing_newlines("a\r\n\n"), "a");
        assert_eq!(trim_trailing_newlines("a\nb\n"), "a\nb");
        assert_eq!(trim_trailing_newlines("a"), "a");
        assert_eq!(trim_trailing_newlines(""), "");
        // Interior newlines are kept.
        assert_eq!(trim_trailing_newlines("a\n\nb"), "a\n\nb");
    }
}
