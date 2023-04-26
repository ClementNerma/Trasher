#![forbid(unsafe_code)]
#![forbid(unused_must_use)]
#![forbid(unused_crate_dependencies)]

mod actions;
mod args;
mod fsutils;
mod items;

use clap::Parser;
mod logging;
use args::*;
use fsutils::cleanup_transfer_dir;
use std::{fs, path::PathBuf};

use crate::fsutils::TRASH_TRANSFER_DIRNAME;

fn main() {
    let opts = Opts::parse();

    let trash_dir = opts.trash_dir.unwrap_or_else(|| {
        let path = std::env::var_os("TRASH_DIR").unwrap_or_else(|| {
            fail!("None of --trash-dir option and TRASH_DIR environment variable were provided")
        });

        PathBuf::from(path)
    });

    let partial_trash_dir = trash_dir.join(TRASH_TRANSFER_DIRNAME);

    if !partial_trash_dir.exists() {
        if opts.dont_create_trash_dir {
            fail!("Trash directory does not exist. Specify '--create-trash-dir' to create it automatically.");
        }

        fs::create_dir_all(&partial_trash_dir).unwrap();

        debug!("Created trash directory.");
    }

    match opts.action {
        Action::List(action) => actions::list(action, &trash_dir),
        Action::Remove(action) => actions::remove(action, &trash_dir),
        Action::Drop(action) => actions::drop(action, &trash_dir),
        Action::PathOf(action) => actions::path_of(action, &trash_dir),
        Action::Restore(action) => actions::restore(action, &trash_dir),
        Action::Clear(action) => actions::clear(action, &trash_dir),
    }

    if !opts.no_cleanup {
        if let Err(err) = cleanup_transfer_dir(&partial_trash_dir) {
            fail!("Failed to cleanup the transfer directory: {}", err)
        }
    }
}
