use crate::debug;

use super::items::TrashItem;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use fs_extra::dir::TransitProcessResult;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use mountpoints::mountpaths;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Name of the trash directory
const TRASH_DIR_NAME: &str = ".trasher";

/// Name of the transfer directory in the trash
pub const TRASH_TRANSFER_DIRNAME: &str = "#PARTIAL";

/// Determine path to the trash directory for a given item and create it if required
pub fn determine_trash_dir_for(item: &Path) -> Result<PathBuf> {
    debug!("Determining trasher directory for item: {}", item.display());

    let parent_dir = match determine_mountpoint_for(item)? {
        Some(path) => path,
        None => dirs::home_dir().context("Failed to determine path to user's home directory")?,
    };

    let trash_dir_path = parent_dir.join(TRASH_DIR_NAME);

    if !trash_dir_path.exists() {
        fs::create_dir_all(trash_dir_path.join(TRASH_TRANSFER_DIRNAME)).with_context(|| {
            format!(
                "Failed to create trash directory (with transfer directory) at: {}",
                trash_dir_path.display()
            )
        })?;
    }

    Ok(trash_dir_path)
}

/// Determine the (canonicalized) path to the mountpoint the provided path is on
pub fn determine_mountpoint_for(item: &Path) -> Result<Option<PathBuf>> {
    let item = fs::canonicalize(item)
        .with_context(|| format!("Failed to canonicalize item path: {}", item.display()))?;

    let mountpoints = mountpaths().context("Failed to list system mountpoints")?;

    for mountpoint in &mountpoints {
        if mountpoint.to_str() == Some("/") {
            continue;
        }

        let canon_mountpoint = fs::canonicalize(mountpoint).with_context(|| {
            format!(
                "Failed to canonicalize mountpoint: {}",
                mountpoint.display()
            )
        })?;

        if item.starts_with(&canon_mountpoint) {
            return Ok(Some(canon_mountpoint));
        }
    }

    Ok(None)
}

/// List all trash directories
pub fn list_trash_dirs() -> Result<BTreeSet<PathBuf>> {
    let canon_root = fs::canonicalize("/").context("Failed to canonicalize the root directory")?;

    let trash_dirs = mountpaths()
        .context("Failed to list system mountpoints")?
        .iter()
        .chain([canon_root].iter())
        .map(|path| determine_trash_dir_for(path))
        .collect::<Result<BTreeSet<_>, _>>()?;

    Ok(trash_dirs)
}

/// List and parse all items in the trash
pub fn list_trash_items(trash_dir: &Path) -> Result<Vec<TrashedItem>> {
    let dir_entries = fs::read_dir(trash_dir)
        .context("Failed to read trash directory")?
        .collect::<Result<Vec<_>, _>>()?;

    let items = dir_entries
        .into_iter()
        .filter_map(|item| {
            match item.file_name().into_string() {
                Err(invalid_filename) => eprintln!(
                    "WARN: Trash item '{}' does not have a valid UTF-8 filename!",
                    invalid_filename.to_string_lossy()
                ),

                Ok(filename) => {
                    if filename == TRASH_TRANSFER_DIRNAME {
                        return None;
                    }

                    match TrashItem::decode(&filename, Some(item.file_type().unwrap())) {
                        Err(err) => {
                            eprintln!(
                                "WARN: Trash item '{}' does not have a valid trash filename!",
                                filename
                            );
                            super::debug!("Invalid trash item filename: {:?}", err);
                        }

                        Ok(item) => {
                            return Some(TrashedItem {
                                data: item,
                                trash_dir: trash_dir.to_path_buf(),
                            })
                        }
                    }
                }
            }

            None
        })
        .collect();

    Ok(items)
}

/// List all trash items
pub fn list_all_trash_items() -> Result<impl Iterator<Item = TrashedItem>> {
    let all_trash_items = list_trash_dirs()?
        .into_iter()
        .map(|trash_dir| list_trash_items(&trash_dir))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(all_trash_items.into_iter().flatten())
}

/// Find a specific item in the trash (panic if not found)
pub fn expect_trash_item(filename: &str, id: Option<&str>) -> Result<FoundTrashItems> {
    let mut candidates = list_all_trash_items()?
        .filter(|trashed| trashed.data.filename() == filename)
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        bail!("Specified item was not found in the trash.");
    } else if candidates.len() > 1 {
        match id {
            None => Ok(FoundTrashItems::Multi(candidates)),
            Some(id) => Ok(FoundTrashItems::Single(
                candidates
                    .into_iter()
                    .find(|c| c.data.id() == id)
                    .context("There is no trash item with the provided ID")?,
            )),
        }
    } else {
        Ok(FoundTrashItems::Single(candidates.remove(0)))
    }
}

/// Find a specific item in the trash, fail if none is found or if multiple candidates are found
pub fn expect_single_trash_item(filename: &str, id: Option<&str>) -> Result<TrashedItem> {
    match expect_trash_item(filename, id)? {
        FoundTrashItems::Single(item) => Ok(item),
        FoundTrashItems::Multi(candidates) => bail!(
            "Multiple items with this filename were found in the trash:{}",
            candidates
                .iter()
                .map(|TrashedItem { data, trash_dir }| format!(
                    "\n* {data} (from {})",
                    trash_dir.display()
                ))
                .collect::<String>()
        ),
    }
}

