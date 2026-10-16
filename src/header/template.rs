// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

//! Build the canonical linim header and render it as a comment block.
//!
//! Pure: primitives in, strings out. The template is built once at startup and
//! shared across all file processing. No preamble logic, detection, stripping,
//! injection, or file I/O here.

use crate::header::comment_map::CommentStyle;
use crate::helpers;

/// The canonical header content template, `{spdx}`, `{year}`, `{holder}`
/// substituted at build time. Copyright first, then the SPDX identifier, per
/// the REUSE software spec.
const CONTENT_TEMPLATE: &str =
    "SPDX-FileCopyrightText: {year} {holder}\nSPDX-License-Identifier: {spdx}";
const SPDX_TEMPLATE: &str = "SPDX-License-Identifier: {spdx}";
const COPYRIGHT_TEMPLATE: &str = "SPDX-FileCopyrightText: {year} {holder}";

/// The expected header: content plus the two lines extracted for comparison.
pub struct HeaderTemplate {
    /// The two-line body, e.g. `"SPDX-FileCopyrightText: 2026 wendellmeset\nSPDX-License-Identifier: MIT"`.
    pub content: String,
    /// Just the SPDX line, for comparison and reporting.
    pub spdx_line: String,
    /// Just the SPDX identifier value (no prefix), e.g. `"MIT"`.
    pub spdx_value: String,
    /// Just the copyright line, for comparison and reporting.
    pub copyright_line: String,
}

/// Build a header template from the resolved SPDX identifier, holder, and year.
pub fn build(spdx: &str, holder: &str, year: u16) -> HeaderTemplate {
    let year = helpers::format_year(year);
    let vars = [("spdx", spdx), ("holder", holder), ("year", year.as_str())];
    HeaderTemplate {
        content: helpers::substitute(CONTENT_TEMPLATE, &vars),
        spdx_line: helpers::substitute(SPDX_TEMPLATE, &vars),
        spdx_value: spdx.to_string(),
        copyright_line: helpers::substitute(COPYRIGHT_TEMPLATE, &vars),
    }
}

impl HeaderTemplate {
    /// Render the header content into a comment block for `style`, with a
    /// trailing blank line separating it from the code that follows.
    pub fn render(&self, style: CommentStyle) -> String {
        match style {
            CommentStyle::SingleLine(token) => {
                let prefix = format!("{token} ");
                let mut out = helpers::prefix_lines(&self.content, &prefix);
                out.push('\n'); // trailing blank line
                out
            }
            CommentStyle::Block { open, close } => {
                format!("{open}\n{}\n{close}\n\n", self.content)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_produces_expected_fields() {
        let t = build("MIT", "wendellmeset", 2026);
        assert_eq!(
            t.content,
            "SPDX-FileCopyrightText: 2026 wendellmeset\nSPDX-License-Identifier: MIT"
        );
        assert_eq!(t.spdx_line, "SPDX-License-Identifier: MIT");
        assert_eq!(t.spdx_value, "MIT");
        assert_eq!(t.copyright_line, "SPDX-FileCopyrightText: 2026 wendellmeset");
    }

    #[test]
    fn render_single_line_slash() {
        let t = build("MIT", "wendellmeset", 2026);
        assert_eq!(
            t.render(CommentStyle::SingleLine("//")),
            "// SPDX-FileCopyrightText: 2026 wendellmeset\n// SPDX-License-Identifier: MIT\n\n"
        );
    }

    #[test]
    fn render_single_line_hash_uses_space() {
        let t = build("MIT", "wendellmeset", 2026);
        let out = t.render(CommentStyle::SingleLine("#"));
        assert!(out.starts_with("# SPDX-FileCopyrightText: 2026 wendellmeset\n# SPDX-License-Identifier: MIT"));
    }

    #[test]
    fn render_block_is_well_formed() {
        let t = build("MIT", "wendellmeset", 2026);
        assert_eq!(
            t.render(CommentStyle::Block { open: "<!--", close: "-->" }),
            "<!--\nSPDX-FileCopyrightText: 2026 wendellmeset\nSPDX-License-Identifier: MIT\n-->\n\n"
        );
    }

    #[test]
    fn render_empty_holder_does_not_crash() {
        let t = build("MIT", "", 2026);
        assert_eq!(t.content, "SPDX-FileCopyrightText: 2026 \nSPDX-License-Identifier: MIT");
        let out = t.render(CommentStyle::SingleLine("//"));
        assert!(out.contains("SPDX-License-Identifier: MIT"));
        assert!(out.contains("SPDX-FileCopyrightText: 2026"));
        assert!(out.ends_with("\n\n"));
    }

    #[test]
    fn render_is_idempotent() {
        let t = build("GPL-3.0-or-later", "wendellmeset", 2020);
        let style = CommentStyle::SingleLine("--");
        assert_eq!(t.render(style), t.render(style));
    }
}
