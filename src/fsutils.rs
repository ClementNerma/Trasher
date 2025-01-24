use std::{
    cell::RefCell,
    collections::BTreeSet,
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
    rc::Rc,
};

use anyhow::{bail, Context, Result};
use comfy_table::{presets::UTF8_FULL_CONDENSED, ContentArrangement, Table};
use fs_extra::dir::TransitProcessResult;
use indicatif::{ProgressBar, ProgressStyle};
use jiff::Zoned;
use log::{debug, error, warn};
use mountpoints::mountpaths;
use walkdir::WalkDir;

use super::items::TrashItemInfos;

/// Name of the trash directory
const TRASH_DIR_NAME: &str = ".trasher";

/// Name of the transfer directory in the trash
pub const TRASH_TRANSFER_DIRNAME: &str = ".#PARTIAL";

/// Directories to never create a trash directory for
pub static ALWAYS_EXCLUDE_DIRS: &[&str] = &[
    "/bin",
    "/boot",
    "/dev",
    "/lost+found",
    "/proc",
    "/run",
    "/sbin",
    "/snap",
    "/sys",
    "/usr",
    "/var/lib/docker",
];

/// Compute the list of directories to exclude
pub fn compute_exclusions(exclude_dirs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    debug!("Computing directories to exclude...");

    let mut exclude = exclude_dirs
        .iter()
        .filter(|dir| dir.is_dir())
        .map(|dir| {
            fs::canonicalize(dir).with_context(|| {
                format!(
                    "Failed to canonicalize excluded directory: {}",
                    dir.display()
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Add some directories to always exclude
    exclude.extend(
        ALWAYS_EXCLUDE_DIRS
            .iter()
            .map(Path::new)
            .map(Path::to_owned),
    );

    Ok(exclude)
}

/// Determine path to the trash directory for a given item and create it if required
pub fn determine_trash_dir_for(item: &Path, exclude_dirs: &[PathBuf]) -> Result<PathBuf> {
    debug!("Determining trasher directory for item: {}", item.display());

    let home_dir = dirs::home_dir().context("Failed to determine path to user's home directory")?;

    // Don't canonicalize excluded item paths
    // NOTE: Only works if item path is absolute
    if exclude_dirs.iter().any(|dir| item.starts_with(dir)) {
        return Ok(home_dir.join(TRASH_DIR_NAME));
    }

    let item = fs::canonicalize(item)
        .with_context(|| format!("Failed to canonicalize item path: {}", item.display()))?;

    let mut mountpoints = mountpaths().context("Failed to list system mountpoints")?;

    // Add home directory for specialization
    // e.g. if "/home" is a mounted directory, and we delete an item instead "/home/$USER",
    // this line will allow the algorithm to pick the more specialized "/home/$USER" instead
    mountpoints.push(home_dir.clone());

    let mut found = None::<PathBuf>;

    for mountpoint in &mountpoints {
        if mountpoint.to_str() == Some("/") {
            continue;
        }

        let Ok(mt) = fs::metadata(mountpoint) else {
            continue;
        };

        if mt.permissions().readonly() {
            continue;
        }

        #[cfg(target_family = "unix")]
        {
            use std::os::unix::fs::PermissionsExt;

            // Skip directories without write permissions
            if mt.permissions().mode() & 0o222 == 0 {
                continue;
            }
        }

        let canon_mountpoint = fs::canonicalize(mountpoint).with_context(|| {
            format!(
                "Failed to canonicalize mountpoint: {}",
                mountpoint.display()
            )
        })?;

        if !item.starts_with(&canon_mountpoint) {
            continue;
        }

        if exclude_dirs.iter().any(|parent| item.starts_with(parent)) {
            found = None;
            break;
        }

        if found.is_none() || matches!(found, Some(ref prev) if canon_mountpoint.starts_with(prev))
        {
            found = Some(canon_mountpoint);
        }
    }

    Ok(found.unwrap_or(home_dir).join(TRASH_DIR_NAME))
}

/// List all trash directories
pub fn list_trash_dirs(exclude_dirs: &[PathBuf]) -> Result<BTreeSet<PathBuf>> {
    let canon_root = fs::canonicalize("/").context("Failed to canonicalize the root directory")?;

    let trash_dirs = mountpaths()
        .context("Failed to list system mountpoints")?
        .iter()
        .chain([canon_root].iter())
        .filter(|dir| {
            !exclude_dirs
                .iter()
                .any(|excluded| dir.starts_with(excluded))
        })
        .filter_map(|dir| match fs::metadata(dir) {
            Ok(_) => Some(dir.join(TRASH_DIR_NAME)),
            Err(_) => {
                warn!("Skipping unavailable directory: {}", dir.display());
                None
            }
        })
        .collect();

    Ok(trash_dirs)
}

/// List and parse all items in the trash
pub fn list_trash_items(trash_dir: &Path) -> Result<Vec<TrashedItem>> {
    if !trash_dir.exists() {
        return Ok(vec![]);
    }

    let dir_entries = fs::read_dir(trash_dir)
        .context("Failed to read trash directory")?
        .collect::<Result<Vec<_>, _>>()?;

    let items = dir_entries
        .into_iter()
        .filter_map(|item| {
            match item.file_name().into_string() {
                Err(_) => error!(
                    "WARN: Trash item '{}' does not have a valid UTF-8 filename!",
                    item.path().display()
                ),

                Ok(filename) => {
                    if filename == TRASH_TRANSFER_DIRNAME {
                        return None;
                    }

                    match TrashItemInfos::decode(&filename) {
                        Err(err) => {
                            error!(
                                "WARN: Trash item '{}' does not have a valid trash filename!",
                                item.path().display()
                            );

                            debug!("Invalid trash item filename: {:?}", err);
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
pub fn list_all_trash_items(exclude_dirs: &[PathBuf]) -> Result<Vec<TrashedItem>> {
    let all_trash_items = list_trash_dirs(exclude_dirs)?
        .into_iter()
        .map(|trash_dir| list_trash_items(&trash_dir))
        .collect::<Result<Vec<_>, _>>()?;

    let mut items = all_trash_items.into_iter().flatten().collect::<Vec<_>>();
    items.sort_by_key(|item| item.data.datetime);

    Ok(items)
}

/// Find a specific item in the trash (panic if not found)
pub fn expect_trash_item(
    filename: &str,
    id: Option<&str>,
    exclude_dirs: &[PathBuf],
) -> Result<FoundTrashItems> {
    let mut candidates = list_all_trash_items(exclude_dirs)?
        .into_iter()
        .filter(|trashed| trashed.data.filename == filename)
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        bail!("Specified item was not found in the trash.");
    } else if candidates.len() > 1 {
        match id {
            None => Ok(FoundTrashItems::Multi(candidates)),
            Some(id) => Ok(FoundTrashItems::Single(
                candidates
                    .into_iter()
                    .find(|c| c.data.compute_id() == id)
                    .context("There is no trash item with the provided ID")?,
            )),
        }
    } else {
        Ok(FoundTrashItems::Single(candidates.remove(0)))
    }
}

/// Find a specific item in the trash, fail if none is found or if multiple candidates are found
pub fn expect_single_trash_item(
    filename: &str,
    id: Option<&str>,
    exclude_dirs: &[PathBuf],
) -> Result<TrashedItem> {
    match expect_trash_item(filename, id, exclude_dirs)? {
        FoundTrashItems::Single(item) => Ok(item),
        FoundTrashItems::Multi(candidates) => bail!(
            "Multiple items with this filename were found in the trash:\n\n{}",
            table_for_items(&candidates)
        ),
    }
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
    pub data: TrashItemInfos,
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

pub fn table_for_items(items: &[TrashedItem]) -> Table {
    let mut table = Table::new();

    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            "Type",
            "Filename",
            "Size",
            "ID",
            "Deleted on",
            "Trash directory",
        ]);

    for item in items {
        let TrashedItem { data, trash_dir } = item;

        let TrashItemInfos { filename, datetime } = data;

        let mt = fs::metadata(item.complete_trash_item_path());

        table.add_row(vec![
            mt.as_ref()
                .map(|mt| {
                    if mt.file_type().is_file() {
                        "File"
                    } else if mt.file_type().is_dir() {
                        "Directory"
                    } else {
                        "<Unknown>"
                    }
                    .to_owned()
                })
                .unwrap_or_else(|err| format!("ERROR: {err}")),
            filename.clone(),
            mt.as_ref()
                .map(|mt| {
                    if mt.file_type().is_file() {
                        human_readable_size(mt.len())
                    } else {
                        String::new()
                    }
                })
                .unwrap_or_else(|_| "ERROR".to_owned()),
            data.compute_id(),
            Zoned::try_from(*datetime)
                .and_then(|date| jiff::fmt::rfc2822::to_string(&date))
                .unwrap_or_else(|_| "<Failed to format date>".to_owned()),
            trash_dir.to_string_lossy().into_owned(),
        ]);
    }

    table
}

pub fn are_on_same_fs(a: &Path, b: &Path) -> Result<bool> {
    fn get_dev(item: &Path) -> Result<u64> {
        let mt = fs::metadata(item)?;

        #[cfg(target_family = "windows")]
        {
            use std::os::windows::fs::MetadataExt;
            mt.volume_serial_number()
                .map(u64::from)
                .context("Item does not have a volume serial number attached")
        }

        #[cfg(target_family = "unix")]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(mt.dev())
        }
    }

    let a_fs_id = get_dev(a)
        .with_context(|| format!("Failed to get filesystem ID for item '{}'", a.display()))?;

    let b_fs_id = get_dev(b)
        .with_context(|| format!("Failed to get filesystem ID for item '{}'", b.display()))?;

    Ok(a_fs_id == b_fs_id)
}

pub fn list_deletable_fs_items(path: &Path) -> Result<Vec<PathBuf>> {
    WalkDir::new(path)
        .contents_first(true)
        .into_iter()
        .map(|entry| entry.map(|entry| entry.into_path()))
        .collect::<Result<Vec<PathBuf>, _>>()
        .context("Failed to read directory entry")
}
