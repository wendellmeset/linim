// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

//! Find existing license headers in the top window of a file.
//!
//! Purely detection: produces spans and extracted fields, never edits. Uses
//! [`crate::header::preamble::Preamble`] to know where to start scanning and
//! strict qualifying rules to avoid treating doc comments or prose as headers.

use crate::header::comment_map::CommentStyle;
use crate::header::preamble::Preamble;
use crate::helpers;

/// A license header is only a header if it appears within this many lines
/// after the preamble. Deeper than this, it is author content.
const TOP_WINDOW_LINES: usize = 30;

/// License-specific phrases that (with a `Copyright` line) qualify a block.
const LICENSED_PHRASES: [&str; 7] = [
    "All rights reserved",
    "Permission is hereby granted",
    "GNU General Public License",
    "GNU Lesser General Public License",
    "Apache License",
    "Mozilla Public License",
    "Redistribution and use",
];

/// The identified span of a license header in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderBlock {
    /// First line of the block (zero-based, from the top of the file).
    pub start_line: usize,
    /// Last line of the block (inclusive).
    pub end_line: usize,
    /// Byte offset of the block's first byte.
    pub start_byte: usize,
    /// Byte offset one past the block's last byte.
    pub end_byte: usize,
    /// Extracted SPDX identifier, if the block declares one.
    pub spdx: Option<String>,
    /// Extracted copyright line(s), joined, if the block has any.
    pub copyright: Option<String>,
}

/// The outcome of scanning a file's top window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detection {
    /// No qualifying license header found.
    None,
    /// Exactly one qualifying header found.
    Single(HeaderBlock),
    /// Two or more qualifying headers found.
    Ambiguous(Vec<HeaderBlock>),
}

/// Detect license headers in the top window of `buf`.
pub fn detect(buf: &[u8], preamble: &Preamble, style: CommentStyle) -> Detection {
    let mut iter = helpers::lines_upto(buf, buf.len()).peekable();
    // Skip the preamble lines; `preamble.byte_offset` is the start of the
    // first non-preamble line.
    for _ in 0..preamble.line_count {
        let _ = iter.next();
    }
    let mut pos = preamble.byte_offset;
    let mut line_idx = preamble.line_count;
    let window_end = preamble.line_count + TOP_WINDOW_LINES;
    let mut blocks: Vec<HeaderBlock> = Vec::new();

    while line_idx < window_end {
        let Some(content) = iter.next() else { break };
        let start = pos;
        let end = pos + line_len(buf, pos, content);
        pos = end;
        let trimmed = content.trim();

        if trimmed.is_empty() {
            line_idx += 1;
            continue;
        }

        match style {
            CommentStyle::SingleLine(token) => {
                if !trimmed.starts_with(token) {
                    break; // code line: top window is done
                }
                let block_start_line = line_idx;
                let block_start_byte = start;
                let mut inner = vec![comment_text(content, style).unwrap()];
                let mut block_end_line = line_idx;
                let mut block_end_byte = end;

                // Extend while the next line is also a comment (peek, don't
                // consume, so the terminating line is left for the outer loop).
                while let Some(next) = iter.peek() {
                    if !next.trim_start().starts_with(token) {
                        break;
                    }
                    let nc = iter.next().unwrap();
                    let ne = pos + line_len(buf, pos, nc);
                    pos = ne;
                    inner.push(comment_text(nc, style).unwrap());
                    block_end_line += 1;
                    block_end_byte = ne;
                    line_idx += 1;
                }

                if let Some(block) = qualify(
                    &inner,
                    block_start_line,
                    block_end_line,
                    block_start_byte,
                    block_end_byte,
                ) {
                    blocks.push(block);
                }
                line_idx += 1;
            }
            CommentStyle::Block { open, close } => {
                if !content.contains(open) {
                    break; // markup/code: top window is done
                }
                let block_start_line = line_idx;
                let block_start_byte = start;
                let mut inner = vec![comment_text(content, style).unwrap()];
                let mut block_end_line = line_idx;
                let mut block_end_byte = end;
                let mut found_close = content.contains(close);

                if !found_close {
                    while line_idx + 1 < window_end {
                        let Some(nc) = iter.next() else { break };
                        let ne = pos + line_len(buf, pos, nc);
                        pos = ne;
                        inner.push(comment_text(nc, style).unwrap());
                        line_idx += 1;
                        block_end_line = line_idx;
                        block_end_byte = ne;
                        if nc.contains(close) {
                            found_close = true;
                            break;
                        }
                    }
                }

                if !found_close {
                    // Unterminated block: safer to not qualify than to assume
                    // the rest of the file is a comment.
                    break;
                }

                if let Some(block) = qualify(
                    &inner,
                    block_start_line,
                    block_end_line,
                    block_start_byte,
                    block_end_byte,
                ) {
                    blocks.push(block);
                }
                line_idx += 1;
            }
        }
    }

    match blocks.len() {
        0 => Detection::None,
        1 => Detection::Single(blocks.pop().unwrap()),
        _ => Detection::Ambiguous(blocks),
    }
}

