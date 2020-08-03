use std::path::PathBuf;
use std::fmt;
use std::str;
use crc_any::CRC;
use base64::{encode_config, URL_SAFE_NO_PAD};
use chrono::prelude::*;
use chrono::LocalResult;
use regex::Regex;

lazy_static! {
    static ref DECODER: Regex = Regex::new(
        "^(?P<filename>.*)\\s\\[@\\s(?P<datetime>.*)\\]\\s\\{(?P<id>[\\da-zA-Z]+)\\}(\\.[^\\.]*)?$"
    ).unwrap();
}

static DATETIME_FORMAT: &'static str = "%Y.%m.%d_%Hh%Mm%Ss.%f%z";

pub struct TrashItem {
    id: String,
    filename: String,
    datetime: DateTime<Local>
}

impl TrashItem {
    pub fn new(filename: String, datetime: DateTime<Local>) -> Self {
        Self { id: Self::hash(datetime), filename, datetime }
    }

    pub fn new_now(filename: String) -> Self {
        Self::new(filename, Local::now())
    }

    pub fn hash(datetime: DateTime<Local>) -> String {
        let mut crc24 = CRC::crc24();
        crc24.digest(&datetime.timestamp().to_le_bytes());
        crc24.digest(&datetime.timestamp_subsec_nanos().to_le_bytes());
        encode_config(crc24.get_crc_vec_le(), URL_SAFE_NO_PAD)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }

    // pub fn datetime(&self) -> &DateTime<Local> {
    //     &self.datetime
    // }

    pub fn trash_filename(&self) -> String {
        let extension = match PathBuf::from(&self.filename).extension() {
            Some(ext) => ext.to_string_lossy().to_string(),
            None => "".to_string()
        };

        format!("{} [@ {}] {{{}}}{}", self.filename, self.datetime.format(DATETIME_FORMAT), self.id, extension)
    }

    pub fn decode(final_name: &str) -> Result<TrashItem, TrashItemDecodingError> {
        let captured = DECODER.captures(final_name)
            .ok_or(TrashItemDecodingError::InvalidFilenameFormat)?;
        
        /*let id = decode_config(&captured["id"], URL_SAFE_NO_PAD)
            .map_err(TrashItemDecodingError::InvalidIDFormat)?;

        if id.len() != 3 {
            return Err(TrashItemDecodingError::InvalidIDLength { found: id.len(), expected: 3 });
        }*/

        let datetime = DateTime::parse_from_str(&captured["datetime"], DATETIME_FORMAT)
            .map_err(TrashItemDecodingError::InvalidDateTime)?;
        
        let timezoned = match Local.from_local_datetime(&datetime.naive_local()) {
            LocalResult::None => return Err(TrashItemDecodingError::TimezoneDecodingError),
            LocalResult::Single(datetime) => datetime,
            LocalResult::Ambiguous(_, _) => return Err(TrashItemDecodingError::TimezoneDecodingError)
        };

        let decoded = Self::new(captured["filename"].to_string(), timezoned);

        if decoded.id != captured["id"] {
            return Err(TrashItemDecodingError::IDDoesNotMatch { found: captured["id"].to_string(), expected: decoded.id });
        } else {
            return Ok(decoded);
        }
    }
}

impl fmt::Display for TrashItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[ ID: {} ] {} (removed on: {})", self.id, self.filename, self.datetime.to_rfc2822())
    }
}

pub enum TrashItemDecodingError {
    InvalidFilenameFormat,
    InvalidDateTime(chrono::ParseError),
    TimezoneDecodingError,
    //InvalidIDFormat(base64::DecodeError),
    //InvalidIDLength { found: usize, expected: usize },
    IDDoesNotMatch { found: String, expected: String }
}

impl fmt::Debug for TrashItemDecodingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match self {
            Self::InvalidFilenameFormat => "File name format is invalid".to_string(),
            Self::InvalidDateTime(err) => format!("Invalid date/time in file name: {:?}", err.to_string()),
            Self::TimezoneDecodingError => format!("Date/time is invalid for the local timezone"),
            //Self::InvalidIDFormat(err) => format!("ID is not correctly base64-encoded: {:?}", err),
            //Self::InvalidIDLength { found, expected } => format!("Decoded ID is {} byte(s) long but is should be made of {} byte(s)", found, expected),
            Self::IDDoesNotMatch { found, expected } => format!("Found ID '{}' but expected one was '{}'", found, expected)
        })
    }
}
