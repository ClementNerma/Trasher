use std::fs;
use std::path::PathBuf;
use super::{debug, fail};
use super::command::*;
use super::items::*;
use super::fsutils::*;

pub fn list(action: &ListTrashItems) {
    let ListTrashItems { name, details } = action;

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
                    let details = get_fs_details(complete_trash_item_path(&item)).unwrap();
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
}

pub fn remove(action: &MoveToTrash) {
    let MoveToTrash {
        path,
        permanently,
        allow_invalid_utf8_item_names,
        move_ext_filesystems
    } = action;

    let path = PathBuf::from(path);

    debug!("Checking if item exists...");

    if !path.exists() {
        fail!("Item path does not exist.");
    }

    if *permanently {
        match fs::remove_dir_all(&path) {
            Err(err) => fail!("Failed to permanently remove item: {}", err),
            Ok(()) => return
        }
    }

    let filename = path.file_name().unwrap_or_else(|| fail!("Specified item path has no file name"));
    let filename = match filename.to_str() {
        Some(str) => str.to_string(),
        None => if *allow_invalid_utf8_item_names {
            filename.to_string_lossy().to_string()
        } else {
            fail!("Specified item does not have a valid UTF-8 file name")
        }
    };

    let trash_item = TrashItem::new_now(filename.to_string());

    debug!("Moving item to trash under name '{}'...", trash_item.trash_filename());

    let trash_item_path = transfer_trash_item_path(&trash_item);

    if let Err(err) = fs::rename(&path, &trash_item_path) {
        if !move_ext_filesystems {
            fail!("Failed to move item to trash: {}", err);
        }
        
        debug!("Renaming failed: {}", err);
        debug!("Falling back to copying.");

        move_item_pbr(&path, &trash_item_path).unwrap_or_else(|err|
            fail!("Failed to move item to trash (using copying fallback): {}", err)
        )
    }

    let mut rename_errors = 0;

    while let Err(err) = move_transferred_trash_item(&trash_item) {
        rename_errors += 1;

        debug!("Failed to rename transferred item in trash (try n°{}): {}", rename_errors, err);

        if rename_errors == 5 {
            fail!("Failed to rename transferred item in trash after {} tries: {}", rename_errors, err);
        }
    }
}

pub fn drop(action: &DropItem) {
    let DropItem { filename, id } = action;
    
    debug!("Listing trash items...");

    match expect_trash_item(&OPTS.trash_dir, &filename, id.as_deref()).unwrap() {
        FoundTrashItems::Single(item) => {
            let item_path = complete_trash_item_path(&item);

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
}

pub fn restore(action: &RestoreItem) {
    let RestoreItem {
        filename,
        to,
        id,
        force,
        move_ext_filesystems
    } = action;

    debug!("Listing trash items...");

    match expect_trash_item(&OPTS.trash_dir, &filename, id.as_deref()).unwrap() {
        FoundTrashItems::Single(item) => {
            let item_path = complete_trash_item_path(&item);
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
                if !move_ext_filesystems {
                    fail!("Failed to restore item '{}' from trash: {}", item.filename(), err);
                }
                
                debug!("Renaming failed: {}", err);
                debug!("Falling back to copying.");

                move_item_pbr(&item_path, &target_path).unwrap_or_else(|err|
                    fail!("Failed to restore item from trash (using copying fallback): {}", err)
                )
            }
        },

        FoundTrashItems::Multi(candidates) => println!(
            "Multiple items with this filename were found in the trash:{}",
            candidates.iter().map(|c| format!("\n* {}", c)).collect::<String>()
        )
    }
}

pub fn clear(action: &EmptyTrash) {
    let EmptyTrash {} = action;

    debug!("Emptying the trash...");

    // TODO: Ask confirmation
    
    if let Err(err) = fs::remove_dir_all(&OPTS.trash_dir) {
        fail!("Failed to empty trash: {}", err);
    }

    fs::create_dir_all(&OPTS.trash_dir).unwrap();
    println!("Trash has been emptied.");
}