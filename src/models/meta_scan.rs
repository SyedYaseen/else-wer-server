use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use sqlx::sqlite::{SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i64)]
pub enum ResolvedStatus {
    UnResolved = 0,
    AutoResolved = 1,
    UserResolved = 2,
    Ignored = 4,
}

impl ResolvedStatus {
    pub const fn value(&self) -> i64 {
        match self {
            ResolvedStatus::UnResolved => 0,
            ResolvedStatus::AutoResolved => 1,
            ResolvedStatus::UserResolved => 2,
            ResolvedStatus::Ignored => 3,
        }
    }

    pub const fn from_value(value: i64) -> Option<Self> {
        match value {
            0 => Some(ResolvedStatus::UnResolved),
            1 => Some(ResolvedStatus::AutoResolved),
            2 => Some(ResolvedStatus::UserResolved),
            3 => Some(ResolvedStatus::Ignored),
            _ => None,
        }
    }
}

impl Type<Sqlite> for ResolvedStatus {
    fn type_info() -> SqliteTypeInfo {
        <i64 as Type<Sqlite>>::type_info()
    }
}

impl<'r> Decode<'r, Sqlite> for ResolvedStatus {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let int_val = <i64 as Decode<Sqlite>>::decode(value)?;
        match int_val {
            0 => Ok(ResolvedStatus::UnResolved),
            1 => Ok(ResolvedStatus::AutoResolved),
            2 => Ok(ResolvedStatus::UserResolved),
            4 => Ok(ResolvedStatus::Ignored),
            other => Err(format!("Invalid ResolvedStatus value: {}", other).into()),
        }
    }
}

// impl<'q> Encode<'q, Sqlite> for ResolvedStatus {
//     fn encode_by_ref(
//         &self,
//         buf: &mut <Sqlite as sqlx::database::HasArguments<'q>>::ArgumentBuffer,
//     ) -> sqlx::encode::IsNull {
//         (*(*self as i64)).encode_by_ref(buf)
//     }
// }

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FileScanCache {
    // pub library_id: i64,
    pub author: Option<String>,
    pub title: Option<String>,
    pub clean_title: Option<String>,
    pub file_path: String,
    pub path_parent: String,
    pub file_name: String,
    pub series: Option<String>,
    pub dramatized: bool,
    pub clean_series: Option<String>,
    pub series_part: Option<i64>,
    pub cover_art: Option<String>,
    pub pub_year: Option<i64>,
    pub narrated_by: Option<String>,
    pub duration: i64,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub file_size: i64,
    pub mime_type: Option<String>,
    pub channels: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bitrate: Option<i64>,
    pub extracts: Option<String>,
    pub raw_metadata: Option<String>,
    pub hash: Option<String>,
    pub resolve_status: ResolvedStatus,
}

impl FileScanCache {
    pub fn new(file_path: String, file_name: String, path_parent: String) -> FileScanCache {
        FileScanCache {
            file_path: file_path,
            file_name: file_name,
            path_parent: path_parent,
            dramatized: false,
            duration: 0,
            file_size: 0,
            author: None,
            title: None,
            clean_title: None,
            series: None,
            clean_series: None,
            series_part: None,
            cover_art: None,
            pub_year: None,
            narrated_by: None,
            track_number: None,
            disc_number: None,
            mime_type: None,
            channels: None,
            sample_rate: None,
            bitrate: None,
            extracts: None,
            raw_metadata: None,
            hash: None,
            resolve_status: ResolvedStatus::UnResolved,
        }
    }
}

#[derive(Serialize, Debug)]
pub struct FileInfo {
    pub id: i64,
    pub title: String,
    pub series: String,
    pub file_path: String,
    pub path_parent: String,
    pub file_name: String,
}

#[derive(Serialize)]
pub struct BookInfo {
    pub series: String,
    pub files: Vec<FileInfo>,
}

#[derive(Serialize)]
pub struct AuthorInfo {
    pub books: Vec<BookInfo>,
}

#[derive(Serialize)]
pub struct FileScanGrouped {
    pub series: String,
    pub authors: Vec<AuthorInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeType {
    Rename,
    MoveTitle,
    MergeTitle,
    FileMove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeDto {
    pub change_type: ChangeType,

    pub file_ids: Vec<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_author: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_series: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_filetitle: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_author: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_series: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_filetitle: Option<String>,
}
