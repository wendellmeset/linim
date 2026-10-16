// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

//! Resolve the copyright holder and year.
//!
//! The entire copyright layer: either read from stored state or prompt the
//! user interactively. No git, no environment fallback, no `LICENSE` scanning.
//! Produces the [`CopyrightInfo`] that `header::template::build` needs.

use crate::error::{LinimError, Result};
use crate::helpers;
use crate::state::State;

/// The resolved copyright holder and year.
pub struct CopyrightInfo {
    /// The copyright holder name.
    pub holder: String,
    /// The copyright year.
    pub year: u16,
}

/// Resolve the copyright info.
///
/// - `Some(state)`: reuse `state.holder` and `state.year`; no prompt.
/// - `None`: prompt the user for both fields (first-run / `--new` case).
pub fn resolve(state: Option<&State>) -> Result<CopyrightInfo> {
    if let Some(s) = state {
        return Ok(CopyrightInfo {
            holder: s.holder.clone(),
            year: s.year,
        });
    }
    let holder = prompt_holder()?;
    let year = prompt_year()?;
    Ok(CopyrightInfo { holder, year })
}

/// Prompt for a non-empty copyright holder, looping until one is given.
fn prompt_holder() -> Result<String> {
    LinimError::ensure_tty()?;
    loop {
        let input: String = dialoguer::Input::new()
            .with_prompt("Copyright holder")
            .interact_text()
            .map_err(LinimError::dialoguer)?;
        let trimmed = input.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
        eprintln!("Copyright holder cannot be empty.");
    }
}

/// Prompt for a year, defaulting to the current year, looping on invalid input.
fn prompt_year() -> Result<u16> {
    let default = helpers::current_year();
    loop {
        let input: String = dialoguer::Input::new()
            .with_prompt("Year")
            .default(helpers::format_year(default))
            .interact_text()
            .map_err(LinimError::dialoguer)?;
        match helpers::parse_year(input.trim()) {
            Some(year) => return Ok(year),
            None => eprintln!("Please enter a valid year."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::State;

    #[test]
    fn resolve_uses_state_when_present() {
        let state = State::new("MIT".into(), "wendellmeset".into(), 2026);
        let info = resolve(Some(&state)).unwrap();
        assert_eq!(info.holder, "wendellmeset");
        assert_eq!(info.year, 2026);
    }

    #[test]
    fn resolve_accepts_empty_holder_from_state() {
        // The prompt is the only place non-empty is enforced; the state path
        // trusts whatever is stored.
        let state = State::new("MIT".into(), String::new(), 2021);
        let info = resolve(Some(&state)).unwrap();
        assert_eq!(info.holder, "");
        assert_eq!(info.year, 2021);
    }

    // The `None` path requires an interactive TTY, so it cannot run in a test
    // harness. Run manually with: cargo test -- --ignored
    #[test]
    #[ignore]
    fn resolve_prompts_when_no_state() {
        let info = resolve(None).unwrap();
        assert!(!info.holder.is_empty());
        assert!((1000..=9999).contains(&info.year));
    }
}
