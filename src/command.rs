use clap::{crate_version, Clap};
use std::path::PathBuf;

lazy_static! {
    pub static ref OPTS: Opts = Opts::parse();
}

#[derive(Clap)]
#[clap(version = crate_version!(), author = "Clément Nerma <clement.nerma@gmail.com>")]
pub struct Opts {
    #[clap(short, long, parse(from_os_str))]
    pub trash_dir: PathBuf,

    #[clap(short, long)]
    pub create_trash_dir: bool,

    #[clap(long, about = "Don't clean up the transfer directory automatically")]
    pub no_cleanup: bool,

    #[clap(short, long)]
    pub verbose: bool,

    #[clap(subcommand)]
    pub action: Action,
}

#[derive(Clap)]
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

#[derive(Clap)]
pub struct ListTrashItems {
    #[clap(long, about = "Only list occurrences of items with a specific name")]
    pub name: Option<String>,

    #[clap(
        short,
        long,
        about = "Show details (size, number of files and directories)"
    )]
    pub details: bool,
}

#[derive(Clap)]
pub struct MoveToTrash {
    #[clap(about = "Path of the items to move to the trash")]
    pub paths: Vec<String>,

    #[clap(short, long, about = "Delete the items permanently")]
    pub permanently: bool,

    #[clap(
        short,
        long,
        about = "Do nothing if the item doesn't exist instead of failing"
    )]
    pub ignore: bool,

    #[clap(
        short,
        long,
        about = "For external filesystems, move the items to the main filesystem's trash directory"
    )]
    pub move_ext_filesystems: bool,

    #[clap(
        short,
        long,
        about = "Only apply '--move-ext-filesystems' if the items' size is lower or equal to the provided one"
    )]
    pub size_limit_move_ext_filesystems: Option<String>,

    #[clap(
        short,
        long,
        about = "Do not fail when encoutering invalid UTF-8 file names"
    )]
    pub allow_invalid_utf8_item_names: bool,
}

#[derive(Clap)]
pub struct RestoreItem {
    #[clap(about = "Name of the item to restore")]
    pub filename: String,

    #[clap(
        long,
        parse(from_os_str),
        about = "Destination path (defaults to the current directory)"
    )]
    pub to: Option<PathBuf>,

    #[clap(
        long,
        about = "ID of the item to restore in case multiple exist with the same name"
    )]
    pub id: Option<String>,

    #[clap(
        short,
        long,
        about = "For external filesystems, move the item from the main filesystem's trash directory"
    )]
    pub move_ext_filesystems: bool,

    #[clap(short, long, about = "Overwrite target path if it already exists")]
    pub force: bool,
}

#[derive(Clap)]
pub struct DropItem {
    #[clap(about = "Name of the item to permanently delete from the trash")]
    pub filename: String,

    #[clap(
        long,
        about = "ID of the item to drop in case multiple exist with the same name"
    )]
    pub id: Option<String>,
}

#[derive(Clap)]
pub struct GetItemPath {
    #[clap(about = "Name of the item to get the path of in the trash")]
    pub filename: String,

    #[clap(
        long,
        about = "ID of the item to get in case multiple exist with the same name"
    )]
    pub id: Option<String>,

    #[clap(
        short,
        long,
        about = "Do not fail if the path contains invalid UTF-8 characters"
    )]
    pub allow_invalid_utf8_path: bool,
}

#[derive(Clap)]
pub struct EmptyTrash {}