/// Get details on a filesystem item
pub fn get_fs_details(path: impl AsRef<Path>) -> Result<FSDetails> {
    let metadata = fs::metadata(&path)?;

    let is_dir = metadata.is_dir();

    if metadata.file_type().is_symlink() {
        return Ok(FSDetails {
            is_symlink: true,
            is_dir,
            sub_directories: 0,
            sub_files: 0,
            size: 0,
        });
    }

    if !is_dir {
        return Ok(FSDetails {
            is_symlink: false,
            is_dir: false,
            sub_directories: 0,
            sub_files: 0,
            size: metadata.len(),
        });
    }

    let mut details = FSDetails {
        is_symlink: false,
        is_dir: true,
        sub_directories: 0,
        sub_files: 0,
        size: 0,
    };

    for item in fs::read_dir(&path)? {
        let item_details = get_fs_details(item?.path())?;
        let dir_one = if item_details.is_dir { 1 } else { 0 };

        details.sub_directories += item_details.sub_directories + dir_one;
        details.sub_files += item_details.sub_files + (1 - dir_one);
        details.size += item_details.size;
    }

    Ok(details)
}

/// Move a partial item to the trash's main directory once the transfer is complete
pub fn move_transferred_trash_item(item: &TrashedItem) -> Result<()> {
    fs::rename(
        item.transfer_trash_item_path(),
        item.complete_trash_item_path(),
    )?;

    Ok(())
}

/// Convert a size in bytes to a human-readable size
pub fn human_readable_size(bytes: u64) -> String {
    let names = ["KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];

    if bytes < 1024 {
        return format!("{} B", bytes);
    }

    let mut compare = 1024;

    for name in names.iter() {
        compare *= 1024;

        if bytes <= compare {
            return format!("{:.2} {}", bytes as f64 * 1024f64 / compare as f64, name);
        }
    }

    format!(
        "{:.2} {}",
        bytes as f64 / compare as f64,
        names.last().unwrap()
    )
}

/// Trash item with the trash directory is contained into, generated by the [`list_trash_items`] function
#[derive(Debug, Clone)]
pub struct TrashedItem {
    pub data: TrashItem,
    pub trash_dir: PathBuf,
}

impl TrashedItem {
    /// Get the trash path for an item that's going to be transferred to it
    pub fn transfer_trash_item_path(&self) -> PathBuf {
        self.trash_dir
            .join(TRASH_TRANSFER_DIRNAME)
            .join(self.data.trash_filename())
    }

    pub fn complete_trash_item_path(&self) -> PathBuf {
        self.trash_dir.join(self.data.trash_filename())
    }
}

/// Trash items found with the [`expect_trash_item`] function
pub enum FoundTrashItems {
    Single(TrashedItem),
    Multi(Vec<TrashedItem>),
}

/// Details on a filesystem item returned by the [`get_fs_details`] function
pub struct FSDetails {
    pub is_symlink: bool,
    pub is_dir: bool,
    pub sub_directories: u64,
    pub sub_files: u64,
    pub size: u64,
}

impl fmt::Display for FSDetails {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            " | [{}] Size: {}{}",
            if self.is_symlink {
                "Symlink"
            } else if self.is_dir {
                "Directory"
            } else {
                "File"
            },
            human_readable_size(self.size),
            if self.is_dir {
                format!(
                    ", Items: {}, Directories: {}, Files: {}",
                    self.sub_directories + self.sub_files,
                    self.sub_directories,
                    self.sub_files
                )
            } else {
                "".to_string()
            }
        )
    }
}

// Check if a path is dangerous to delete
pub fn is_dangerous_path(path: &Path) -> bool {
    let mut components = path.components();

    match (
        components.next(),
        components.next(),
        components.next(),
        components.next(),
    ) {
        // The root directory (/)
        (Some(Component::RootDir), None, None, None) => true,

        // Root directories (/home, /bin, etc.)
        (Some(Component::RootDir), Some(_), None, None) => true,

        // Home directories (/home/username, etc.)
        (Some(Component::RootDir), Some(Component::Normal(dir)), Some(_), None) => {
            dir == OsStr::new("home")
        }

        // Non-dangerous paths
        _ => false,
    }
}

/// Move items around with a progressbar
pub fn move_item_pbr(path: &Path, target: &Path) -> Result<()> {
    let pbr = Rc::new(RefCell::new(None));

    let update_pbr = |copied, total, item_name: &str| {
        let mut pbr = pbr.borrow_mut();
        let pbr = pbr.get_or_insert_with(|| {
            let pbr = ProgressBar::new(total);
            pbr.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .expect("Invalid progress bar template")
            .progress_chars("#>-"));
            pbr
        });

        pbr.set_position(copied);
        pbr.set_message(item_name.to_string());
    };

    if path.metadata()?.is_file() {
        let file_name = path.file_name().unwrap().to_string_lossy();

        fs_extra::file::move_file_with_progress(
            path,
            target,
            &fs_extra::file::CopyOptions::new(),
            |tp| {
                update_pbr(tp.copied_bytes, tp.total_bytes, &file_name);
            },
        )?;
    } else {
        let mut config = fs_extra::dir::CopyOptions::new();
        config.copy_inside = true;
        fs_extra::dir::move_dir_with_progress(path, target, &config, |tp| {
            update_pbr(tp.copied_bytes, tp.total_bytes, &tp.file_name);
            TransitProcessResult::ContinueOrAbort
        })?;
    }

    let mut pbr = pbr.borrow_mut();
    let pbr = pbr.as_mut();

    if let Some(pbr) = pbr {
        pbr.finish_with_message("Moving complete.")
    }

    Ok(())
}
