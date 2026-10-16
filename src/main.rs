// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

use linim::cli;
use linim::error::LinimError;

fn main() {
    let args = cli::args::parse();
    match linim::app::run(args) {
        Ok(true) => std::process::exit(1), // some files had errors
        Ok(false) => {}
        Err(LinimError::Cancelled) => {} // user backed out; silent exit 0
        Err(e) => {
            eprintln!("linim: {e}");
            std::process::exit(1);
        }
    }
}
