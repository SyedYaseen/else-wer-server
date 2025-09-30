use sqlx::{Pool, Sqlite};

use crate::{
    api::api_error::ApiError,
    db::meta_scan::{apply_dbchanges, propagate_changes},
    models::meta_scan::ChangeDto,
};

// Saves books organized on webui
pub async fn save_organized_books(
    db: &Pool<Sqlite>,
    changes: Vec<ChangeDto>,
) -> Result<(), ApiError> {
    apply_dbchanges(db, changes.clone()).await?;
    propagate_changes(db).await?; // TODO: verify if this affects files currently in progress
    Ok(())
}
