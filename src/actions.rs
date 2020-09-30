use super::command::*;
use super::fsutils::*;
use super::items::*;
use super::{debug, fail};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

pub fn list(action: &ListTrashItems) {
    let ListTrashItems { name, details } = action;

    debug!("Listing trash items...");
    let mut items = list_trash_items(&OPTS.trash_dir).unwrap();

    if let Some(name) = &name {
        debug!("Filtering {} items by name...", items.len());
        items = items
            .into_iter()
            .filter(|item| item.filename().contains(name))
            .collect();
    }

    match items.len() {
        0 => println!(
            "{}",
            if name.is_some() {
                "No item in trash match the provided name."
            } else {
                "Trash is empty."
            }
        ),
        count => {
            println!(
                "Found {} item{} in trash:",
                count,
                if count >= 2 { "s" } else { "" }
            );

            let mut total_size = 0;
            let mut total_dirs = 0;
            let mut total_files = 0;

            for item in items {
                println!(
                    "* {}{}",
                    item,
                    if *details || OPTS.verbose {
                        let details = get_fs_details(complete_trash_item_path(&item)).unwrap();
                        let dir_one = if details.is_dir { 1 } else { 0 };

                        total_size += details.size;
                        total_dirs += details.sub_files + dir_one;
                        total_files += details.sub_files + (1 - dir_one);

                        format!("{}", details)
                    } else {
                        "".to_string()
                    }
                );
            }

            if *details || OPTS.verbose {
                println!(
                    "{} directories, {} files, total size is: {}",
                    total_dirs,
                    total_files,
                    human_readable_size(total_size)
                );
            }
        }
    }
}

pub fn remove(action: &MoveToTrash) {
    let MoveToTrash {
        paths,
        permanently,
        ignore,
        move_ext_filesystems,
        size_limit_move_ext_filesystems,
        allow_invalid_utf8_item_names,
    } = action;

    let size_limit_move_ext_filesystems =
        size_limit_move_ext_filesystems.as_ref().map(|size_limit| {
            parse_human_readable_size(&size_limit).unwrap_or_else(|err| {
                fail!(
                    "Invalid size limit provided for externals filesystems' items moving: {}",
                    err
                )
            })
        });

    debug!("Going to remove {} item(s)...", paths.len());

    for (i, path) in paths.iter().enumerate() {
        debug!("Treating item {} on {}...", i, paths.len());

        let path = PathBuf::from(path);

        debug!("Checking if item exists...");

        if !path.exists() {
            if *ignore {
                return;
            }

            fail!("Item path does not exist.");
        }

        if *permanently {
            match fs::remove_dir_all(&path) {
                Err(err) => fail!("Failed to permanently remove item: {}", err),
                Ok(()) => return,
            }
        }

        let filename = path
            .file_name()
            .unwrap_or_else(|| fail!("Specified item path has no file name"));
        let filename = match filename.to_str() {
            Some(str) => str.to_string(),
            None => {
                if *allow_invalid_utf8_item_names {
                    filename.to_string_lossy().to_string()
                } else {
                    fail!("Specified item does not have a valid UTF-8 file name")
                }
            }
        };

        let trash_item = TrashItem::new_now(filename.to_string());

        debug!(
            "Moving item to trash under name '{}'...",
            trash_item.trash_filename()
        );

        let trash_item_path = transfer_trash_item_path(&trash_item);

        if !TRASH_TRANSFER_DIR.exists() {
            fs::create_dir_all(TRASH_TRANSFER_DIR.as_path()).unwrap_or_else(|err| {
                fail!("Failed to create trash's transfer directory: {}", err)
            });
        }

        if let Err(err) = fs::rename(&path, &trash_item_path) {
            debug!("Renaming failed: {:?}", err);

            if err.kind() != ErrorKind::Other {
                fail!(
                    "An error occured while trying to move item to trash: {}",
                    err
                );
            }

            if !move_ext_filesystems {
                fail!("Failed to move item to trash: {}\nHelp: Item may be located on another drive, try with '--move-ext-filesystems'.", err);
            }

            debug!("Falling back to copying.");

            if let Some(size_limit) = size_limit_move_ext_filesystems {
                debug!(
                    "Size limit was provided: {}",
                    human_readable_size(size_limit)
                );
                debug!("Computing size of the item to remove...");

                let details = get_fs_details(&path).unwrap_or_else(|err| {
                    fail!(
                        "Failed to compute size of item before sending it to the trash: {}",
                        err
                    )
                });

                if details.size > size_limit {
                    fail!(
                        "This item ({}) is larger than the provided size limit ({}).",
                        human_readable_size(details.size),
                        human_readable_size(size_limit)
                    );
                }
            }

            move_item_pbr(&path, &trash_item_path).unwrap_or_else(|err| {
                fail!(
                    "Failed to move item to trash (using copying fallback): {}",
                    err
                )
            })
        }

        let mut rename_errors = 0;

        while let Err(err) = move_transferred_trash_item(&trash_item) {
            rename_errors += 1;

            debug!(
                "Failed to rename transferred item in trash (try n°{}): {}",
                rename_errors, err
            );

            if rename_errors == 5 {
                fail!(
                    "Failed to rename transferred item in trash after {} tries: {}",
                    rename_errors,
                    err
                );
            }
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
                fail!(
                    "Failed to remove item '{}' from trash: {}",
                    item.filename(),
                    err
                );
            }
        }

        FoundTrashItems::Multi(candidates) => println!(
            "Multiple items with this filename were found in the trash:{}",
            candidates
                .iter()
                .map(|c| format!("\n* {}", c))
                .collect::<String>()
        ),
    }
}

pub fn restore(action: &RestoreItem) {
    let RestoreItem {
        filename,
        to,
        id,
        force,
        move_ext_filesystems,
    } = action;

    debug!("Listing trash items...");

    match expect_trash_item(&OPTS.trash_dir, &filename, id.as_deref()).unwrap() {
        FoundTrashItems::Single(item) => {
            let item_path = complete_trash_item_path(&item);
            let target_path = to
                .clone()
                .unwrap_or_else(|| std::env::current_dir().unwrap())
                .join(item.filename());

            if target_path.exists() {
                if !force {
                    fail!("Target path already exists, use '-f' / '--force' to override the existing item.");
                }

                debug!("Restoration path already exists, permanently removing it...");

                if let Err(err) = fs::remove_dir_all(&target_path) {
                    fail!(
                        "Failed to remove existing item at restoration path: {}",
                        err
                    );
                }
            }

            debug!("Restoring item from trash...");

            if let Err(err) = fs::rename(&item_path, &target_path) {
                if !move_ext_filesystems {
                    fail!(
                        "Failed to restore item '{}' from trash: {}",
                        item.filename(),
                        err
                    );
                }

                debug!("Renaming failed: {}", err);
                debug!("Falling back to copying.");

                move_item_pbr(&item_path, &target_path).unwrap_or_else(|err| {
                    fail!(
                        "Failed to restore item from trash (using copying fallback): {}",
                        err
                    )
                })
            }
        }

        FoundTrashItems::Multi(candidates) => println!(
            "Multiple items with this filename were found in the trash:{}",
            candidates
                .iter()
                .map(|c| format!("\n* {}", c))
                .collect::<String>()
        ),
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
