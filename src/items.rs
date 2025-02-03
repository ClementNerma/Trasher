use std::{str, sync::LazyLock};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jiff::{civil::{Date, DateTime, Time}, SignedDuration, Zoned};

static NAME_ID_SEPARATOR: &str = " ^";

static DATE_REFERENTIAL: LazyLock<DateTime> =
    LazyLock::new(|| 
        // 2024 January 1st. 00:00:00 UTC
        Date::new(2024, 1, 1)
            .unwrap()
            .to_datetime(Time::midnight())
    );

#[derive(Debug, Clone)]
pub struct TrashItemInfos {
    pub filename: String,
    pub deleted_at: DateTime,
}

impl TrashItemInfos {
    pub fn new(filename: String, deleted_at: DateTime) -> Self {
        Self { filename, deleted_at }
    }

    pub fn new_now(filename: String) -> Self {
        Self::new(filename, Zoned::now().datetime())
    }

    pub fn compute_id(&self) -> String {
        let id_bytes = self.deleted_at.duration_since(*DATE_REFERENTIAL).as_nanos().to_be_bytes();
        let id_bytes = &id_bytes[id_bytes.iter().position(|b| *b != 0).unwrap_or(0)..];

        URL_SAFE_NO_PAD.encode(id_bytes)
    }

    pub fn trash_filename(&self) -> String {
        format!("{}{NAME_ID_SEPARATOR}{}", self.filename, self.compute_id())
    }

    pub fn decode(trash_filename: &str) -> Result<TrashItemInfos, TrashItemDecodingError> {
        let circumflex_pos = trash_filename
            .rfind(NAME_ID_SEPARATOR)
            .ok_or(TrashItemDecodingError::InvalidFilenameFormat)?;

        let id = URL_SAFE_NO_PAD
            .decode(&trash_filename[circumflex_pos + NAME_ID_SEPARATOR.len()..])
            .map_err(|_| TrashItemDecodingError::BadlyEncodedId)?;

        if id.is_empty() || id.len() > 16 {
            return Err(TrashItemDecodingError::InvalidIdLength);
        }

        let mut int_bytes = [0u8; 16];
        int_bytes[16 - id.len()..16].copy_from_slice(&id);

        let id = i128::from_be_bytes(int_bytes);

        let datetime = *DATE_REFERENTIAL
            + SignedDuration::from_secs((id / 1_000_000_000) as i64)
            + SignedDuration::from_nanos((id % 1_000_000_000) as i64);

        Ok(Self::new(
            trash_filename[0..circumflex_pos].to_owned(),
            datetime,
        ))
    }
}

#[derive(Debug)]
pub enum TrashItemDecodingError {
    InvalidFilenameFormat,
    BadlyEncodedId,
    InvalidIdLength,
}
