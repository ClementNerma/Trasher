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
            eprintln!("\x1B[91mERROR: {err:?}\x1B[0m");
            ExitCode::FAILURE
        }
    }
}

fn inner_main() -> Result<()> {
    let Opts { verbose, action } = Opts::parse();

    if verbose {
        PRINT_DEBUG_MESSAGES.store(true, Ordering::SeqCst);
    }

    match action {
        Action::List(args) => actions::list(args)?,
        Action::Remove(args) => actions::remove(args)?,
        Action::Drop(args) => actions::drop(args)?,
        Action::PathOf(args) => actions::path_of(args)?,
        Action::Restore(args) => actions::restore(args)?,
        Action::Empty => actions::empty()?,
        Action::TrashPath => actions::trash_path()?,
    }

    Ok(())
}

pub static PRINT_DEBUG_MESSAGES: AtomicBool = AtomicBool::new(false);

#[macro_export]
macro_rules! debug {
    ($message: expr$(,$params: expr)*) => { if $crate::PRINT_DEBUG_MESSAGES.load(::std::sync::atomic::Ordering::SeqCst) { println!(concat!("[DEBUG] ", $message), $($params,)*); } }
}
