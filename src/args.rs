use std::path::PathBuf;

use clap::{Parser, Subcommand};
use log::LevelFilter;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
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
    List(ListTrashItems),

    #[clap(name = "rm", about = "Move an item to the trash")]
    Remove(MoveToTrash),

    #[clap(
        name = "unrm",
        alias = "restore",
        about = "Restore an item from the trash"
    )]
    Restore(RestoreItem),

    #[clap(name = "drop", about = "Permanently delete an item from the trash")]
    Drop(DropItem),

    #[clap(
        name = "path-of",
        about = "Get the path of an item inside the trash directory"
    )]
    PathOf(GetItemPath),

    #[clap(
        name = "trash-path",
        about = "Get the path of the trash directory for the current folder"
    )]
    TrashPath,

    #[clap(name = "empty", about = "Permanently delete all items in the trash")]
    Empty,
}

#[derive(Parser)]
pub struct ListTrashItems {
    #[clap(long, help = "Only list occurrences of items with a specific name")]
    pub name: Option<String>,
}

#[derive(Parser)]
pub struct MoveToTrash {
    #[clap(num_args = 1.., help = "Path of the items to move to the trash")]
    pub paths: Vec<String>,

    #[clap(short, long, help = "Delete the items permanently")]
    pub permanently: bool,

    #[clap(
        short,
        long,
        help = "Do nothing if the item doesn't exist instead of failing"
    )]
    pub ignore: bool,

    #[clap(
        short,
        long,
        help = "Do not fail when encoutering invalid UTF-8 file names"
    )]
    pub allow_invalid_utf8_item_names: bool,
}

#[derive(Parser)]
pub struct RestoreItem {
    #[clap(help = "Name of the item to restore")]
    pub filename: Option<String>,

    #[clap(
        long,
        help = "Destination path (defaults to the current directory)",
        requires = "filename"
    )]
    pub to: Option<PathBuf>,

    #[clap(
        long,
        help = "ID of the item to restore in case multiple exist with the same name",
        requires = "filename"
    )]
    pub id: Option<String>,
}

#[derive(Parser)]
pub struct DropItem {
    #[clap(help = "Name of the item to permanently delete from the trash")]
    pub filename: String,

    #[clap(
        long,
        help = "ID of the item to drop in case multiple exist with the same name"
    )]
    pub id: Option<String>,
}

#[derive(Parser)]
pub struct GetItemPath {
    #[clap(help = "Name of the item to get the path of in the trash")]
    pub filename: String,

    #[clap(
        long,
        help = "ID of the item to get in case multiple exist with the same name"
    )]
    pub id: Option<String>,

    #[clap(
        short,
        long,
        help = "Do not fail if the path contains invalid UTF-8 characters"
    )]
    pub allow_invalid_utf8_path: bool,
}
