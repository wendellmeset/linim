// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

//! Embedded SPDX license registry: pure data plus lookup.
//!
//! No I/O and no per-run state: the registry is immutable data accessed by
//! reference. It only decides what the root `LICENSE` body looks like; file
//! header policy lives in `header/template.rs` and is independent.

use crate::error::{LinimError, Result};

/// A substitution the body of a license supports.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Placeholder {
    /// A `{year}` marker in the body.
    Year,
    /// A `{holder}` marker in the body.
    Holder,
}

/// One entry in the embedded SPDX registry.
pub struct LicenseEntry {
    /// The SPDX identifier, e.g. `"MIT"`.
    pub spdx: &'static str,
    /// Human-readable name, e.g. `"MIT License"`.
    pub name: &'static str,
    /// The verbatim license text.
    pub body: &'static str,
    /// Which substitutions apply to the body.
    pub placeholders: &'static [Placeholder],
    /// Whether the picker should surface this near the top.
    pub popular: bool,
}

/// The embedded registry. Popular licenses first (pick order), then the
/// remaining entries alphabetically so diffs stay stable as entries grow.
pub static LICENSES: &[LicenseEntry] = &[
    LicenseEntry {
        spdx: "MIT",
        name: "MIT License",
        body: include_str!("../../data/licenses/MIT.txt"),
        placeholders: &[Placeholder::Year, Placeholder::Holder],
        popular: true,
    },
    LicenseEntry {
        spdx: "Apache-2.0",
        name: "Apache License 2.0",
        body: include_str!("../../data/licenses/Apache-2.0.txt"),
        placeholders: &[Placeholder::Year, Placeholder::Holder],
        popular: true,
    },
    LicenseEntry {
        spdx: "GPL-3.0-only",
        name: "GNU General Public License v3.0 only",
        body: include_str!("../../data/licenses/GPL-3.0-only.txt"),
        placeholders: &[],
        popular: true,
    },
    LicenseEntry {
        spdx: "GPL-3.0-or-later",
        name: "GNU General Public License v3.0 or later",
        body: include_str!("../../data/licenses/GPL-3.0-or-later.txt"),
        placeholders: &[],
        popular: true,
    },
    LicenseEntry {
        spdx: "BSD-3-Clause",
        name: "BSD 3-Clause License",
        body: include_str!("../../data/licenses/BSD-3-Clause.txt"),
        placeholders: &[Placeholder::Year, Placeholder::Holder],
        popular: true,
    },
    LicenseEntry {
        spdx: "MPL-2.0",
        name: "Mozilla Public License 2.0",
        body: include_str!("../../data/licenses/MPL-2.0.txt"),
        placeholders: &[],
        popular: true,
    },
    LicenseEntry {
        spdx: "ISC",
        name: "ISC License",
        body: include_str!("../../data/licenses/ISC.txt"),
        placeholders: &[Placeholder::Year, Placeholder::Holder],
        popular: true,
    },
    LicenseEntry {
        spdx: "Unlicense",
        name: "The Unlicense",
        body: include_str!("../../data/licenses/Unlicense.txt"),
        placeholders: &[],
        popular: true,
    },
    LicenseEntry {
        spdx: "0BSD",
        name: "BSD Zero Clause License",
        body: include_str!("../../data/licenses/0BSD.txt"),
        placeholders: &[Placeholder::Year, Placeholder::Holder],
        popular: true,
    },
    LicenseEntry {
        spdx: "AGPL-3.0-only",
        name: "GNU Affero General Public License v3.0 only",
        body: include_str!("../../data/licenses/AGPL-3.0-only.txt"),
        placeholders: &[],
        popular: false,
    },
    LicenseEntry {
        spdx: "AGPL-3.0-or-later",
        name: "GNU Affero General Public License v3.0 or later",
        body: include_str!("../../data/licenses/AGPL-3.0-or-later.txt"),
        placeholders: &[],
        popular: false,
    },
    LicenseEntry {
        spdx: "BSD-2-Clause",
        name: "BSD 2-Clause License",
        body: include_str!("../../data/licenses/BSD-2-Clause.txt"),
        placeholders: &[Placeholder::Year, Placeholder::Holder],
        popular: false,
    },
    LicenseEntry {
        spdx: "GPL-2.0-only",
        name: "GNU General Public License v2.0 only",
        body: include_str!("../../data/licenses/GPL-2.0-only.txt"),
        placeholders: &[],
        popular: false,
    },
    LicenseEntry {
        spdx: "GPL-2.0-or-later",
        name: "GNU General Public License v2.0 or later",
        body: include_str!("../../data/licenses/GPL-2.0-or-later.txt"),
        placeholders: &[],
        popular: false,
    },
    LicenseEntry {
        spdx: "LGPL-2.1-only",
        name: "GNU Lesser General Public License v2.1 only",
        body: include_str!("../../data/licenses/LGPL-2.1-only.txt"),
        placeholders: &[],
        popular: false,
    },
    LicenseEntry {
        spdx: "LGPL-2.1-or-later",
        name: "GNU Lesser General Public License v2.1 or later",
        body: include_str!("../../data/licenses/LGPL-2.1-or-later.txt"),
        placeholders: &[],
        popular: false,
    },
    LicenseEntry {
        spdx: "LGPL-3.0-only",
        name: "GNU Lesser General Public License v3.0 only",
        body: include_str!("../../data/licenses/LGPL-3.0-only.txt"),
        placeholders: &[],
        popular: false,
    },
    LicenseEntry {
        spdx: "LGPL-3.0-or-later",
        name: "GNU Lesser General Public License v3.0 or later",
        body: include_str!("../../data/licenses/LGPL-3.0-or-later.txt"),
        placeholders: &[],
        popular: false,
    },
];

