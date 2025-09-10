use sqlx::{Pool, Sqlite};

use crate::{
    api::api_error::ApiError,
    db::meta_scan::apply_dbchanges,
    models::meta_scan::{ChangeDto, FileScanCache},
};

pub async fn organize_books(db: &Pool<Sqlite>, changes: Vec<ChangeDto>) -> Result<(), ApiError> {
    apply_dbchanges(db, changes.clone()).await?;
    // handle_metadata_changes(db, changes).await?;

    Ok(())
}

// pub async fn handle_metadata_changes(
//     db: &Pool<Sqlite>,
//     changes: Vec<ChangeDto>,
// ) -> Result<(), ApiError> {
//     let scan_caches = get_file_scan_cache(db, changes).await?;
//     for f in scan_caches {
//         update_metadata(f).await?;
//     }

//     // tracing::info!("{:#?}", scan_caches);
//     Ok(())
// }

// async fn update_metadata(f: FileScanCache) -> Result<(), ApiError> {
//     Ok(())
// }

// async fn get_file_scan_cache(
//     db: &Pool<Sqlite>,
//     changes: Vec<ChangeDto>,
// ) -> Result<Vec<FileScanCache>, ApiError> {
//     let mut ids: Vec<i64> = vec![];

//     for c in &changes {
//         let mut file_ids = c.file_ids.clone();
//         println!("This work {:#?}", c.current_author);
//         ids.append(&mut file_ids)
//     }

//     let scan_caches = get_changes(db, &ids).await?;
//     Ok(scan_caches)
// }
