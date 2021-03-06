#![forbid(unused_must_use)]
#![forbid(unsafe_code)]

mod actions;
mod command;
mod fsutils;
mod items;

#[macro_use]
extern crate lazy_static;

use command::*;
use fsutils::cleanup_transfer_dir;
use std::fs;

#[macro_export]
macro_rules! fail {
    ($message: expr$(,$params: expr)*) => {{
        eprintln!(concat!("\x1B[91m", "ERROR: ", $message, "\x1B[0m"), $($params,)*);
        std::process::exit(1);
    }}
}

#[macro_export]
macro_rules! debug {
    ($message: expr$(,$params: expr)*) => { if OPTS.verbose { println!(concat!("[DEBUG] ", $message), $($params,)*); } }
}

fn main() {
    if !OPTS.trash_dir.exists() {
        if !OPTS.create_trash_dir {
            fail!("Trash directory does not exist. Specify '--create-trash-dir' to create it automatically.");
        }

        fs::create_dir_all(&OPTS.trash_dir).unwrap();

        debug!("Created trash directory.");
    }

    match &OPTS.action {
        Action::List(action) => actions::list(action),
        Action::Remove(action) => actions::remove(action),
        Action::Drop(action) => actions::drop(action),
        Action::PathOf(action) => actions::path_of(action),
        Action::Restore(action) => actions::restore(action),
        Action::Clear(action) => actions::clear(action),
    }

    if !OPTS.no_cleanup {
        if let Err(err) = cleanup_transfer_dir() {
            fail!("Failed to cleanup the transfer directory: {}", err)
        }
    }
}
