//! Generate the static extension → comment-style table for linim from tokei's
//! `languages.json`.
//!
//! Dev-only tool: reads `data/tokei_languages.json`, prints tuple lines to
//! stdout, and never writes files. Not run by build or CI. Usage:
//!
//! ```text
//! cargo run --manifest-path tools/gen-comment-map/Cargo.toml -- [path]
//! ```
//!
//! The optional path defaults to `data/tokei_languages.json` (relative to the
//! working directory). The emitted tuples paste directly into
//! `src/header/comment_map.rs`.

use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::io::{self, Write};

use serde::Deserialize;

/// A comment style with owned tokens (from the JSON).
#[derive(Clone, Debug, PartialEq)]
enum Style {
    SingleLine(String),
    Block { open: String, close: String },
}

/// Const-friendly override style (same shape, `&'static str` tokens).
#[derive(Clone, Copy, Debug, PartialEq)]
enum OverrideStyle {
    SingleLine(&'static str),
    Block { open: &'static str, close: &'static str },
}

impl From<OverrideStyle> for Style {
    fn from(o: OverrideStyle) -> Style {
        match o {
            OverrideStyle::SingleLine(t) => Style::SingleLine(t.to_string()),
            OverrideStyle::Block { open, close } => Style::Block {
                open: open.to_string(),
                close: close.to_string(),
            },
        }
    }
}

/// Corrections to tokei's data. `Some` forces a style (or adds a missing ext);
/// `None` drops the extension entirely.
const OVERRIDES: &[(&str, Option<OverrideStyle>)] = &[
    // tokei says CSS line comments are `//`, which is not valid CSS.
    ("css", Some(OverrideStyle::Block { open: "/*", close: "*/" })),
    // Generic config files, not in tokei.
    ("conf", Some(OverrideStyle::SingleLine("#"))),
    // MATLAB has no tokei entry; `.matlab` gets `%`.
    ("matlab", Some(OverrideStyle::SingleLine("%"))),
    // PHP's dominant comment style is `//` (tokei picks `#`).
    ("php", Some(OverrideStyle::SingleLine("//"))),
    // XHTML, not in tokei; same comment syntax as HTML.
    ("xhtml", Some(OverrideStyle::Block { open: "<!--", close: "-->" })),
    // Emoji junk extensions present in tokei's data.
    ("🍇", None),
    ("🔥", None),
];

/// One tokei language entry. Unknown fields are ignored.
#[derive(Deserialize)]
struct Lang {
    #[serde(default)]
    line_comment: Vec<String>,
    #[serde(default)]
    multi_line_comments: Vec<Vec<String>>,
    #[serde(default)]
    extensions: Vec<String>,
}

#[derive(Deserialize)]
struct TokeiFile {
    languages: HashMap<String, Lang>,
}

fn style_of(lang: &Lang) -> Option<Style> {
    if let Some(tok) = lang.line_comment.first() {
        if !tok.is_empty() {
            return Some(Style::SingleLine(tok.clone()));
        }
    }
    if let Some(pair) = lang.multi_line_comments.first() {
        if pair.len() >= 2 {
            return Some(Style::Block {
                open: pair[0].clone(),
                close: pair[1].clone(),
            });
        }
    }
    None
}

fn rust_literal(s: &str) -> String {
    format!("{s:?}")
}

fn emit(style: &Style) -> String {
    match style {
        Style::SingleLine(tok) => format!("CommentStyle::SingleLine({})", rust_literal(tok)),
        Style::Block { open, close } => format!(
            "CommentStyle::Block {{ open: {}, close: {} }}",
            rust_literal(open),
            rust_literal(close)
        ),
    }
}

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/tokei_languages.json".to_string());
    let data = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
    let tokei: TokeiFile =
        serde_json::from_str(&data).unwrap_or_else(|e| panic!("failed to parse {path}: {e}"));

    if tokei.languages.len() <= 300 {
        eprintln!(
            "warning: expected >300 languages, found {} (schema drift?)",
            tokei.languages.len()
        );
    }

    // ext -> (style, source lang name). Iterating languages in sorted name
    // order makes the first insert the alphabetically-first language.
    let mut map: BTreeMap<String, (Style, String)> = BTreeMap::new();
    let mut lang_names: Vec<&String> = tokei.languages.keys().collect();
    lang_names.sort();
    for name in lang_names {
        let lang = &tokei.languages[name];
        let Some(style) = style_of(lang) else {
            continue;
        };
        for ext in &lang.extensions {
            let ext = ext.to_lowercase();
            if let Some((existing, _)) = map.get(&ext) {
                if *existing != style {
                    eprintln!(
                        "warning: {ext} claimed by both {name} and another language with different styles; keeping first"
                    );
                }
                continue;
            }
            map.insert(ext, (style.clone(), name.clone()));
        }
    }

    // Apply overrides after generation so they always win.
    for (ext, override_style) in OVERRIDES {
        match override_style {
            Some(s) => {
                map.insert((*ext).to_string(), (Style::from(*s), "override".to_string()));
            }
            None => {
                map.remove(*ext);
            }
        }
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();
    for (ext, (style, src)) in &map {
        let _ = writeln!(out, "    ({}, {}),", rust_literal(ext), emit(style));
        let _ = src; // source recorded only for warnings/debugging
    }
    eprintln!(
        "generated {} entries from {} languages",
        map.len(),
        tokei.languages.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang(line: Vec<&str>, multi: Vec<[&str; 2]>) -> Lang {
        Lang {
            line_comment: line.into_iter().map(str::to_string).collect(),
            multi_line_comments: multi
                .into_iter()
                .map(|[a, b]| vec![a.to_string(), b.to_string()])
                .collect(),
            extensions: Vec::new(),
        }
    }

    #[test]
    fn single_line_preferred_over_block() {
        let l = lang(vec!["//"], vec![["/*", "*/"]]);
        assert_eq!(style_of(&l), Some(Style::SingleLine("//".to_string())));
    }

    #[test]
    fn block_when_no_single_line() {
        let l = lang(vec![], vec![["<!--", "-->"]]);
        assert_eq!(
            style_of(&l),
            Some(Style::Block { open: "<!--".into(), close: "-->".into() })
        );
    }

    #[test]
    fn no_style_skipped() {
        let l = lang(vec![], vec![]);
        assert_eq!(style_of(&l), None);
    }

    #[test]
    fn overrides_include_css_conf_matlab_php_xhtml() {
        let forced: Vec<&str> = OVERRIDES
            .iter()
            .filter(|(_, opt)| opt.is_some())
            .map(|(ext, _)| *ext)
            .collect();
        for want in ["css", "conf", "matlab", "php", "xhtml"] {
            assert!(forced.contains(&want), "{want} should force a style");
        }
        // Emoji junk extensions are dropped, not forced.
        assert!(OVERRIDES.contains(&("🍇", None)));
        assert!(OVERRIDES.contains(&("🔥", None)));
    }
}
