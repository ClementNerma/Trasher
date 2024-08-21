use std::{
    str,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

static NAME_ID_SEPARATOR: &str = " ^";

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
        URL_SAFE_NO_PAD.encode(
            self.datetime
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .to_le_bytes(),
        )
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

        let id = u64::from_le_bytes(
            id.try_into()
                .map_err(|_| TrashItemDecodingError::InvalidIdLength)?,
        );

        let datetime = UNIX_EPOCH + Duration::from_secs(id);

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
