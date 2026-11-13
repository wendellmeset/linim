// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

//! Insert a rendered header at the preamble's byte offset.
//!
//! Pure byte manipulation, no I/O. Splices `rendered` into `buf` just after the
//! preamble, inserting a single newline first if the preamble has no trailing
//! newline so the header never lands on the same line as e.g. a shebang. The
//! header, and that separator, use the file's line ending (LF or CRLF).

use crate::header::preamble::Preamble;
use crate::helpers::LineEnding;

/// Return a copy of `buf` with `rendered` inserted at `preamble.byte_offset`.
///
/// `rendered` is LF-only; it is re-encoded with `ending` so the result matches
/// the file's existing line endings. If the content above the insertion point
/// is non-empty and does not end with a newline (a preamble truncated at EOF),
/// one `ending` newline is inserted first. Otherwise no separator is added:
/// the preamble already ends cleanly.
pub fn inject(buf: &[u8], preamble: &Preamble, rendered: &str, ending: LineEnding) -> Vec<u8> {
    let at = preamble.byte_offset.min(buf.len());
    let mut out = Vec::with_capacity(buf.len() + rendered.len() + 8);
    out.extend_from_slice(&buf[..at]);
    if preamble.line_count > 0 && at > 0 && buf[at - 1] != b'\n' {
        out.extend_from_slice(ending.as_bytes());
    }
    ending.encode_into(&mut out, rendered);
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
            inject(buf, &pre(0, 0), RENDERED, LineEnding::Lf).as_slice(),
            b"// SPDX-License-Identifier: MIT\n\nfn main() {}\n"
        );
    }

    #[test]
    fn header_after_shebang_lf() {
        let buf = b"#!/bin/sh\nrest\n";
        assert_eq!(
            inject(buf, &pre(1, 10), RENDERED, LineEnding::Lf).as_slice(),
            b"#!/bin/sh\n// SPDX-License-Identifier: MIT\n\nrest\n"
        );
    }

    #[test]
    fn header_after_shebang_no_trailing_newline() {
        let buf = b"#!/bin/sh";
        assert_eq!(
            inject(buf, &pre(1, 9), RENDERED, LineEnding::Lf).as_slice(),
            b"#!/bin/sh\n// SPDX-License-Identifier: MIT\n\n"
        );
    }

    #[test]
    fn empty_buffer_gets_header() {
        let buf = b"";
        assert_eq!(
            inject(buf, &pre(0, 0), RENDERED, LineEnding::Lf).as_slice(),
            b"// SPDX-License-Identifier: MIT\n\n"
        );
    }

    #[test]
    fn middle_insertion_preserves_below() {
        let buf = b"#!/usr/bin/env python3\n# coding: utf-8\nbody\n";
        let preamble = crate::header::preamble::detect(buf);
        assert_eq!(preamble.line_count, 2);
        let out = inject(buf, &preamble, RENDERED, LineEnding::Lf);
        assert_eq!(
            out.as_slice(),
            b"#!/usr/bin/env python3\n# coding: utf-8\n// SPDX-License-Identifier: MIT\n\nbody\n"
        );
    }

    #[test]
    fn crlf_header_uses_crlf() {
        let buf = b"fn main() {}\r\n";
        assert_eq!(
            inject(buf, &pre(0, 0), RENDERED, LineEnding::Crlf).as_slice(),
            b"// SPDX-License-Identifier: MIT\r\n\r\nfn main() {}\r\n"
        );
    }

    #[test]
    fn crlf_header_after_shebang() {
        let buf = b"#!/bin/sh\r\nrest\r\n";
        assert_eq!(
            inject(buf, &pre(1, 11), RENDERED, LineEnding::Crlf).as_slice(),
            b"#!/bin/sh\r\n// SPDX-License-Identifier: MIT\r\n\r\nrest\r\n"
        );
    }

    #[test]
    fn crlf_separator_for_preamble_without_newline() {
        let buf = b"#!/bin/sh";
        assert_eq!(
            inject(buf, &pre(1, 9), RENDERED, LineEnding::Crlf).as_slice(),
            b"#!/bin/sh\r\n// SPDX-License-Identifier: MIT\r\n\r\n"
        );
    }

    #[test]
    fn bom_only_gets_no_separator() {
        let buf = b"\xEF\xBB\xBFusing System;\n";
        assert_eq!(
            inject(buf, &pre(0, 3), RENDERED, LineEnding::Lf).as_slice(),
            b"\xEF\xBB\xBF// SPDX-License-Identifier: MIT\n\nusing System;\n"
        );
    }

    #[test]
    fn bom_only_gets_no_separator_crlf() {
        let buf = b"\xEF\xBB\xBFusing System;\r\n";
        assert_eq!(
            inject(buf, &pre(0, 3), RENDERED, LineEnding::Crlf).as_slice(),
            b"\xEF\xBB\xBF// SPDX-License-Identifier: MIT\r\n\r\nusing System;\r\n"
        );
    }
}
