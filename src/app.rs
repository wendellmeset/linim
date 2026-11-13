// SPDX-FileCopyrightText: 2026 wendellmeset
// SPDX-License-Identifier: GPL-3.0-or-later

//! The top-level pipeline: turn parsed arguments into a finished run.
//!
//! Returns `true` if any file hit an error during processing (so `main.rs` can
//! set the exit code); `false` otherwise. No argument parsing, no error
//! printing, no `process::exit`; those belong to `main.rs`.

use std::fs;
use std::io;

use crate::cli::args::Args;
use crate::copyright;
use crate::error::{LinimError, Result};
use crate::header::template;
use crate::helpers;
use crate::license::{picker, registry};
use crate::process::parallel;
use crate::process::single::Context;
use crate::process::summary;
use crate::state::{self, State};
use crate::walker;

/// Resolve the license and copyright for this run.
///
/// Precedence, from strongest to weakest:
///
/// - `--new`: ignore `.linim` and any `LICENSE`; use `--license` or the
///   picker, then always prompt for copyright.
/// - `--license`: honor the explicit SPDX identifier. Reuse the stored
///   copyright when `.linim` exists, otherwise prompt.
/// - existing `.linim`: reuse both the stored license and copyright.
/// - detected `LICENSE`: use its SPDX identifier, then prompt for copyright.
/// - otherwise: the interactive picker, then prompt for copyright.
fn resolve(
    args: &Args,
    existing: Option<&State>,
) -> Result<(&'static registry::LicenseEntry, copyright::CopyrightInfo)> {
    if args.new {
        let entry = match args.license.as_deref() {
            Some(spdx) => registry::by_spdx(spdx)?,
            None => picker::pick()?,
        };
        let info = copyright::resolve(None)?;
        return Ok((entry, info));
    }

    if let Some(spdx) = args.license.as_deref() {
        let entry = registry::by_spdx(spdx)?;
        let info = copyright::resolve(existing)?;
        return Ok((entry, info));
    }

    if let Some(state) = existing {
        let entry = registry::by_spdx(&state.spdx)?;
        let info = copyright::resolve(Some(state))?;
        return Ok((entry, info));
    }

    let entry = match crate::license::detect::detect(&args.path)? {
        Some(spdx) => registry::by_spdx(&spdx)?,
        None => picker::pick()?,
    };
    let info = copyright::resolve(None)?;
    Ok((entry, info))
}

/// Run the full pipeline. `Ok(true)` means files failed; `Ok(false)` means a
/// clean run (with or without changes).
pub fn run(args: Args) -> Result<bool> {
    // Phase 1: fail fast if the path is unusable.
    if !args.path.is_dir() {
        return Err(LinimError::io(
            &args.path,
            io::Error::new(io::ErrorKind::NotFound, "path is not a directory"),
        ));
    }

    // Phase 2: read the existing `.linim`, if any.
    let existing_state = state::read(&args.path)?;

    // Phase 3: resolve license and copyright (reuse or reconfigure).
    let (entry, info) = resolve(&args, existing_state.as_ref())?;

    // Phase 4: build the new state.
    let mut new_state = State::new(entry.spdx.to_string(), info.holder.clone(), info.year);
    new_state.headers_enabled = !args.no_headers;

    // Phase 5: write `LICENSE` if it differs (byte-exact after trailing
    // newlines; legal text is not whitespace-normalized away).
    let license_text = crate::license::template::build(entry, &info.holder, info.year);
    let license_path = args.path.join("LICENSE");
    let license_needs_write = match fs::read_to_string(&license_path) {
        Ok(existing) => {
            helpers::trim_trailing_newlines(&existing)
                != helpers::trim_trailing_newlines(&license_text)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => true,
        Err(e) => return Err(LinimError::io(&license_path, e)),
    };
    if license_needs_write {
        fs::write(&license_path, &license_text).map_err(|e| LinimError::io(&license_path, e))?;
    }

    // Phase 6: write `.linim` only if the state changed.
    if existing_state.as_ref() != Some(&new_state) {
        state::write(&args.path, &new_state)?;
    }

    // Phase 7: `--no-headers` stops here, after the LICENSE/.linim writes.
    if args.no_headers {
        return Ok(false);
    }

    // Phase 8: build the per-run context, deriving expected values from the
    // template so nothing can drift from the rendered header.
    let template = template::build(&new_state.spdx, &info.holder, info.year);
    let ctx = Context {
        expected_spdx: template.spdx_value.clone(),
        expected_copyright: template.copyright_line.clone(),
        template,
    };

    // Phase 9-10: walk and process.
    let paths = walker::collect_files(&args.path)?;
    let results = parallel::process_all(&paths, &ctx);
    let summary = summary::summarize(&results, &paths, args.verbose);

    // Phase 11: report and report failure.
    if summary.verbose {
        eprint!("{}", summary.render_verbose(&args.path));
    }
    print!("{}", summary.render(&args.path));
    Ok(!summary.errors.is_empty())
}
