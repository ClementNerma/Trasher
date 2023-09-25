use anyhow::Context;
use anyhow::Result;

use super::args::*;
use super::fsutils::*;
use super::items::*;
use super::{bail, debug};

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

pub fn list(action: ListTrashItems, trash_dir: &Path) -> Result<()> {
    let ListTrashItems { name, details } = action;

    debug!("Listing trash items...");
    let mut items = list_trash_items(trash_dir)?;

    if let Some(name) = &name {
        debug!("Filtering {} items by name...", items.len());
        items.retain(|item| item.filename().contains(name));
    }

    items.sort_by(|a, b| a.datetime().cmp(b.datetime()));

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
                    if details {
                        let details = get_fs_details(complete_trash_item_path(&item, trash_dir))?;

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

            if details {
                println!(
                    "{} directories, {} files, total size is: {}",
                    total_dirs,
                    total_files,
                    human_readable_size(total_size)
                );
            }
        }
    }

    Ok(())
}

pub fn remove(action: MoveToTrash, trash_dir: &Path) -> Result<()> {
    let MoveToTrash {
        paths,
        permanently,
        ignore,
        dont_move_ext_filesystems,
        size_limit_move_ext_filesystems,
        allow_invalid_utf8_item_names,
    } = action;

    let size_limit_move_ext_filesystems = size_limit_move_ext_filesystems
        .as_ref()
        .map(|size_limit| {
            parse_human_readable_size(size_limit)
                .context("Invalid size limit provided for externals filesystems' items moving")
        })
        .transpose()?;

    debug!("Going to remove {} item(s)...", paths.len());

    for (i, path) in paths.iter().enumerate() {
        debug!("Treating item {} on {}...", i + 1, paths.len());

        let path = PathBuf::from(path);

        debug!("Checking if item exists...");

        if is_dangerous_path(&path) {
            bail!("Removing this path is too dangerous, operation aborted.");
        }

        if !path.exists() {
            if ignore {
                continue;
            }

            bail!("Item path does not exist.");
        }

        if permanently {
            let deletion_result = if path.is_file() {
                fs::remove_file(&path)
            } else {
                fs::remove_dir_all(&path)
            };

            match deletion_result {
                Err(err) => bail!("Failed to permanently remove item: {}", err),
                Ok(()) => continue,
            }
        }

        let filename = path
            .file_name()
            .context("Specified item path has no file name")?;

        let filename = match filename.to_str() {
            Some(str) => str.to_string(),
            None => {
                if allow_invalid_utf8_item_names {
                    filename.to_string_lossy().to_string()
                } else {
                    bail!("Specified item does not have a valid UTF-8 file name")
                }
            }
        };

        let item_metadata = path.metadata().with_context(|| {
            format!(
                "Failed to get metadata for item at path '{}'",
                path.to_string_lossy(),
            )
        })?;

        let trash_item = TrashItem::new_now(filename.to_string(), Some(item_metadata.file_type()));

        debug!(
            "Moving item to trash under name '{}'...",
            trash_item.trash_filename()
        );

        let trash_item_path = transfer_trash_item_path(&trash_item, trash_dir);

        if let Err(err) = fs::rename(&path, &trash_item_path) {
            debug!("Renaming failed: {:?}", err);

            // HACK: *MUST* be removed when this issue is resolved: https://github.com/rust-lang/rust/issues/86442
            if err.kind().to_string() == "cross-device link or rename" {
                if dont_move_ext_filesystems {
                    bail!("Failed to move item to trash: {}\nHelp: Item may be located on another drive, try removing '--move-ext-filesystems'.", err);
                }
            } else if err.kind() != ErrorKind::Other {
                bail!(
                    "An error occured while trying to move item to trash: {}",
                    err
                );
            }

            debug!("Falling back to copying.");

            if let Some(size_limit) = size_limit_move_ext_filesystems {
                debug!(
                    "Size limit was provided: {}",
                    human_readable_size(size_limit)
                );
                debug!("Computing size of the item to remove...");

                let details = get_fs_details(&path)
                    .context("Failed to compute size of item before sending it to the trash: {}")?;

                if details.size > size_limit {
                    bail!(
                        "This item ({}) is larger than the provided size limit ({}).",
                        human_readable_size(details.size),
                        human_readable_size(size_limit)
                    );
                }
            }

            move_item_pbr(&path, &trash_item_path)
                .context("Failed to move item to trash (using copying fallback)")?;
        }

        let mut rename_errors = 0;

        while let Err(err) = move_transferred_trash_item(&trash_item, trash_dir) {
            rename_errors += 1;

            debug!(
                "Failed to rename transferred item in trash (try n°{}): {}",
                rename_errors, err
            );

            if rename_errors == 5 {
                bail!(
                    "Failed to rename transferred item in trash after {} tries: {}",
                    rename_errors,
                    err
                );
            }
        }
    }

    Ok(())
}

