// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

//! Substitute placeholders into a license body to produce the final `LICENSE`
//! text.
//!
//! The registry (`license/registry.rs`) is the source of truth for which
//! placeholders a body supports. This module just turns `{year}` and `{holder}`
//! into values, returning bodies with no placeholders verbatim.

use crate::helpers;
use crate::license::registry::{LicenseEntry, Placeholder};

/// Build the final license text with `{year}` and `{holder}` substituted per
/// `entry.placeholders`.
pub fn build(entry: &LicenseEntry, holder: &str, year: u16) -> String {
    let year_str = year.to_string();
    let mut vars: Vec<(&str, &str)> = Vec::with_capacity(entry.placeholders.len());
    for p in entry.placeholders {
        match p {
            Placeholder::Year => vars.push(("year", year_str.as_str())),
            Placeholder::Holder => vars.push(("holder", holder)),
        }
    }
    helpers::substitute(entry.body, &vars)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::registry;

    #[test]
    fn mit_substitutes_both() {
        let entry = registry::by_spdx("MIT").unwrap();
        let text = build(entry, "wendellmeset", 2026);
        assert!(text.contains("Copyright (c) 2026 wendellmeset"));
        assert!(!text.contains("{year}"));
        assert!(!text.contains("{holder}"));
    }

    #[test]
    fn zero_bsd_substitutes() {
        let entry = registry::by_spdx("0BSD").unwrap();
        let text = build(entry, "wendellmeset", 2026);
        assert!(text.contains("Copyright (c) 2026 wendellmeset"));
        assert!(!text.contains("{year}"));
        assert!(!text.contains("{holder}"));
    }

    #[test]
    fn apache_substitutes() {
        let entry = registry::by_spdx("Apache-2.0").unwrap();
        let text = build(entry, "wendellmeset", 2026);
        assert!(text.contains("Copyright 2026 wendellmeset"));
        assert!(!text.contains("{year}"));
        assert!(!text.contains("{holder}"));
    }

    #[test]
    fn gpl_returned_verbatim() {
        let entry = registry::by_spdx("GPL-3.0-only").unwrap();
        let text = build(entry, "wendellmeset", 2026);
        assert_eq!(text, entry.body);
        assert!(!text.contains("{year}"));
        assert!(!text.contains("{holder}"));
        assert!(!text.contains("wendellmeset"));
    }

    #[test]
    fn unlicense_returned_verbatim() {
        let entry = registry::by_spdx("Unlicense").unwrap();
        let text = build(entry, "wendellmeset", 2026);
        assert_eq!(text, entry.body);
        assert!(!text.contains("{year}"));
        assert!(!text.contains("{holder}"));
    }

    #[test]
    fn no_placeholders_remain_for_any_entry() {
        for entry in registry::LICENSES {
            let text = build(entry, "wendellmeset", 2026);
            assert!(
                !text.contains("{year}"),
                "{{year}} left in {}",
                entry.spdx
            );
            assert!(
                !text.contains("{holder}"),
                "{{holder}} left in {}",
                entry.spdx
            );
        }
    }
}
