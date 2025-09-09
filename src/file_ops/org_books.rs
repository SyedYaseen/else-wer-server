use sqlx::{Pool, Sqlite};

use crate::models::meta_scan::ChangeDto;

pub async fn organize_books(db: &Pool<Sqlite>, changes: Vec<ChangeDto>) {
    println!("{:#?}", changes)
}
