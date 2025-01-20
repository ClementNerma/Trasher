use std::{fs, io::stdin, path::PathBuf};

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use jiff::Zoned;
use log::{debug, info, warn};

use crate::fuzzy::FuzzyFinderItem;

use super::{args::*, bail, fsutils::*, items::*};

pub fn list(action: ListTrashItems, config: &Config) -> Result<()> {
    let ListTrashItems { name } = action;

    debug!("Listing trash items...");

    let mut items = list_all_trash_items(config)?;

    if items.is_empty() {
        info!("All trashes are empty.");
        return Ok(());
    }

    if let Some(name) = &name {
        debug!("Filtering {} items by name...", items.len());
        items.retain(|trashed| trashed.data.filename.contains(name));

        if items.is_empty() {
            info!("No item in trash match the provided name.");
            return Ok(());
        }
    }

    println!("{}", table_for_items(&items));

    Ok(())
}

pub fn remove(action: MoveToTrash, config: &Config) -> Result<()> {
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

            bail!("No item exists at path: {}", path.display());
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

        let data = TrashItemInfos::new_now(filename.to_string());

        debug!(
            "Moving item to trash under name '{}'...",
            data.trash_filename()
        );

        let trash_dir = determine_trash_dir_for(&path, config).with_context(|| {
            format!(
                "Failed to determine path to the trash directory for item: {}",
                path.display()
            )
        })?;

        if !trash_dir.exists() {
            fs::create_dir(&trash_dir).with_context(|| {
                format!(
                    "Failed to create trash directory at path '{}'",
                    trash_dir.display()
                )
            })?;
        }

        let trash_transfer_dir = trash_dir.join(TRASH_TRANSFER_DIRNAME);

        if !trash_transfer_dir.exists() {
            fs::create_dir(&trash_transfer_dir).with_context(|| {
                format!(
                    "Failed to create trash's partial transfer directory at path '{}'",
                    trash_transfer_dir.display()
                )
            })?;
        }

        if !are_on_same_fs(&path, &trash_dir)? {
            info!("Moving item to trash directory {}", trash_dir.display());

            let transfer_path = trash_transfer_dir.join(data.trash_filename());

            move_item_pbr(&path, &transfer_path).context("Failed to move item to the trash")?;

            fs::rename(&transfer_path, trash_dir.join(data.trash_filename()))
                .context("Failed to move item to the final trash directory")?;
        } else {
            let trash_item = TrashedItem { data, trash_dir };
            let trash_item_path = trash_item.transfer_trash_item_path();

            fs::rename(&path, &trash_item_path)
                .with_context(|| format!("Failed to move item '{}' to trash", path.display()))?;

            fs::rename(&trash_item_path, trash_item.complete_trash_item_path()).with_context(
                || {
                    format!(
                        "Failed to move fully transferred item '{}' to trash",
                        path.display()
                    )
                },
            )?;
        }
    }

    Ok(())
}

pub fn drop(action: DropItem, config: &Config) -> Result<()> {
    let DropItem { filename, id } = action;

    debug!("Listing trash items...");

    let item = expect_single_trash_item(&filename, id.as_deref(), config)?;

    debug!("Permanently removing item from trash...");

    let path = item.complete_trash_item_path();

    let result = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };

    result.with_context(|| format!("Failed to remove item '{}' from trash", item.data.filename))
}

