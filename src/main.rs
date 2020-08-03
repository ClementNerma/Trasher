#![forbid(unused_must_use)]
#![forbid(unsafe_code)]

mod command;
mod items;
mod fsutils;

#[macro_use] extern crate lazy_static;

use std::fs;
use std::path::PathBuf;

use command::*;
use items::*;
use fsutils::*;

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
        Action::List(ListTrashItems { name, details }) => {
            debug!("Listing trash items...");
            let mut items = list_trash_items(&OPTS.trash_dir).unwrap();

            if let Some(name) = &name {
                debug!("Filtering {} items by name...", items.len());
                items = items.into_iter().filter(|item| item.filename().contains(name)).collect();
            }

            match items.len() {
                0 => println!("{}", if name.is_some() { "No item in trash match the provided name." } else { "Trash is empty." }),
                count => {
                    println!("Found {} item{} in trash:", count, if count >= 2 { "s" } else { "" });

                    let mut total_size = 0;
                    let mut total_dirs = 0;
                    let mut total_files = 0;

                    for item in items {
                        println!("* {}{}", item, if *details || OPTS.verbose {
                            let details = get_fs_details(OPTS.trash_dir.join(item.trash_filename())).unwrap();
                            let dir_one = if details.is_directory { 1 } else { 0 };

                            total_size += details.size;
                            total_dirs += details.sub_files + dir_one;
                            total_files += details.sub_files + (1 - dir_one);

                            format!("{}", details)
                        } else {
                            "".to_string()
                        });
                    }

                    if *details || OPTS.verbose {
                        println!(
                            "{} directories, {} files, total size is: {}",
                            total_dirs, total_files, human_readable_size(total_size)
                        );
                    }
                }
            }
        },

        Action::Remove(MoveToTrash { path, permanently, allow_invalid_utf8_item_names }) => {
            let path = PathBuf::from(path);

            debug!("Checking if item exists...");

            if !path.exists() {
                fail!("Item path does not exist.");
            }

            if *permanently {
                if let Err(err) = fs::remove_dir_all(&path) {
                    fail!("Failed to permanently remove item: {}", err);
                }
            }

            let file_name = path.file_name().unwrap_or_else(|| fail!("Specified item path has no file name"));
            let filename = match file_name.to_str() {
                Some(str) => str.to_string(),
                None => if *allow_invalid_utf8_item_names {
                    file_name.to_string_lossy().to_string()
                } else {
                    fail!("Specified item does not have a valid UTF-8 file name")
                }
            };

            let trash_item = TrashItem::new_now(filename.to_string());

            debug!("Moving item to trash under name '{}'...", trash_item.trash_filename());

            if let Err(err) = fs::rename(&path, &OPTS.trash_dir.join(trash_item.trash_filename())) {
                fail!("Failed to move item to trash: {}", err);
            }
        },

        Action::Drop(DropItem { filename, id }) => {
            debug!("Listing trash items...");

            match expect_trash_item(&OPTS.trash_dir, &filename, id.as_deref()).unwrap() {
                FoundTrashItems::Single(item) => {
                    let item_path = OPTS.trash_dir.join(item.trash_filename());

                    debug!("Permanently removing item from trash...");

                    if let Err(err) = fs::remove_dir_all(&item_path) {
                        fail!("Failed to remove item '{}' from trash: {}", item.filename(), err);
                    }
                },

                FoundTrashItems::Multi(candidates) => println!(
                    "Multiple items with this filename were found in the trash:{}",
                    candidates.iter().map(|c| format!("\n* {}", c)).collect::<String>()
                )
            }
        },
        
        Action::Restore(RestoreItem { filename, to, id, force }) => {
            debug!("Listing trash items...");

            match expect_trash_item(&OPTS.trash_dir, &filename, id.as_deref()).unwrap() {
                FoundTrashItems::Single(item) => {
                    let item_path = OPTS.trash_dir.join(item.trash_filename());
                    let target_path = to.clone()
                        .unwrap_or_else(|| std::env::current_dir().unwrap())
                        .join(item.filename());

                    if target_path.exists() {
                        if !force {
                            fail!("Target path already exists, use '-f' / '--force' to override the existing item.");
                        }

                        debug!("Restoration path already exists, permanently removing it...");

                        if let Err(err) = fs::remove_dir_all(&target_path) {
                            fail!("Failed to remove existing item at restoration path: {}", err);
                        }
                    }

                    debug!("Restoring item from trash...");

                    if let Err(err) = fs::rename(&item_path, &target_path) {
                        fail!("Failed to restore item '{}' from trash: {}", item.filename(), err);
                    }
                },

                FoundTrashItems::Multi(candidates) => println!(
                    "Multiple items with this filename were found in the trash:{}",
                    candidates.iter().map(|c| format!("\n* {}", c)).collect::<String>()
                )
            }
        },

        Action::Clear(EmptyTrash {}) => {
            debug!("Emptying the trash...");

            // TODO: Ask confirmation
            
            if let Err(err) = fs::remove_dir_all(&OPTS.trash_dir) {
                fail!("Failed to empty trash: {}", err);
            }

            fs::create_dir_all(&OPTS.trash_dir).unwrap();
            println!("Trash has been emptied.");
        }
    }

    debug!("Done.");
}
