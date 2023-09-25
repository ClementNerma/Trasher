use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::prelude::*;
use chrono::LocalResult;
use crc_any::CRC;
use once_cell::sync::Lazy;
use regex::Regex;
use std::fmt;
use std::str;
use std::{fs::FileType, path::PathBuf};

static DECODER: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        "^(?P<filename>.*)\\s\\[@\\s(?P<datetime>.*)\\]\\s\\{(?P<id>[\\da-zA-Z_\\-]+)\\}(\\.[^\\.]*)?$"
    ).unwrap()
});

static DATETIME_FORMAT: &str = "%Y.%m.%d_%Hh%Mm%Ss.%f%z";

#[derive(Debug, Clone)]
pub struct TrashItem {
    id: String,
    filename: String,
    datetime: DateTime<Local>,
    file_type: Option<FileType>,
}

impl TrashItem {
    pub fn new(filename: String, datetime: DateTime<Local>, file_type: Option<FileType>) -> Self {
        Self {
            id: Self::hash(datetime),
            filename,
            datetime,
            file_type,
        }
    }

    pub fn new_now(filename: String, file_type: Option<FileType>) -> Self {
        Self::new(filename, Local::now(), file_type)
    }

    pub fn hash(datetime: DateTime<Local>) -> String {
        let mut crc24 = CRC::crc24();
        crc24.digest(&datetime.timestamp().to_le_bytes());
        crc24.digest(&datetime.timestamp_subsec_nanos().to_le_bytes());
        URL_SAFE_NO_PAD.encode(crc24.get_crc_vec_le())
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }

    pub fn datetime(&self) -> &DateTime<Local> {
        &self.datetime
    }

    pub fn trash_filename(&self) -> String {
        let extension = match PathBuf::from(&self.filename).extension() {
            // There cannot be any loss here as the provided filename is a valid UTF-8 string
            Some(ext) => format!(".{}", ext.to_string_lossy()),
            None => "".to_string(),
        };

        format!(
            "{} [@ {}] {{{}}}{}",
            self.filename,
            self.datetime.format(DATETIME_FORMAT),
            self.id,
            extension
        )
    }

    pub fn decode(
        final_name: &str,
        file_type: Option<FileType>,
    ) -> Result<TrashItem, TrashItemDecodingError> {
        let captured = DECODER
            .captures(final_name)
            .ok_or(TrashItemDecodingError::InvalidFilenameFormat)?;

        let datetime = DateTime::parse_from_str(&captured["datetime"], DATETIME_FORMAT)
            .map_err(TrashItemDecodingError::InvalidDateTime)?;

        let timezoned = match Local.from_local_datetime(&datetime.naive_local()) {
            LocalResult::None => return Err(TrashItemDecodingError::TimezoneDecodingError),
            LocalResult::Single(datetime) => datetime,
            LocalResult::Ambiguous(_, _) => {
                return Err(TrashItemDecodingError::TimezoneDecodingError)
            }
        };

        let decoded = Self::new(captured["filename"].to_string(), timezoned, file_type);

        if decoded.id != captured["id"] {
            Err(TrashItemDecodingError::IDDoesNotMatch {
                found: captured["id"].to_string(),
                expected: decoded.id,
            })
        } else {
            Ok(decoded)
        }
    }
}

impl fmt::Display for TrashItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let file_type = self.file_type.map(|file_type| {
            format!(
                "[{}]",
                if file_type.is_dir() {
                    'D'
                } else if file_type.is_file() {
                    'f'
                } else if file_type.is_symlink() {
                    'S'
                } else {
                    '?'
                }
            )
        });

        write!(
            f,
            "| Removed on: {} | ID: {} | {} {}",
            self.datetime.to_rfc2822(),
            self.id,
            file_type.unwrap_or_else(|| "-".to_string()),
            self.filename
        )
    }
}

pub enum TrashItemDecodingError {
    InvalidFilenameFormat,
    InvalidDateTime(chrono::ParseError),
    TimezoneDecodingError,
    IDDoesNotMatch { found: String, expected: String },
}

impl fmt::Debug for TrashItemDecodingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::InvalidFilenameFormat => "File name format is invalid".to_string(),
                Self::InvalidDateTime(err) =>
                    format!("Invalid date/time in file name: {:?}", err.to_string()),
                Self::TimezoneDecodingError =>
                    "Date/time is invalid for the local timezone".to_string(),
                Self::IDDoesNotMatch { found, expected } =>
                    format!("Found ID '{}' but expected one was '{}'", found, expected),
            }
        )
    }
}