/// Total byte length of a line whose content begins at `start`, including its
/// trailing newline.
fn line_len(buf: &[u8], start: usize, content: &str) -> usize {
    let after = start + content.len();
    let nl = if after < buf.len() {
        match buf[after] {
            b'\n' => 1,
            b'\r' if after + 1 < buf.len() && buf[after + 1] == b'\n' => 2,
            _ => 0,
        }
    } else {
        0
    };
    content.len() + nl
}

/// The meaningful comment text of a line: comment token and surrounding
/// whitespace stripped. `None` for a line that is not a comment.
fn comment_text(line: &str, style: CommentStyle) -> Option<&str> {
    match style {
        CommentStyle::SingleLine(token) => line.trim_start().strip_prefix(token).map(str::trim),
        CommentStyle::Block { open, close } => {
            let t = line.trim_start();
            let t = t.strip_prefix(open).unwrap_or(t);
            let t = t.strip_suffix(close).unwrap_or(t);
            Some(t.trim())
        }
    }
}

/// Whether a comment line starts with `Copyright` at a word boundary.
/// Whether a comment line is a copyright line: the REUSE `SPDX-FileCopyrightText:`
/// tag, or `Copyright` at a word boundary.
fn is_copyright_line(line: &str) -> bool {
    if line.starts_with("SPDX-FileCopyrightText:") {
        return true;
    }
    let Some(rest) = line.strip_prefix("Copyright") else {
        return false;
    };
    match rest.chars().next() {
        None => true,
        Some(c) => !c.is_alphanumeric(),
    }
}

/// Build a [`HeaderBlock`] for `inner` if it qualifies under the strict rules.
fn qualify(
    inner: &[&str],
    start_line: usize,
    end_line: usize,
    start_byte: usize,
    end_byte: usize,
) -> Option<HeaderBlock> {
    let has_spdx = inner
        .iter()
        .any(|l| l.starts_with("SPDX-License-Identifier:"));
    let has_copyright = inner.iter().any(|l| is_copyright_line(l));

    let qualifies = if has_spdx {
        true
    } else if has_copyright {
        inner
            .iter()
            .any(|l| LICENSED_PHRASES.iter().any(|p| l.contains(p)))
    } else {
        false
    };

    if !qualifies {
        return None;
    }

    Some(HeaderBlock {
        start_line,
        end_line,
        start_byte,
        end_byte,
        spdx: extract_spdx(inner),
        copyright: extract_copyright(inner),
    })
}

