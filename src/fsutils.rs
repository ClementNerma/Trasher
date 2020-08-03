use std::fs;
use std::fmt;
use std::path::Path;
use std::io::Result as IoResult;
use super::command::OPTS;
use super::items::TrashItem;

/// List and parse all items in the trash
pub fn list_trash_items(trash_path: impl AsRef<Path>) -> IoResult<Vec<TrashItem>> {
    Ok(fs::read_dir(trash_path)?.collect::<Result<Vec<_>, _>>()?.iter().filter_map(|item| {
        match item.file_name().into_string() {
            Err(invalid_filename) => eprintln!("WARN: Trash item '{}' does not have a valid UTF-8 filename!", invalid_filename.to_string_lossy()),

            Ok(filename) => match TrashItem::decode(&filename) {
                Err(err) => {
                    eprintln!("WARN: Trash item '{}' does not have a valid trash filename!", filename);
                    super::debug!("Invalid trash item filename: {:?}", err);
                },
                Ok(trash_item) => return Some(trash_item)
            }
        }

        return None
    }).collect())
}

/// Find a specific item in the trash (panic if not found)
pub fn expect_trash_item(trash_dir: impl AsRef<Path>, filename: &str, id: Option<&str>) -> IoResult<FoundTrashItems> {
    let mut candidates: Vec<TrashItem> = list_trash_items(&trash_dir).unwrap()
        .into_iter()
        .filter(|item| item.filename() == filename)
        .collect();

    if candidates.len() == 0 {
        super::fail!("Specified item was not found in the trash.");
    }

    else if candidates.len() > 1 {
        match id {
            None => return Ok(FoundTrashItems::Multi(candidates)),
            Some(id) => return Ok(FoundTrashItems::Single(
                candidates
                    .into_iter()
                    .find(|c| c.id() == id)
                    .unwrap_or_else(|| super::fail!("There is no trash item with the provided ID"))
            ))
        }
    }

    return Ok(FoundTrashItems::Single(candidates.remove(0)));
}

/// Get details on a filesystem item
pub fn get_fs_details(path: impl AsRef<Path>) -> IoResult<FSDetails> {
    let metadata = fs::metadata(&path)?;

    let is_symlink = metadata.file_type().is_symlink();

    if metadata.is_file() {
        return Ok(FSDetails {
            is_symlink,
            is_directory: false,
            sub_directories: 0,
            sub_files: 0,
            size: metadata.len()
        });
    }

    let mut details = FSDetails {
        is_symlink,
        is_directory: true,
        sub_directories: 0,
        sub_files: 0,
        size: 0
    };

    for item in fs::read_dir(&path)? {
        let item_details = get_fs_details(item?.path())?;
        let dir_one = if item_details.is_directory { 1 } else { 0 };

        details.sub_directories += item_details.sub_directories + dir_one;
        details.sub_files += item_details.sub_files + (1 - dir_one);
        details.size += item_details.size;
    }

    return Ok(details);
}

/// Convert a size in bytes to a human-readable size
pub fn human_readable_size(bytes: u64) -> String {
    let names = [ "KiB", "MiB", "GiB", "TiB", "PiB", "EiB" ];

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

    return format!("{:.2} {}", bytes as f64 / compare as f64, names.last().unwrap());
}

/// Trash items found with the [`expect_trash_item`] function
pub enum FoundTrashItems {
    Single(TrashItem),
    Multi(Vec<TrashItem>)
}

/// Details on a filesystem item returned by the [`get_fs_details`] function
pub struct FSDetails {
    pub is_symlink: bool,
    pub is_directory: bool,
    pub sub_directories: u64,
    pub sub_files: u64,
    pub size: u64
}

impl fmt::Display for FSDetails {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            " | [{}] Size: {}{}",
            if self.is_symlink { "Symlink" } else if self.is_directory { "Directory" } else { "File" },
            human_readable_size(self.size),
            if self.is_directory {
                format!(
                    ", Items: {}, Directories: {}, Files: {}",
                    self.sub_directories + self.sub_files,
                    self.sub_directories,
                    self.sub_files
                )
            } else { "".to_string() }
        )
    }
}
