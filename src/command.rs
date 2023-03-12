use clap::{Parser, Subcommand};
use std::path::PathBuf;

lazy_static! {
    pub static ref OPTS: Opts = Opts::parse();
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Opts {
    #[clap(short, long)]
    pub trash_dir: PathBuf,

    #[clap(short, long)]
    pub create_trash_dir: bool,

    #[clap(long, help = "Don't clean up the transfer directory automatically")]
    pub no_cleanup: bool,

    #[clap(short, long)]
    pub verbose: bool,

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

    #[clap(name = "clear", about = "Permanently delete all items in the trash")]
    Clear(EmptyTrash),
}

#[derive(Parser)]
pub struct ListTrashItems {
    #[clap(long, help = "Only list occurrences of items with a specific name")]
    pub name: Option<String>,

    #[clap(
        short,
        long,
        help = "Show details (size, number of files and directories)"
    )]
    pub details: bool,
}

#[derive(Parser)]
pub struct MoveToTrash {
    #[clap(help = "Path of the items to move to the trash")]
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
        help = "For external filesystems, move the items to the main filesystem's trash directory"
    )]
    pub move_ext_filesystems: bool,

    #[clap(
        short,
        long,
        help = "Only apply '--move-ext-filesystems' if the items' size is lower or equal to the provided one"
    )]
    pub size_limit_move_ext_filesystems: Option<String>,

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
    pub filename: String,

    #[clap(long, help = "Destination path (defaults to the current directory)")]
    pub to: Option<PathBuf>,

    #[clap(
        long,
        help = "ID of the item to restore in case multiple exist with the same name"
    )]
    pub id: Option<String>,

    #[clap(
        short,
        long,
        help = "For external filesystems, move the item from the main filesystem's trash directory"
    )]
    pub move_ext_filesystems: bool,

    #[clap(short, long, help = "Overwrite target path if it already exists")]
    pub force: bool,
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

#[derive(Parser)]
pub struct EmptyTrash {}
