use std::{
    str,
    sync::LazyLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

static NAME_ID_SEPARATOR: &str = " ^";

static DATE_REFERENTIAL: LazyLock<SystemTime> =
    LazyLock::new(|| 
        // 2024 January 1st. 00:00:00 UTC
        UNIX_EPOCH + Duration::from_secs(1704067200)
    );

#[derive(Debug, Clone)]
pub struct TrashItemInfos {
    pub filename: String,
    pub datetime: SystemTime,
}

impl TrashItemInfos {
    pub fn new(filename: String, datetime: SystemTime) -> Self {
        Self { filename, datetime }
    }

    pub fn new_now(filename: String) -> Self {
        Self::new(filename, SystemTime::now())
    }

    pub fn compute_id(&self) -> String {
        let id_bytes = self.datetime.duration_since(*DATE_REFERENTIAL).unwrap().as_nanos().to_be_bytes();
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

        let mut int_bytes = [0u8; 16];
        int_bytes[16 - id.len()..16].copy_from_slice(&id);

        let id = u128::from_be_bytes(int_bytes);

        let datetime = *DATE_REFERENTIAL
            + Duration::from_secs((id / 1_000_000_000) as u64)
            + Duration::from_nanos((id % 1_000_000_000) as u64);

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
