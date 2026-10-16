// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

//! The interactive, searchable license selector.
//!
//! Search-as-you-type is provided by `dialoguer::FuzzySelect`: no custom
//! filtering. The picker reads `registry::LICENSES`, never writes, and returns
//! a `&'static LicenseEntry`. If the prompt fails or is cancelled, it aborts;
//! there is no silent fallback license.

use crate::error::{LinimError, Result};
use crate::license::registry::{self, LicenseEntry};

/// The fixed width of the SPDX column, long enough for the longest identifier
/// we ship.
const SPDX_WIDTH: usize = 16;

/// Build the display strings for the picker, in `registry::LICENSES` order
/// (popular first).
pub fn build_items() -> Vec<String> {
    registry::LICENSES
        .iter()
        .map(|e| format!("{:<SPDX_WIDTH$} {}", e.spdx, e.name))
        .collect()
}

/// Run the interactive, searchable prompt and return the chosen entry.
///
/// Fuzzy matching runs against the full `"<spdx>   <name>"` display string, so
/// both the SPDX identifier and the human name are searchable. Returns
/// `Err(PromptFailed)` on a genuine prompt failure and `Err(Cancelled)` on
/// Ctrl-C.
pub fn pick() -> Result<&'static LicenseEntry> {
    LinimError::ensure_tty()?;
    let items = build_items();
    let selection = dialoguer::FuzzySelect::new()
        .with_prompt("Choose a license")
        .items(&items)
        .interact()
        .map_err(LinimError::dialoguer)?;
    Ok(&registry::LICENSES[selection])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_items_length_matches_registry() {
        assert_eq!(build_items().len(), registry::LICENSES.len());
    }

    #[test]
    fn build_items_format_matches_expected() {
        let items = build_items();
        let idx = registry::LICENSES
            .iter()
            .position(|e| e.spdx == "MIT")
            .unwrap();
        assert_eq!(items[idx], "MIT              MIT License");
    }

    #[test]
    fn popular_entries_come_first() {
        let items = build_items();
        let first_popular = registry::LICENSES.iter().position(|e| e.popular).unwrap();
        let last_popular = registry::LICENSES.iter().rposition(|e| e.popular).unwrap();
        // Every item in the leading popular block matches a popular entry, and
        // every popular entry appears in the leading block of the item list.
        for (i, item) in items.iter().enumerate() {
            let entry = &registry::LICENSES[i];
            let in_block = i >= first_popular && i <= last_popular;
            assert_eq!(entry.popular, in_block, "mismatch at index {i}: {item}");
        }
    }

    #[test]
    fn every_spdx_appears_in_items() {
        let items = build_items();
        for entry in registry::LICENSES {
            assert!(
                items.iter().any(|it| it.starts_with(entry.spdx)),
                "missing {} in items",
                entry.spdx
            );
        }
    }

    // Requires an interactive TTY. Run manually: cargo test -- --ignored
    #[test]
    #[ignore]
    fn pick_returns_a_license() {
        let entry = pick().unwrap();
        assert!(!entry.spdx.is_empty());
    }
}
