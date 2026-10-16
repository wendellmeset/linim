// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

//! Insert a rendered header at the preamble's byte offset.
//!
//! Pure byte manipulation, no I/O. Splices `rendered` into `buf` just after the
//! preamble, inserting a single `\n` first if the preamble has no trailing
//! newline so the header never lands on the same line as e.g. a shebang.

use crate::header::preamble::Preamble;

/// Return a copy of `buf` with `rendered` inserted at `preamble.byte_offset`.
///
/// If the content above the insertion point is non-empty and does not end with
/// a newline (a preamble truncated at EOF), a single `\n` is inserted first.
/// Otherwise no separator is added: the preamble already ends cleanly.
pub fn inject(buf: &[u8], preamble: &Preamble, rendered: &str) -> Vec<u8> {
    let at = preamble.byte_offset.min(buf.len());
    let mut out = Vec::with_capacity(buf.len() + rendered.len() + 1);
    out.extend_from_slice(&buf[..at]);
    if at > 0 && buf[at - 1] != b'\n' {
        out.push(b'\n');
    }
    out.extend_from_slice(rendered.as_bytes());
    out.extend_from_slice(&buf[at..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pre(line_count: usize, byte_offset: usize) -> Preamble {
        Preamble {
            line_count,
            byte_offset,
        }
    }

    const RENDERED: &str = "// SPDX-License-Identifier: MIT\n\n";

    #[test]
    fn no_preamble_header_first() {
        let buf = b"fn main() {}\n";
        assert_eq!(
            inject(buf, &pre(0, 0), RENDERED).as_slice(),
            b"// SPDX-License-Identifier: MIT\n\nfn main() {}\n"
        );
    }

    #[test]
    fn header_after_shebang_lf() {
        let buf = b"#!/bin/sh\nrest\n";
        assert_eq!(
            inject(buf, &pre(1, 10), RENDERED).as_slice(),
            b"#!/bin/sh\n// SPDX-License-Identifier: MIT\n\nrest\n"
        );
    }

    #[test]
    fn header_after_shebang_no_trailing_newline() {
        let buf = b"#!/bin/sh";
        assert_eq!(
            inject(buf, &pre(1, 9), RENDERED).as_slice(),
            b"#!/bin/sh\n// SPDX-License-Identifier: MIT\n\n"
        );
    }

    #[test]
    fn header_after_shebang_crlf() {
        let buf = b"#!/bin/sh\r\nrest\n";
        assert_eq!(
            inject(buf, &pre(1, 11), RENDERED).as_slice(),
            b"#!/bin/sh\r\n// SPDX-License-Identifier: MIT\n\nrest\n"
        );
    }

    #[test]
    fn empty_buffer_gets_header() {
        let buf = b"";
        assert_eq!(
            inject(buf, &pre(0, 0), RENDERED).as_slice(),
            b"// SPDX-License-Identifier: MIT\n\n"
        );
    }

    #[test]
    fn middle_insertion_preserves_below() {
        let buf = b"#!/usr/bin/env python3\n# coding: utf-8\nbody\n";
        let preamble = crate::header::preamble::detect(buf);
        assert_eq!(preamble.line_count, 2);
        let out = inject(buf, &preamble, RENDERED);
        assert_eq!(
            out.as_slice(),
            b"#!/usr/bin/env python3\n# coding: utf-8\n// SPDX-License-Identifier: MIT\n\nbody\n"
        );
    }
}