pub fn drop(action: DropItem, trash_dir: &Path) -> Result<()> {
    let DropItem { filename, id } = action;

    debug!("Listing trash items...");

    let item = expect_single_trash_item(trash_dir, &filename, id.as_deref())?;
    let item_path = complete_trash_item_path(&item, trash_dir);

    debug!("Permanently removing item from trash...");

    fs::remove_dir_all(item_path)
        .with_context(|| format!("Failed to remove item '{}' from trash", item.filename()))
}

pub fn path_of(action: GetItemPath, trash_dir: &Path) -> Result<()> {
    let GetItemPath {
        filename,
        id,
        allow_invalid_utf8_path,
    } = action;

    debug!("Listing trash items...");

    let item = expect_single_trash_item(trash_dir, &filename, id.as_deref())?;
    let item_path = complete_trash_item_path(&item, trash_dir);

    match item_path.to_str() {
        Some(path) => println!("{}", path),
        None => {
            if allow_invalid_utf8_path {
                println!("{}", item_path.to_string_lossy())
            } else {
                bail!(
                    "Path contains invalid UTF-8 characters (lossy: {})",
                    item_path.to_string_lossy()
                );
            }
        }
    }

    Ok(())
}

pub fn restore(action: RestoreItem, trash_dir: &Path) -> Result<()> {
    let RestoreItem {
        filename,
        to,
        id,
        force,
        move_ext_filesystems,
    } = action;

    debug!("Listing trash items...");

    match expect_trash_item(trash_dir, &filename, id.as_deref())? {
        FoundTrashItems::Single(item) => {
            let item_path = complete_trash_item_path(&item, trash_dir);

            let target_path = match to {
                Some(to) => to,
                None => std::env::current_dir()?,
            };

            let target_path = target_path.join(item.filename());

            if target_path.exists() {
                if !force {
                    bail!("Target path already exists, use '-f' / '--force' to override the existing item.");
                }

                debug!("Restoration path already exists, permanently removing it...");

                fs::remove_dir_all(&target_path)
                    .context("Failed to remove existing item at restoration path")?;
            }

            debug!("Restoring item from trash...");

            if let Err(err) = fs::rename(&item_path, &target_path) {
                if !move_ext_filesystems {
                    bail!(
                        "Failed to restore item '{}' from trash: {}",
                        item.filename(),
                        err
                    );
                }

                debug!("Renaming failed: {}", err);
                debug!("Falling back to copying.");

                move_item_pbr(&item_path, &target_path)
                    .context("Failed to restore item from trash (using copying fallback)")?;
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

    Ok(())
}

pub fn clear(action: EmptyTrash, trash_dir: &Path) -> Result<()> {
    let EmptyTrash {} = action;

    debug!("Emptying the trash...");

    // TODO: Ask confirmation

    fs::remove_dir_all(trash_dir).context("Failed to empty the trash")?;
    fs::create_dir_all(trash_dir).context("Failed to re-create trash directory")?;

    println!("Trash has been emptied.");

    Ok(())
}
