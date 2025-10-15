use sqlx::{Pool, Sqlite};

use crate::{
    api::api_error::ApiError,
    db::meta_scan::{add_modify_audiobook_files, save_user_file_org_changes_filescan_cache},
    models::meta_scan::ChangeDto,
};

// Saves books organized on webui
pub async fn save_organized_books(
    db: &Pool<Sqlite>,
    changes: Vec<ChangeDto>,
) -> Result<(), ApiError> {
    save_user_file_org_changes_filescan_cache(db, changes.clone()).await?;
    add_modify_audiobook_files(db).await?;
    Ok(())
}

// Save all books
pub async fn init_books_from_file_scan_cache(db: &Pool<Sqlite>) -> Result<(), ApiError> {
    add_modify_audiobook_files(db).await?;
    Ok(())
}
