use std::{fmt, path::PathBuf, str};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{prelude::*, LocalResult};
use crc_any::CRC;
use once_cell::sync::Lazy;
use regex::Regex;

static DECODER: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        "^(?P<filename>.*)\\s\\[@\\s(?P<datetime>.*)\\]\\s\\{(?P<id>[\\da-zA-Z_\\-]+)\\}(\\.[^\\.]*)?$"
    ).unwrap()
});

static DATETIME_FORMAT: &str = "%Y.%m.%d_%Hh%Mm%Ss.%f%z";

#[derive(Debug, Clone)]
pub struct TrashItemInfos {
    pub id: String,
    pub filename: String,
    pub datetime: DateTime<Local>,
}

impl TrashItemInfos {
    pub fn new(filename: String, datetime: DateTime<Local>) -> Self {
        Self {
            id: Self::hash(datetime),
            filename,
            datetime,
        }
    }

    pub fn new_now(filename: String) -> Self {
        Self::new(filename, Local::now())
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

    pub fn decode(final_name: &str) -> Result<TrashItemInfos, TrashItemDecodingError> {
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

        let decoded = Self::new(captured["filename"].to_string(), timezoned);

        if decoded.id != captured["id"] {
            Err(TrashItemDecodingError::IdDoesNotMatch {
                found: captured["id"].to_string(),
                expected: decoded.id,
            })
        } else {
            Ok(decoded)
        }
    }
}

pub enum TrashItemDecodingError {
    InvalidFilenameFormat,
    InvalidDateTime(chrono::ParseError),
    TimezoneDecodingError,
    IdDoesNotMatch { found: String, expected: String },
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
                Self::IdDoesNotMatch { found, expected } =>
                    format!("Found ID '{}' but expected one was '{}'", found, expected),
            }
        )
    }
}
