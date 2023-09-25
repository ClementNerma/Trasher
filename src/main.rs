#![forbid(unsafe_code)]
#![forbid(unused_must_use)]
#![warn(unused_crate_dependencies)]

mod actions;
mod args;
mod fsutils;
mod items;

use anyhow::{bail, Context, Result};
use args::*;
use clap::Parser;
use fsutils::cleanup_transfer_dir;
use std::{
    env, fs,
    path::PathBuf,
    process::ExitCode,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::fsutils::TRASH_TRANSFER_DIRNAME;

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
    let Opts {
        trash_dir,
        dont_create_trash_dir,
        no_cleanup,
        verbose,
        action,
    } = Opts::parse();

    if verbose {
        PRINT_DEBUG_MESSAGES.store(true, Ordering::SeqCst);
    }

    let trash_dir = match trash_dir {
        Some(trash_dir) => trash_dir,
        None => {
            let path = env::var_os("TRASH_DIR").context(
                "None of --trash-dir option and TRASH_DIR environment variable were provided",
            )?;

            PathBuf::from(path)
        }
    };

    let partial_trash_dir = trash_dir.join(TRASH_TRANSFER_DIRNAME);

    if !partial_trash_dir.exists() {
        if dont_create_trash_dir {
            bail!("Trash directory does not exist. Specify '--create-trash-dir' to create it automatically.");
        }

        fs::create_dir_all(&partial_trash_dir).unwrap();

        debug!("Created trash directory.");
    }

    match action {
        Action::List(args) => actions::list(args, &trash_dir)?,
        Action::Remove(args) => actions::remove(args, &trash_dir)?,
        Action::Drop(args) => actions::drop(args, &trash_dir)?,
        Action::PathOf(args) => actions::path_of(args, &trash_dir)?,
        Action::Restore(args) => actions::restore(args, &trash_dir)?,
        Action::Clear(args) => actions::clear(args, &trash_dir)?,
    }

    if !no_cleanup {
        cleanup_transfer_dir(&partial_trash_dir)
            .context("Failed to clean the transfer directory")?;
    }

    Ok(())
}

pub static PRINT_DEBUG_MESSAGES: AtomicBool = AtomicBool::new(false);

#[macro_export]
macro_rules! debug {
    ($message: expr$(,$params: expr)*) => { if $crate::PRINT_DEBUG_MESSAGES.load(::std::sync::atomic::Ordering::SeqCst) { println!(concat!("[DEBUG] ", $message), $($params,)*); } }
}