/// Look up a license entry by exact, case-sensitive SPDX identifier.
///
/// SPDX identifiers are case-and-punctuation-sensitive (`GPL-3.0-or-later`,
/// not `gpl-3.0` or `GPL-3.0`). A linear scan is plenty for a ~30-50 entry
/// registry called once per run.
pub fn by_spdx(spdx: &str) -> Result<&'static LicenseEntry> {
    LICENSES
        .iter()
        .find(|e| e.spdx == spdx)
        .ok_or_else(|| LinimError::LicenseNotFound {
            spdx: spdx.to_string(),
        })
}

/// Yield the popular entries, in array order, for the picker's top section.
pub fn popular() -> impl Iterator<Item = &'static LicenseEntry> {
    LICENSES.iter().filter(|e| e.popular)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::LinimError;

    #[test]
    fn by_spdx_mit() {
        let entry = by_spdx("MIT").expect("MIT present");
        assert_eq!(entry.spdx, "MIT");
        assert_eq!(entry.name, "MIT License");
        assert!(entry.popular);
        assert_eq!(entry.placeholders, &[Placeholder::Year, Placeholder::Holder]);
    }

    #[test]
    fn by_spdx_gpl_or_later() {
        let entry = by_spdx("GPL-3.0-or-later").expect("GPL-3.0-or-later present");
        assert_eq!(entry.spdx, "GPL-3.0-or-later");
        assert!(!entry.body.is_empty());
    }

    #[test]
    fn by_spdx_case_sensitive() {
        assert!(matches!(
            by_spdx("gpl-3.0-or-later"),
            Err(LinimError::LicenseNotFound { .. })
        ));
    }

    #[test]
    fn by_spdx_missing() {
        assert!(matches!(
            by_spdx("NOPE"),
            Err(LinimError::LicenseNotFound { .. })
        ));
    }

    #[test]
    fn every_body_non_empty() {
        for e in LICENSES {
            assert!(!e.body.is_empty(), "empty body for {}", e.spdx);
        }
    }

    #[test]
    fn every_spdx_unique() {
        let mut seen = std::collections::HashSet::new();
        for e in LICENSES {
            assert!(seen.insert(e.spdx), "duplicate spdx: {}", e.spdx);
        }
    }

    #[test]
    fn every_spdx_starts_upper_or_digit() {
        for e in LICENSES {
            let first = e.spdx.chars().next().expect("non-empty spdx");
            assert!(
                first.is_ascii_uppercase() || first.is_ascii_digit(),
                "spdx {:#?} starts with {:#?}",
                e.spdx,
                first
            );
        }
    }

    #[test]
    fn popular_includes_mit_and_apache() {
        let names: Vec<&str> = popular().map(|e| e.spdx).collect();
        assert!(names.contains(&"MIT"), "popular = {names:?}");
        assert!(names.contains(&"Apache-2.0"), "popular = {names:?}");
    }

    #[test]
    fn popular_comes_first_in_array() {
        let first_popular = LICENSES.iter().position(|e| e.popular);
        let last_popular = LICENSES.iter().rposition(|e| e.popular);
        // All popular entries form a leading contiguous block.
        if let (Some(first), Some(last)) = (first_popular, last_popular) {
            for (i, entry) in LICENSES.iter().enumerate() {
                let in_block = i >= first && i <= last;
                assert_eq!(entry.popular, in_block, "popular split at index {i}");
            }
        }
    }
}
