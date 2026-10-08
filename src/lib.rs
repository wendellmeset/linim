//! linim: manage a project's open-source licensing in one command.
//!
//! The library crate holds the domain modules; the binary is a thin CLI on top.

pub mod cli;
pub mod copyright;
pub mod error;
pub mod header;
pub mod helpers;
pub mod license;
pub mod process;
pub mod state;
pub mod walker;
