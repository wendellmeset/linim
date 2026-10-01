//! Detect the "preamble": leading lines that must stay above any injected
//! license header (shebangs, encoding cookies, XML/PHP/doctype/charset lines).
//!
//! Pure and allocation-free: operates on a byte slice the caller already read,
//! iterating only the first line or two. No I/O, no `CommentStyle` knowledge,
//! no license-header detection.

use crate::helpers;

/// The detected preamble at the top of a file.
pub struct Preamble {
    /// How many leading lines are preamble.
    pub line_count: usize,
    /// Byte index where the header should be inserted (just after the preamble).
    pub byte_offset: usize,
}

/// Detect the preamble in `buf`.
///
/// Only the first two lines are ever inspected. Line 1 may be a shebang,
/// Python encoding cookie, XML declaration, PHP tag, `@charset`, or HTML
/// doctype. A shebang on line 1 may additionally stack with a Python encoding
/// cookie on line 2.
pub fn detect(buf: &[u8]) -> Preamble {
    let mut lines = helpers::lines_upto(buf, buf.len());
    let mut line_count = 0;
    let mut byte_offset = 0;
    let mut pos = 0;

    let Some(line1) = lines.next() else {
        return Preamble {
            line_count,
            byte_offset,
        };
    };

    let end1 = line_end(buf, pos, line1.len());
    let shebang = line1.starts_with("#!");
    let is_preamble = shebang
        || is_encoding_cookie(line1)
        || line1.starts_with("<?xml")
        || line1.starts_with("<?php")
        || line1.starts_with("@charset")
        || starts_with_doctype(line1);

    if !is_preamble {
        return Preamble {
            line_count,
            byte_offset,
        };
    }

    line_count += 1;
    byte_offset = end1;
    pos = end1;

    if shebang
        && let Some(line2) = lines.next()
        && is_encoding_cookie(line2)
    {
        let end2 = line_end(buf, pos, line2.len());
        line_count += 1;
        byte_offset = end2;
    }

    Preamble {
        line_count,
        byte_offset,
    }
}

/// Byte offset just past the line whose content begins at `start` and spans
/// `content_len` bytes, including its trailing newline (`\n` or `\r\n`). A
/// line with no trailing newline ends exactly at `start + content_len`.
fn line_end(buf: &[u8], start: usize, content_len: usize) -> usize {
    let after = start + content_len;
    after + newline_len(buf, after)
}

/// Length of the newline starting at `at`, or 0 if there is none.
fn newline_len(buf: &[u8], at: usize) -> usize {
    if at >= buf.len() {
        return 0;
    }
    match buf[at] {
        b'\n' => 1,
        b'\r' if at + 1 < buf.len() && buf[at + 1] == b'\n' => 2,
        _ => 0,
    }
}

/// Whether `line` is a PEP 263 encoding cookie: optional leading whitespace,
/// then `#`, then `coding` immediately followed by `:` or `=`.
///
/// Catches both `# -*- coding: utf-8 -*-` and `# coding: utf-8` without a
/// regex.
fn is_encoding_cookie(line: &str) -> bool {
    let line = line.trim_start_matches([' ', '\t', '\u{0c}']);
    let Some(rest) = line.strip_prefix('#') else {
        return false;
    };
    let bytes = rest.as_bytes();
    const CODING: &[u8] = b"coding";
    let mut i = 0;
    while i + CODING.len() <= bytes.len() {
        if &bytes[i..i + CODING.len()] == CODING
            && let Some(&next) = bytes.get(i + CODING.len())
            && (next == b':' || next == b'=')
        {
            return true;
        }
        i += 1;
    }
    false
}

/// Whether `line` starts with `<!doctype` case-insensitively, without
/// allocating.
fn starts_with_doctype(line: &str) -> bool {
    const DOCTYPE: &[u8] = b"<!doctype";
    let b = line.as_bytes();
    b.len() >= DOCTYPE.len()
        && b[..DOCTYPE.len()]
            .iter()
            .zip(DOCTYPE)
            .all(|(x, y)| x.eq_ignore_ascii_case(y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer() {
        let p = detect(b"");
        assert_eq!((p.line_count, p.byte_offset), (0, 0));
    }

    #[test]
    fn plain_code_no_preamble() {
        let p = detect(b"fn main() {}\n");
        assert_eq!((p.line_count, p.byte_offset), (0, 0));
    }

    #[test]
    fn shebang_lf() {
        let p = detect(b"#!/bin/sh\nrest\n");
        assert_eq!((p.line_count, p.byte_offset), (1, 10));
    }

    #[test]
    fn shebang_crlf() {
        let p = detect(b"#!/bin/sh\r\nrest\n");
        assert_eq!((p.line_count, p.byte_offset), (1, 11));
    }

    #[test]
    fn shebang_no_trailing_newline() {
        let buf = b"#!/bin/sh";
        let p = detect(buf);
        assert_eq!((p.line_count, p.byte_offset), (1, buf.len()));
    }

    #[test]
    fn shebang_plus_encoding_cookie_line2() {
        let p = detect(b"#!/usr/bin/env python3\n# coding: utf-8\nbody\n");
        assert_eq!((p.line_count, p.byte_offset), (2, 39));
    }

    #[test]
    fn encoding_cookie_line1_no_shebang() {
        let p = detect(b"# coding: utf-8\nbody\n");
        assert_eq!((p.line_count, p.byte_offset), (1, 16));
    }

    #[test]
    fn encoding_cookie_emacs_style() {
        let p = detect(b"# -*- coding: utf-8 -*-\nbody\n");
        assert_eq!((p.line_count, p.byte_offset), (1, 24));
    }

    #[test]
    fn plain_comment_not_cookie() {
        let p = detect(b"# This is a comment\n");
        assert_eq!((p.line_count, p.byte_offset), (0, 0));
    }

    #[test]
    fn shebang_then_plain_comment_not_stacked() {
        let p = detect(b"#!/usr/bin/env python3\n# a comment\nbody\n");
        assert_eq!((p.line_count, p.byte_offset), (1, 23));
    }

    #[test]
    fn xml_declaration() {
        let p = detect(b"<?xml version=\"1.0\"?>\nbody\n");
        assert_eq!((p.line_count, p.byte_offset), (1, 22));
    }

    #[test]
    fn php_tag() {
        let p = detect(b"<?php\nbody\n");
        assert_eq!((p.line_count, p.byte_offset), (1, 6));
    }

    #[test]
    fn css_charset() {
        let p = detect(b"@charset \"UTF-8\";\nbody\n");
        assert_eq!((p.line_count, p.byte_offset), (1, 18));
    }

    #[test]
    fn html_doctype_uppercase() {
        let p = detect(b"<!DOCTYPE html>\nbody\n");
        assert_eq!((p.line_count, p.byte_offset), (1, 16));
    }

    #[test]
    fn html_doctype_lowercase() {
        let p = detect(b"<!doctype html>\nbody\n");
        assert_eq!((p.line_count, p.byte_offset), (1, 16));
    }
}
