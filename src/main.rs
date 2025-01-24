#![forbid(unsafe_code)]
#![forbid(unused_must_use)]
#![warn(unused_crate_dependencies)]
#![allow(clippy::format_collect)]
// NOTE: NIGHTLY
#![cfg_attr(target_family = "windows", feature(windows_by_handle))]

mod actions;
mod args;
mod fsutils;
mod fuzzy;
mod items;
mod logger;

use std::{path::PathBuf, process::ExitCode};

use anyhow::{bail, Result};
use args::*;
use clap::Parser;
use log::error;

use self::{fsutils::compute_exclusions, logger::Logger};

fn main() -> ExitCode {
    let Args {
        verbosity,
        exclude,
        action,
    } = Args::parse();

    // Set up the logger
    Logger::new(verbosity).init().unwrap();

    match inner_main(action, &exclude) {
        Ok(()) => ExitCode::SUCCESS,

        Err(err) => {
            error!("ERROR: {err:?}");
            ExitCode::FAILURE
        }
    }
}

fn inner_main(action: Action, exclude: &[PathBuf]) -> Result<()> {
    // Compute the list diectories to exclude
    let exclude_dirs = compute_exclusions(exclude)?;

    match action {
        Action::List(args) => actions::list(args, &exclude_dirs)?,
        Action::Remove(args) => actions::remove(args, &exclude_dirs)?,
        Action::Drop(args) => actions::drop(args, &exclude_dirs)?,
        Action::PathOf(args) => actions::path_of(args, &exclude_dirs)?,
        Action::Restore(args) => actions::restore(args, &exclude_dirs)?,
        Action::Empty => actions::empty(&exclude_dirs)?,
        Action::TrashPath => actions::trash_path(&exclude_dirs)?,
    }

    Ok(())
}
