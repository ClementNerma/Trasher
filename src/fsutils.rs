use super::command::OPTS;
use super::items::TrashItem;
use fs_extra::dir::TransitProcessResult;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use std::cell::RefCell;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::Result as IoResult;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Name of the transfer directory in the trash
pub const TRASH_TRANSFER_DIRNAME: &str = "#PARTIAL";

lazy_static! {
    /// Path to the transfer directory in the trash
    pub static ref TRASH_TRANSFER_DIR: PathBuf = OPTS.trash_dir.join(TRASH_TRANSFER_DIRNAME);
}

/// List and parse all items in the trash
pub fn list_trash_items(trash_path: impl AsRef<Path>) -> IoResult<Vec<TrashItem>> {
    Ok(fs::read_dir(trash_path)?
        .collect::<Result<Vec<_>, _>>()?
        .iter()
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
                        Ok(trash_item) => return Some(trash_item),
                    }
                }
            }

            None
        })
        .collect())
}

/// Find a specific item in the trash (panic if not found)
pub fn expect_trash_item(
    trash_dir: impl AsRef<Path>,
    filename: &str,
    id: Option<&str>,
) -> FoundTrashItems {
    let mut candidates: Vec<TrashItem> = list_trash_items(&trash_dir)
        .unwrap()
        .into_iter()
        .filter(|item| item.filename() == filename)
        .collect();

    if candidates.is_empty() {
        super::fail!("Specified item was not found in the trash.");
    } else if candidates.len() > 1 {
        match id {
            None => return FoundTrashItems::Multi(candidates),
            Some(id) => {
                return FoundTrashItems::Single(
                    candidates
                        .into_iter()
                        .find(|c| c.id() == id)
                        .unwrap_or_else(|| {
                            super::fail!("There is no trash item with the provided ID")
                        }),
                )
            }
        }
    }

    FoundTrashItems::Single(candidates.remove(0))
}

/// Find a specific item in the trash, fail if none is found or if multiple candidates are found
pub fn expect_single_trash_item(
    trash_dir: impl AsRef<Path>,
    filename: &str,
    id: Option<&str>,
) -> TrashItem {
    match expect_trash_item(trash_dir, filename, id) {
        FoundTrashItems::Single(item) => item,
        FoundTrashItems::Multi(candidates) => super::fail!(
            "Multiple items with this filename were found in the trash:{}",
            candidates
                .iter()
                .map(|c| format!("\n* {}", c))
                .collect::<String>()
        ),
    }
}

/// Get details on a filesystem item
pub fn get_fs_details(path: impl AsRef<Path>) -> IoResult<FSDetails> {
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

/// Get the trash path for an item that's going to be transferred to it
pub fn transfer_trash_item_path(item: &TrashItem) -> PathBuf {
    OPTS.trash_dir
        .join(TRASH_TRANSFER_DIRNAME)
        .join(item.trash_filename())
}

pub fn complete_trash_item_path(item: &TrashItem) -> PathBuf {
    OPTS.trash_dir.join(item.trash_filename())
}

/// Move a partial item to the trash's main directory once the transfer is complete
pub fn move_transferred_trash_item(item: &TrashItem) -> IoResult<()> {
    fs::rename(
        transfer_trash_item_path(item),
        complete_trash_item_path(item),
    )
}

/// Cleanup the transfer directory
pub fn cleanup_transfer_dir() -> IoResult<()> {
    if TRASH_TRANSFER_DIR.exists() {
        fs::remove_dir_all(TRASH_TRANSFER_DIR.as_path())
    } else {
        Ok(())
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

    return format!(
        "{:.2} {}",
        bytes as f64 / compare as f64,
        names.last().unwrap()
    );
}

lazy_static! {
    static ref PARSE_SIZE_STR: Regex =
        Regex::new("^(?i)(?P<intqty>\\d+)(?:\\.(?P<decqty>\\d+))?(?P<unit>[BKMGTPE])(?:i?B)?$")
            .unwrap();
}

/// Convert a human-readable size back to a number of bytes
pub fn parse_human_readable_size(size: &str) -> Result<u64, &'static str> {
    let captured = PARSE_SIZE_STR.captures(size).ok_or("Unknown size format")?;

    let int = captured["intqty"].parse::<u64>().unwrap();
    let dec = captured.name("decqty");

    let unit_char = captured["unit"]
        .chars()
        .next()
        .unwrap()
        .to_ascii_uppercase();
    let unit_size = 1024u64.pow("BKMGTPE".chars().position(|c| c == unit_char).unwrap() as u32);

    if dec.is_some() && unit_size == 1 {
        return Err("Cannot use decimal bytes");
    }

    let dec_size = match dec {
        None => 0,
        Some(dec) => {
            let dec = dec.as_str();

            let dec_num = dec.parse::<u64>().unwrap();
            let unit_divider = 10u64.pow(dec.len() as u32);

            if unit_divider.to_string().len() > unit_size.to_string().len() {
                return Err("Too many decimals for this unit, would give decimal bytes");
            }

            unit_size * dec_num / unit_divider
        }
    };

    Ok(int * unit_size + dec_size)
}

/// Move items around with a progressbar
pub fn move_item_pbr(path: &Path, target: &Path) -> Result<(), Box<dyn Error>> {
    let pbr = Rc::new(RefCell::new(None));

    let update_pbr = |copied, total, item_name: &str| {
        let mut pbr = pbr.borrow_mut();
        let pbr = pbr.get_or_insert_with(|| {
            let pbr = ProgressBar::new(total);
            pbr.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
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

/// Trash items found with the [`expect_trash_item`] function
pub enum FoundTrashItems {
    Single(TrashItem),
    Multi(Vec<TrashItem>),
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
