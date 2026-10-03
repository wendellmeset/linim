//! Remove a detected license-header block from a buffer.
//!
//! Pure byte manipulation, no I/O. Removes the block's span plus any trailing
//! blank lines, so a replacement inserted later yields exactly one blank line
//! between header and code. Content above the block is never touched.

use crate::header::detect::HeaderBlock;

/// Return a copy of `buf` with `block` removed, including any trailing blank
/// lines up to (but not including) the first non-blank line.
pub fn strip(buf: &[u8], block: &HeaderBlock) -> Vec<u8> {
    let cut_start = block.start_byte.min(buf.len());
    let mut cut_end = block.end_byte.min(buf.len());

    // Consume whitespace-only lines immediately following the block.
    while cut_end < buf.len() {
        let rest = &buf[cut_end..];
        let line_end = rest
            .iter()
            .position(|&b| b == b'\n')
            .map(|i| cut_end + i)
            .unwrap_or(buf.len());
        let content = &buf[cut_end..line_end];
        if content.iter().all(|b| b.is_ascii_whitespace()) {
            // Blank line: consume it and its newline (or to EOF if none).
            cut_end = if line_end < buf.len() {
                line_end + 1
            } else {
                buf.len()
            };
        } else {
            break;
        }
    }

    let mut out = Vec::with_capacity(buf.len() - (cut_end - cut_start));
    out.extend_from_slice(&buf[..cut_start]);
    out.extend_from_slice(&buf[cut_end..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(start_byte: usize, end_byte: usize) -> HeaderBlock {
        HeaderBlock {
            start_line: 0,
            end_line: 0,
            start_byte,
            end_byte,
            spdx: None,
            copyright: None,
        }
    }

    #[test]
    fn block_at_start_code_below() {
        let header = b"// SPDX-License-Identifier: MIT\n";
        let buf = b"// SPDX-License-Identifier: MIT\nfn main() {}\n";
        assert_eq!(
            strip(buf, &block(0, header.len())).as_slice(),
            b"fn main() {}\n"
        );
    }

    #[test]
    fn block_after_shebang_preserves_shebang() {
        let shebang = b"#!/bin/sh\n";
        let header = b"// SPDX-License-Identifier: MIT\n";
        let buf = b"#!/bin/sh\n// SPDX-License-Identifier: MIT\nrest\n";
        assert_eq!(
            strip(buf, &block(shebang.len(), shebang.len() + header.len())).as_slice(),
            b"#!/bin/sh\nrest\n"
        );
    }

    #[test]
    fn block_followed_by_one_blank_line() {
        let header = b"// SPDX-License-Identifier: MIT\n";
        let buf = b"// SPDX-License-Identifier: MIT\n\nfn main() {}\n";
        assert_eq!(
            strip(buf, &block(0, header.len())).as_slice(),
            b"fn main() {}\n"
        );
    }

    #[test]
    fn block_followed_by_three_blank_lines() {
        let header = b"// SPDX-License-Identifier: MIT\n";
        let buf = b"// SPDX-License-Identifier: MIT\n\n\n\nfn main() {}\n";
        assert_eq!(
            strip(buf, &block(0, header.len())).as_slice(),
            b"fn main() {}\n"
        );
    }

    #[test]
    fn block_followed_immediately_by_code() {
        let header = b"// SPDX-License-Identifier: MIT\n";
        let buf = b"// SPDX-License-Identifier: MIT\nfn main() {}\n";
        let out = strip(buf, &block(0, header.len()));
        assert_eq!(out.as_slice(), b"fn main() {}\n");
        assert!(out.starts_with(b"fn"));
    }

    #[test]
    fn block_is_entire_file() {
        let buf = b"// SPDX-License-Identifier: MIT\n// Copyright (c) 2026 wendellmeset\n";
        assert_eq!(strip(buf, &block(0, buf.len())).as_slice(), b"");
    }

    #[test]
    fn block_no_trailing_newline_at_eof() {
        let buf = b"// SPDX-License-Identifier: MIT";
        assert_eq!(strip(buf, &block(0, buf.len())).as_slice(), b"");
    }

    #[test]
    fn block_last_content_with_trailing_newline() {
        let buf = b"// SPDX-License-Identifier: MIT\n";
        assert_eq!(strip(buf, &block(0, buf.len())).as_slice(), b"");
    }

    #[test]
    fn content_above_block_untouched() {
        let shebang = b"#!/usr/bin/env python3\n";
        let header = b"# SPDX-License-Identifier: MIT\n";
        let buf = b"#!/usr/bin/env python3\n# SPDX-License-Identifier: MIT\n\n\nbody\n";
        assert_eq!(
            strip(buf, &block(shebang.len(), shebang.len() + header.len())).as_slice(),
            b"#!/usr/bin/env python3\nbody\n"
        );
    }
}
