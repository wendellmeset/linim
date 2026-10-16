// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact strings shown in `--help`.
//!
//! Kept separate from `args.rs` so the help text is explicit and reviewable
//! rather than scattered as doc comments across struct fields.

/// The one-line description shown at the top of `--help`.
pub const LONG_ABOUT: &str =
    "linim: manage a project's license and per-file SPDX headers.";

/// Help for the positional path argument.
pub const PATH_HELP: &str = "Directory to operate on (default: .)";

/// Help for `--new`.
pub const NEW_HELP: &str = "Reconfigure from scratch. Prompts for license and \
    copyright, rewrites LICENSE and .linim, and updates every file's header.";

/// Help for `--no-headers`.
pub const NO_HEADERS_HELP: &str =
    "Write or update LICENSE only. Do not touch source file headers.";

/// Help for `--verbose`.
pub const VERBOSE_HELP: &str =
    "Emit one line per updated file to stderr.";