/// Extract the SPDX identifier after `SPDX-License-Identifier:`.
fn extract_spdx(inner: &[&str]) -> Option<String> {
    for line in inner {
        if let Some(rest) = line.strip_prefix("SPDX-License-Identifier:") {
            let id = rest.trim();
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

/// Extract the copyright line(s), joined with newlines.
fn extract_copyright(inner: &[&str]) -> Option<String> {
    let lines: Vec<&str> = inner.iter().copied().filter(|l| is_copyright_line(l)).collect();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pre() -> Preamble {
        Preamble {
            line_count: 0,
            byte_offset: 0,
        }
    }

    #[test]
    fn empty_buffer() {
        assert_eq!(detect(b"", &pre(), CommentStyle::SingleLine("//")), Detection::None);
    }

    #[test]
    fn preamble_only() {
        let buf = b"#!/bin/sh\n";
        let preamble = crate::header::preamble::detect(buf);
        assert_eq!(detect(buf, &preamble, CommentStyle::SingleLine("#")), Detection::None);
    }

    #[test]
    fn code_only() {
        assert_eq!(
            detect(b"fn main() {}\nlet x = 1;\n", &pre(), CommentStyle::SingleLine("//")),
            Detection::None
        );
    }

    #[test]
    fn single_line_no_header() {
        assert_eq!(
            detect(b"pub fn f() {}\n", &pre(), CommentStyle::SingleLine("//")),
            Detection::None
        );
    }

    #[test]
    fn single_line_spdx_header() {
        let buf = b"// SPDX-License-Identifier: MIT\n// Copyright (c) 2026 wendellmeset\n";
        match detect(buf, &pre(), CommentStyle::SingleLine("//")) {
            Detection::Single(b) => {
                assert_eq!((b.start_line, b.end_line), (0, 1));
                assert_eq!(b.spdx.as_deref(), Some("MIT"));
                assert_eq!(b.copyright.as_deref(), Some("Copyright (c) 2026 wendellmeset"));
            }
            other => panic!("expected Single, got {other:?}"),
        }
    }

    #[test]
    fn single_line_old_style_mit() {
        let buf = b"// Copyright (c) 2026 wendellmeset\n//\n// Permission is hereby granted, free of charge, to any person\n// obtaining a copy of this software...\n";
        match detect(buf, &pre(), CommentStyle::SingleLine("//")) {
            Detection::Single(b) => {
                assert_eq!((b.start_line, b.end_line), (0, 3));
                assert_eq!(b.spdx, None);
                assert_eq!(b.copyright.as_deref(), Some("Copyright (c) 2026 wendellmeset"));
            }
            other => panic!("expected Single, got {other:?}"),
        }
    }

    #[test]
    fn doc_comment_then_header() {
        let buf = b"// This module does things.\n// See docs for details.\n\n// SPDX-License-Identifier: MIT\n";
        match detect(buf, &pre(), CommentStyle::SingleLine("//")) {
            Detection::Single(b) => {
                assert_eq!(b.start_line, 3);
                assert_eq!(b.spdx.as_deref(), Some("MIT"));
            }
            other => panic!("expected Single, got {other:?}"),
        }
    }

    #[test]
    fn two_headers_ambiguous() {
        let buf = b"// SPDX-License-Identifier: MIT\n\n// SPDX-License-Identifier: Apache-2.0\n";
        match detect(buf, &pre(), CommentStyle::SingleLine("//")) {
            Detection::Ambiguous(blocks) => {
                assert_eq!(blocks.len(), 2);
                assert_eq!(blocks[0].spdx.as_deref(), Some("MIT"));
                assert_eq!(blocks[1].spdx.as_deref(), Some("Apache-2.0"));
                assert!(blocks[0].start_line < blocks[1].start_line);
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn doc_comment_mentioning_license_not_qualifying() {
        let buf = b"// This project is licensed under the MIT License.\n// See LICENSE for details.\n";
        assert_eq!(detect(buf, &pre(), CommentStyle::SingleLine("//")), Detection::None);
    }

    #[test]
    fn copyrightable_word_boundary() {
        let buf = b"// Copyrightable content here\n";
        assert_eq!(detect(buf, &pre(), CommentStyle::SingleLine("//")), Detection::None);
    }

    #[test]
    fn block_style_no_header() {
        let buf = b"<html>\n<body>hi</body>\n</html>\n";
        assert_eq!(
            detect(buf, &pre(), CommentStyle::Block { open: "<!--", close: "-->" }),
            Detection::None
        );
    }

    #[test]
    fn block_style_license() {
        let buf = b"<!--\nSPDX-License-Identifier: MIT\n-->\n";
        match detect(buf, &pre(), CommentStyle::Block { open: "<!--", close: "-->" }) {
            Detection::Single(b) => {
                assert_eq!((b.start_line, b.end_line), (0, 2));
                assert_eq!(b.spdx.as_deref(), Some("MIT"));
            }
            other => panic!("expected Single, got {other:?}"),
        }
    }

    #[test]
    fn block_style_two_blocks() {
        let buf = b"<!-- SPDX-License-Identifier: MIT -->\n<!-- SPDX-License-Identifier: Apache-2.0 -->\n";
        match detect(buf, &pre(), CommentStyle::Block { open: "<!--", close: "-->" }) {
            Detection::Ambiguous(blocks) => {
                assert_eq!(blocks.len(), 2);
                assert_eq!(blocks[0].spdx.as_deref(), Some("MIT"));
                assert_eq!(blocks[1].spdx.as_deref(), Some("Apache-2.0"));
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn block_style_unterminated() {
        let buf = b"<!-- SPDX-License-Identifier: MIT\n";
        assert_eq!(
            detect(buf, &pre(), CommentStyle::Block { open: "<!--", close: "-->" }),
            Detection::None
        );
    }

    #[test]
    fn header_after_shebang() {
        let buf = b"#!/usr/bin/env python3\n# SPDX-License-Identifier: MIT\n# Copyright (c) 2026 wendellmeset\n";
        let preamble = crate::header::preamble::detect(buf);
        assert_eq!(preamble.line_count, 1);
        match detect(buf, &preamble, CommentStyle::SingleLine("#")) {
            Detection::Single(b) => {
                assert_eq!(b.start_line, 1);
                assert_eq!(b.end_line, 2);
                assert_eq!(b.spdx.as_deref(), Some("MIT"));
            }
            other => panic!("expected Single, got {other:?}"),
        }
    }
}
