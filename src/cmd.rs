use std::path::PathBuf;

use clap::{Parser, Subcommand};
use log::LevelFilter;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Cmd {
    #[clap(
        short,
        long,
        global = true,
        help = "Level of verbosity",
        default_value = "info"
    )]
    pub verbosity: LevelFilter,

    #[clap(
        global = true,
        short,
        long,
        help = "Disallow making a filesystem-local trash directory in some paths"
    )]
    pub exclude: Vec<PathBuf>,

    #[clap(subcommand)]
    pub action: Action,
}

#[derive(Subcommand)]
pub enum Action {
    #[clap(name = "ls", about = "List all items in the trash")]
    List {
        #[clap(long, help = "Only list occurrences of items with a specific name")]
        name: Option<String>,
    },

    #[clap(name = "rm", about = "Move an item to the trash")]
    Remove {
        #[clap(num_args = 1.., help = "Path of the items to move to the trash")]
        paths: Vec<String>,

        #[clap(short, long, help = "Delete the items permanently")]
        permanently: bool,

        #[clap(
            short,
            long,
            help = "Do nothing if the item doesn't exist instead of failing"
        )]
        ignore: bool,

        #[clap(
            short,
            long,
            help = "Do not fail when encoutering invalid UTF-8 file names"
        )]
        allow_invalid_utf8_item_names: bool,
    },

    #[clap(name = "unrm", about = "Restore an item from the trash")]
    Restore {
        #[clap(help = "Name of the item to restore")]
        filename: Option<String>,

        #[clap(
            long,
            help = "Destination path (defaults to the current directory)",
            requires = "filename"
        )]
        to: Option<PathBuf>,

        #[clap(
            long,
            help = "ID of the item to restore in case multiple exist with the same name",
            requires = "filename"
        )]
        id: Option<String>,
    },

    #[clap(about = "Permanently delete an item from the trash")]
    Drop {
        #[clap(help = "Name of the item to permanently delete from the trash")]
        filename: String,

        #[clap(
            long,
            help = "ID of the item to drop in case multiple exist with the same name"
        )]
        id: Option<String>,
    },

    #[clap(about = "Get the path of an item inside the trash directory")]
    PathOf {
        #[clap(help = "Name of the item to get the path of in the trash")]
        filename: String,

        #[clap(
            long,
            help = "ID of the item to get in case multiple exist with the same name"
        )]
        id: Option<String>,

        #[clap(
            short,
            long,
            help = "Do not fail if the path contains invalid UTF-8 characters"
        )]
        allow_invalid_utf8_path: bool,
    },

    #[clap(about = "Get the path of the trash directory for the current folder")]
    TrashPath,

    #[clap(about = "Permanently delete all items in the trash")]
    Empty,
}
