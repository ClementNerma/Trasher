use clap::Clap;
use std::path::PathBuf;

lazy_static! {
    pub static ref OPTS: Opts = Opts::parse();
}

#[derive(Clap)]
#[clap(version = "1.1.0", author = "Clément Nerma <clement.nerma@gmail.com>")]
pub struct Opts {
    #[clap(short, long, parse(from_os_str))]
    pub trash_dir: PathBuf,

    #[clap(short, long)]
    pub create_trash_dir: bool,

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

    #[clap(name = "unrm", alias = "restore", about = "Restore an item from the trash")]
    Restore(RestoreItem),

    #[clap(name = "drop", about = "Permanently delete an item from the trash")]
    Drop(DropItem),

    #[clap(name = "clear", about = "Permanently delete all items in the trash")]
    Clear(EmptyTrash)
}

#[derive(Clap)]
pub struct ListTrashItems {
    #[clap(long, about = "Only list occurrences of items with a specific name")]
    pub name: Option<String>,

    #[clap(short, long, about = "Show details (size, number of files and directories)")]
    pub details: bool
}

#[derive(Clap)]
pub struct MoveToTrash {
    #[clap(about = "Item path to move to the trash")]
    pub path: String,

    #[clap(short, long, about = "Delete the item permanently")]
    pub permanently: bool,

    #[clap(short, long, conflicts_with="permanently", about = "For external filesystems, move the item to the main filesystem's trash directory")]
    pub move_ext_filesystems: bool,

    #[clap(short, long, conflicts_with="permanently", about = "Do not fail when encoutering invalid UTF-8 file names")]
    pub allow_invalid_utf8_item_names: bool,
}

#[derive(Clap)]
pub struct RestoreItem {
    #[clap(about = "Name of the item to restore")]
    pub filename: String,

    #[clap(long, parse(from_os_str), about = "Destination path (defaults to the current directory)")]
    pub to: Option<PathBuf>,

    #[clap(long, about = "ID of the item to restore in case multiple exist with the same name")]
    pub id: Option<String>,

    #[clap(short, long, conflicts_with="permanently", about = "For external filesystems, move the item from the main filesystem's trash directory")]
    pub move_ext_filesystems: bool,

    #[clap(short, long, about = "Overwrite target path if it already exists")]
    pub force: bool
}

#[derive(Clap)]
pub struct DropItem {
    #[clap(about = "Name of the item to permanently delete from the trash")]
    pub filename: String,

    #[clap(long, about = "ID of the item to drop in case multiple exist with the same name")]
    pub id: Option<String>
}

#[derive(Clap)]
pub struct EmptyTrash { }