use sqlx::{Pool, Sqlite};

use crate::{api::api_error::ApiError, db::meta_scan::apply_changes, models::meta_scan::ChangeDto};

pub async fn organize_books(db: &Pool<Sqlite>, changes: Vec<ChangeDto>) -> Result<(), ApiError> {
    println!("{:#?}", changes);
    apply_changes(db, changes).await?;

    Ok(())
}