pub fn path_of(action: GetItemPath, config: &Config) -> Result<()> {
    let GetItemPath {
        filename,
        id,
        allow_invalid_utf8_path,
    } = action;

    debug!("Listing trash items...");

    let item = expect_single_trash_item(&filename, id.as_deref(), config)?;
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

pub fn restore(action: RestoreItem, config: &Config) -> Result<()> {
    let RestoreItem { filename, to, id } = action;

    debug!("Listing trash items...");

    let Some(filename) = filename else {
        return restore_with_ui(config);
    };

    let item = expect_single_trash_item(&filename, id.as_deref(), config)?;

    let item_path = item.complete_trash_item_path();

    let target_path = match to {
        Some(to) => to,
        None => std::env::current_dir()?,
    };

    let target_path = target_path.join(&item.data.filename);

    if target_path.exists() {
        bail!("Target path already exists.");
    }

    let target_parent = target_path.parent().unwrap();

    if !target_parent.exists() {
        bail!(
            "Target directory '{}' does not exist",
            target_parent.display()
        );
    }

    let result = if are_on_same_fs(&item.complete_trash_item_path(), target_parent)? {
        debug!("Restoring item from trash...");

        fs::rename(item_path, &target_path).context("Rename operation failed")
    } else {
        info!("Moving file across filesystems...");

        move_item_pbr(&item_path, &target_path)
    };

    result.with_context(|| format!("Failed to restore item '{}' from trash", item.data.filename))
}

pub fn restore_with_ui(config: &Config) -> Result<()> {
    let items = list_all_trash_items(config)?;

    if items.is_empty() {
        info!("Trash is empty");
        return Ok(());
    }

    let to_remove = crate::fuzzy::run_fuzzy_finder(
        items
            .into_iter()
            .map(|item| FuzzyFinderItem {
                display: format!(
                    "[{}] {}",
                    Zoned::try_from(item.data.datetime)
                        .and_then(|date| jiff::fmt::rfc2822::to_string(&date))
                        .unwrap_or_else(|_| "<Failed to format date>".to_owned()),
                    item.data.filename
                ),
                value: item,
            })
            .collect(),
    )?;

    restore(
        RestoreItem {
            filename: Some(to_remove.data.filename.to_owned()),
            to: None,
            id: Some(to_remove.data.compute_id().to_owned()),
        },
        config,
    )?;

    Ok(())
}

pub fn empty(config: &Config) -> Result<()> {
    let trash_dirs = list_trash_dirs(config)?;
    let items = list_all_trash_items(config)?;

    if items.is_empty() {
        info!("Trash is empty");
        return Ok(());
    }

    warn!("You are about to delete the entire trash directories of:\n");

    for trash_dir in &trash_dirs {
        warn!(
            "  {} ({} items)",
            trash_dir.display(),
            items
                .iter()
                .filter(|item| &item.trash_dir == trash_dir)
                .count()
        );
    }

    warn!("\nAre you sure you want to continue [y/N]?");

    let mut confirm_str = String::new();

    stdin()
        .read_line(&mut confirm_str)
        .context("Failed to get user confirmation")?;

    if !confirm_str.trim().eq_ignore_ascii_case("y") {
        warn!("Cancelled.");
        return Ok(());
    }

    info!("Emptying the trash...");

    for trash_dir in trash_dirs {
        info!("Emptying trash directory: {}", trash_dir.display());

        warn!("> Listing files and directories to delete...");

        let items = list_deletable_fs_items(&trash_dir)?;

        warn!("> Deleting all {} items...", items.len());

        let pbr = ProgressBar::new(items.len().try_into().unwrap());

        pbr.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {human_pos}/{human_len} ({eta})")
            .expect("Invalid progress bar template")
            .progress_chars("#>-"));

        for (i, item) in items.iter().enumerate() {
            let metadata = item
                .symlink_metadata()
                .with_context(|| format!("Failed to get metadata for item: {}", item.display()))?
                .file_type();

            if metadata.is_dir() {
                fs::remove_dir(item)
                    .with_context(|| format!("Failed to remove directory: {}", item.display()))?;
            } else {
                fs::remove_file(item)
                    .with_context(|| format!("Failed to remove file: {}", item.display()))?;
            }

            if i % 25 == 0 || i + 1 == items.len() {
                pbr.set_position((i + 1).try_into().unwrap());
            }
        }

        pbr.finish();
    }

    info!("Trash was successfully emptied.");

    Ok(())
}

pub fn trash_path(config: &Config) -> Result<()> {
    let current_dir =
        std::env::current_dir().context("Failed to determine path to the current directory")?;

    let trash_dir = determine_trash_dir_for(&current_dir, config)?;

    println!("{}", trash_dir.display());

    Ok(())
}
