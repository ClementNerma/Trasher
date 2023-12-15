use anyhow::Context;
use anyhow::Result;

use crate::fuzzy::FuzzyFinderItem;

use super::args::*;
use super::fsutils::*;
use super::items::*;
use super::{bail, debug};

use std::fs;
use std::io::stdin;
use std::io::ErrorKind;
use std::path::PathBuf;

pub fn list(action: ListTrashItems) -> Result<()> {
    let ListTrashItems { name, details } = action;

    debug!("Listing trash items...");

    let mut items = list_all_trash_items()?.collect::<Vec<_>>();

    if let Some(name) = &name {
        debug!("Filtering {} items by name...", items.len());
        items.retain(|trashed| trashed.data.filename().contains(name));
    }

    items.sort_by(|a, b| a.data.datetime().cmp(b.data.datetime()));

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
                let TrashedItem { data, trash_dir } = &item;

                println!(
                    "* {data}{}",
                    if details {
                        let details = get_fs_details(item.complete_trash_item_path())?;

                        let dir_one = if details.is_dir { 1 } else { 0 };

                        total_size += details.size;
                        total_dirs += details.sub_files + dir_one;
                        total_files += details.sub_files + (1 - dir_one);

                        format!("{}, trash dir: {}", details, trash_dir.display())
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

pub fn remove(action: MoveToTrash) -> Result<()> {
    let MoveToTrash {
        paths,
        permanently,
        ignore,
        allow_invalid_utf8_item_names,
    } = action;

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

        let trash_dir = determine_trash_dir_for(&path).with_context(|| {
            format!(
                "Failed to determine path to the trash directory for item: {}",
                path.display()
            )
        })?;

        let trash_item = TrashedItem {
            data: trash_item,
            trash_dir,
        };

        let trash_item_path = trash_item.transfer_trash_item_path();

        if let Err(err) = fs::rename(&path, &trash_item_path) {
            debug!("Renaming failed: {:?}", err);

            // HACK: *MUST* be removed when this issue is resolved: https://github.com/rust-lang/rust/issues/86442
            if err.kind().to_string() == "cross-device link or rename" {
                bail!("Failed to move item to trash: {}\n\nDetails: tried to move item:\n   {}\n-> {}", err, path.display(), trash_item_path.display());
            } else if err.kind() != ErrorKind::Other {
                bail!(
                    "An error occured while trying to move item to trash: {}",
                    err
                );
            }
        }

        let mut rename_errors = 0;

        while let Err(err) = move_transferred_trash_item(&trash_item) {
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

pub fn drop(action: DropItem) -> Result<()> {
    let DropItem { filename, id } = action;

    debug!("Listing trash items...");

    let item = expect_single_trash_item(&filename, id.as_deref())?;

    debug!("Permanently removing item from trash...");

    fs::remove_dir_all(item.complete_trash_item_path()).with_context(|| {
        format!(
            "Failed to remove item '{}' from trash",
            item.data.filename()
        )
    })
}

pub fn path_of(action: GetItemPath) -> Result<()> {
    let GetItemPath {
        filename,
        id,
        allow_invalid_utf8_path,
    } = action;

    debug!("Listing trash items...");

    let item = expect_single_trash_item(&filename, id.as_deref())?;
    let item_path = item.complete_trash_item_path();

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

pub fn restore(action: RestoreItem) -> Result<()> {
    let RestoreItem { filename, to, id } = action;

    debug!("Listing trash items...");

    let Some(filename) = filename else {
        return restore_with_ui();
    };

    match expect_trash_item(&filename, id.as_deref())? {
        FoundTrashItems::Single(item) => {
            let item_path = item.complete_trash_item_path();

            let target_path = match to {
                Some(to) => to,
                None => std::env::current_dir()?,
            };

            let target_path = target_path.join(item.data.filename());

            if target_path.exists() {
                bail!("Target path already exists.");
            }

            debug!("Restoring item from trash...");

            if let Err(err) = fs::rename(item_path, &target_path) {
                bail!(
                    "Failed to restore item '{}' from trash: {}",
                    item.data.filename(),
                    err
                );
            }
        }

        FoundTrashItems::Multi(candidates) => println!(
            "Multiple items with this filename were found in the trash:{}",
            candidates
                .iter()
                .map(|trashed| format!("\n* {}", trashed.data))
                .collect::<String>()
        ),
    }

    Ok(())
}

pub fn restore_with_ui() -> Result<()> {
    let mut items = list_all_trash_items()?.collect::<Vec<_>>();

    if items.is_empty() {
        println!("Trash is empty");
        return Ok(());
    }

    items.sort_by(|a, b| a.data.datetime().cmp(b.data.datetime()));

    let to_remove = crate::fuzzy::run_fuzzy_finder(
        items
            .into_iter()
            .map(|item| FuzzyFinderItem {
                display: format!("[{}] {}", item.data.datetime(), item.data.filename()),
                value: item,
            })
            .collect(),
    )?;

    restore(RestoreItem {
        filename: Some(to_remove.data.filename().to_owned()),
        to: None,
        id: Some(to_remove.data.id().to_owned()),
    })?;

    Ok(())
}

pub fn empty(action: EmptyTrash) -> Result<()> {
    let EmptyTrash {} = action;

    let current_dir =
        std::env::current_dir().context("Failed to determine path to the current directory")?;

    let trash_dir = determine_trash_dir_for(&current_dir)?;

    println!(
        "You are about to delete the entire directory of {}",
        trash_dir.display()
    );

    println!("Are you sure you want to continue? If so, type 'Y' or 'y' then <Return> / <Enter>");

    let mut confirm_str = String::new();

    stdin()
        .read_line(&mut confirm_str)
        .context("Failed to get user confirmation")?;

    if confirm_str.trim().to_ascii_lowercase() != "y" {
        println!("Cancelled.");
        return Ok(());
    }

    println!("Emptying the trash...");

    fs::remove_dir_all(&trash_dir)
        .with_context(|| format!("Failed to empty the trash at path: {}", trash_dir.display()))?;

    fs::create_dir_all(&trash_dir).with_context(|| {
        format!(
            "Failed to re-create trash directory at path: {}",
            trash_dir.display()
        )
    })?;

    println!("Trash was successfully emptied.");

    Ok(())
}
