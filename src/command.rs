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
    #[clap(long)]
    pub name: Option<String>,

    #[clap(short, long)]
    pub details: bool
}

#[derive(Clap)]
pub struct MoveToTrash {
    #[clap()]
    pub path: String,

    #[clap(short, long)]
    pub permanently: bool,

    #[clap(short, long, conflicts_with="permanently")]
    pub allow_invalid_utf8_item_names: bool
}

#[derive(Clap)]
pub struct RestoreItem {
    #[clap()]
    pub filename: String,

    #[clap(long, parse(from_os_str))]
    pub to: Option<PathBuf>,

    #[clap(long)]
    pub id: Option<String>,

    #[clap(short, long)]
    pub force: bool
}

#[derive(Clap)]
pub struct DropItem {
    #[clap()]
    pub filename: String,

    #[clap(long)]
    pub id: Option<String>
}

#[derive(Clap)]
pub struct EmptyTrash { }