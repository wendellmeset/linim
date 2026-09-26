//! Static file-extension → comment-style table and lookup.
//!
//! Pure data plus a single lookup function; no I/O. Extensions are normalized
//! (lowercased, leading dot stripped) before lookup.

use crate::helpers;

/// How source files of a given kind express comments.
///
/// A [`CommentStyle::SingleLine`] token prefixes every line of a comment block;
/// a [`CommentStyle::Block`] opens and closes a single contiguous block.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CommentStyle {
    /// A token like `"//"`, `"#"`, `"--"`, or `";"`.
    SingleLine(&'static str),
    /// A pair like `("/*", "*/")` or `("<!--", "-->" )`.
    Block {
        /// Opening token.
        open: &'static str,
        /// Closing token.
        close: &'static str,
    },
}

/// Static extension → comment-style table, sorted by extension for binary
/// search.
///
/// Rule: if a language supports both single-line and block comments, we use
/// `SingleLine` to keep stripping and detection uniform. `Block` is only used
/// for languages with no single-line token at all (HTML, XML, CSS, ...).
static TABLE: &[(&str, CommentStyle)] = &[
    ("ada", CommentStyle::SingleLine("--")),
    ("asm", CommentStyle::SingleLine(";")),
    ("bash", CommentStyle::SingleLine("#")),
    ("c", CommentStyle::SingleLine("//")),
    ("cc", CommentStyle::SingleLine("//")),
    ("cfg", CommentStyle::SingleLine("#")),
    ("cjs", CommentStyle::SingleLine("//")),
    ("clj", CommentStyle::SingleLine(";")),
    ("cljs", CommentStyle::SingleLine(";")),
    ("conf", CommentStyle::SingleLine("#")),
    ("cpp", CommentStyle::SingleLine("//")),
    ("cr", CommentStyle::SingleLine("#")),
    ("cs", CommentStyle::SingleLine("//")),
    ("css", CommentStyle::Block { open: "/*", close: "*/" }),
    ("cxx", CommentStyle::SingleLine("//")),
    ("dart", CommentStyle::SingleLine("//")),
    ("elm", CommentStyle::SingleLine("--")),
    ("erl", CommentStyle::SingleLine("%")),
    ("ex", CommentStyle::SingleLine("#")),
    ("exs", CommentStyle::SingleLine("#")),
    ("fish", CommentStyle::SingleLine("#")),
    ("go", CommentStyle::SingleLine("//")),
    ("h", CommentStyle::SingleLine("//")),
    ("hpp", CommentStyle::SingleLine("//")),
    ("hrl", CommentStyle::SingleLine("%")),
    ("hs", CommentStyle::SingleLine("--")),
    ("htm", CommentStyle::Block { open: "<!--", close: "-->" }),
    ("html", CommentStyle::Block { open: "<!--", close: "-->" }),
    ("ini", CommentStyle::SingleLine("#")),
    ("java", CommentStyle::SingleLine("//")),
    ("jl", CommentStyle::SingleLine("#")),
    ("js", CommentStyle::SingleLine("//")),
    ("jsx", CommentStyle::SingleLine("//")),
    ("kt", CommentStyle::SingleLine("//")),
    ("kts", CommentStyle::SingleLine("//")),
    ("less", CommentStyle::Block { open: "/*", close: "*/" }),
    ("lisp", CommentStyle::SingleLine(";")),
    ("lua", CommentStyle::SingleLine("--")),
    ("m", CommentStyle::SingleLine("%")),
    ("makefile", CommentStyle::SingleLine("#")),
    ("matlab", CommentStyle::SingleLine("%")),
    ("mjs", CommentStyle::SingleLine("//")),
    ("nim", CommentStyle::SingleLine("#")),
    ("pl", CommentStyle::SingleLine("#")),
    ("pm", CommentStyle::SingleLine("#")),
    ("py", CommentStyle::SingleLine("#")),
    ("r", CommentStyle::SingleLine("#")),
    ("rb", CommentStyle::SingleLine("#")),
    ("rs", CommentStyle::SingleLine("//")),
    ("s", CommentStyle::SingleLine(";")),
    ("scala", CommentStyle::SingleLine("//")),
    ("scss", CommentStyle::Block { open: "/*", close: "*/" }),
    ("sh", CommentStyle::SingleLine("#")),
    ("sql", CommentStyle::SingleLine("--")),
    ("svg", CommentStyle::Block { open: "<!--", close: "-->" }),
    ("swift", CommentStyle::SingleLine("//")),
    ("tex", CommentStyle::SingleLine("%")),
    ("toml", CommentStyle::SingleLine("#")),
    ("ts", CommentStyle::SingleLine("//")),
    ("tsx", CommentStyle::SingleLine("//")),
    ("vue", CommentStyle::SingleLine("//")),
    ("xhtml", CommentStyle::Block { open: "<!--", close: "-->" }),
    ("xml", CommentStyle::Block { open: "<!--", close: "-->" }),
    ("yaml", CommentStyle::SingleLine("#")),
    ("yml", CommentStyle::SingleLine("#")),
    ("zig", CommentStyle::SingleLine("//")),
    ("zsh", CommentStyle::SingleLine("#")),
];

/// Look up the comment style for an extension.
///
/// The raw extension may carry a leading dot and any case; it is normalized
/// before lookup. Returns `None` for unknown extensions (files with no comment
/// syntax, which are skipped entirely).
pub fn comment_style(ext: &str) -> Option<CommentStyle> {
    let ext = helpers::normalize_extension(ext);
    TABLE
        .binary_search_by(|&(name, _)| name.cmp(ext.as_str()))
        .ok()
        .map(|i| TABLE[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted() {
        for pair in TABLE.windows(2) {
            assert!(
                pair[0].0 <= pair[1].0,
                "table out of order: {} after {}",
                pair[1].0,
                pair[0].0
            );
        }
    }

    #[test]
    fn slash_slash_language() {
        assert_eq!(
            comment_style("rs"),
            Some(CommentStyle::SingleLine("//"))
        );
    }

    #[test]
    fn hash_language() {
        assert_eq!(
            comment_style("py"),
            Some(CommentStyle::SingleLine("#"))
        );
    }

    #[test]
    fn dash_dash_language() {
        assert_eq!(
            comment_style("lua"),
            Some(CommentStyle::SingleLine("--"))
        );
    }

    #[test]
    fn block_language() {
        assert_eq!(
            comment_style("html"),
            Some(CommentStyle::Block { open: "<!--", close: "-->" })
        );
    }

    #[test]
    fn uppercase_extension() {
        assert_eq!(
            comment_style("RS"),
            Some(CommentStyle::SingleLine("//"))
        );
    }

    #[test]
    fn leading_dot_extension() {
        assert_eq!(
            comment_style(".rs"),
            Some(CommentStyle::SingleLine("//"))
        );
    }

    #[test]
    fn unknown_extension() {
        assert_eq!(comment_style("xyz"), None);
        assert_eq!(comment_style(""), None);
    }
}
