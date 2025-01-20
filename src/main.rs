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

use std::process::ExitCode;

use anyhow::{bail, Result};
use args::*;
use clap::Parser;
use log::error;

use self::logger::Logger;

fn main() -> ExitCode {
    let CmdArgs {
        verbosity,
        action,
        config,
    } = CmdArgs::parse();

    // Set up the logger
    Logger::new(verbosity).init().unwrap();

    match inner_main(action, config) {
        Ok(()) => ExitCode::SUCCESS,

        Err(err) => {
            error!("ERROR: {err:?}");
            ExitCode::FAILURE
        }
    }
}

fn inner_main(action: Action, config: Config) -> Result<()> {
    match action {
        Action::List(args) => actions::list(args, &config)?,
        Action::Remove(args) => actions::remove(args, &config)?,
        Action::Drop(args) => actions::drop(args, &config)?,
        Action::PathOf(args) => actions::path_of(args, &config)?,
        Action::Restore(args) => actions::restore(args, &config)?,
        Action::Empty => actions::empty(&config)?,
        Action::TrashPath => actions::trash_path(&config)?,
    }

    Ok(())
}
