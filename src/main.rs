#![forbid(unsafe_code)]
#![forbid(unused_must_use)]
#![warn(unused_crate_dependencies)]
#![allow(clippy::format_collect)]
// NOTE: NIGHTLY
#![cfg_attr(target_family = "windows", feature(windows_by_handle))]

mod actions;
mod args;
mod display;
mod fsutils;
mod fuzzy;
mod items;

use std::{
    process::ExitCode,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::{bail, Result};
use args::*;
use clap::Parser;

fn main() -> ExitCode {
    match inner_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!("ERROR: {err:?}");
            ExitCode::FAILURE
        }
    }
}

fn inner_main() -> Result<()> {
    let Opts {
        verbose,
        action,
        config,
    } = Opts::parse();

    if verbose {
        PRINT_DEBUG_MESSAGES.store(true, Ordering::SeqCst);
    }

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

pub static PRINT_DEBUG_MESSAGES: AtomicBool = AtomicBool::new(false);

#[macro_export]
macro_rules! debug {
    ($message: expr$(,$params: expr)*) => {{
        use ::std::sync::atomic::Ordering;

        if $crate::PRINT_DEBUG_MESSAGES.load(Ordering::SeqCst) {
            println!(concat!("[DEBUG] ", $message), $($params,)*);
        }
    }}
}
