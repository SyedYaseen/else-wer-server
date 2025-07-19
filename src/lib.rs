mod storage;
mod models;
use storage::fs;
use storage::database;
use std::error::Error;

pub fn scan_for_audiobooks() {
    fs::scan_for_audiobooks();
}

pub fn init_db() -> Result<(), Box<dyn Error>> {
    let db = database::Db::new()
        .map_err(|e| {
            eprintln!("Failed to create new database");
            e
        })?;
    
    db.init_db()
        .map_err(|e| {
            eprintln!("Failed to execute SQL batch");
            e
        })?;
    Ok(())

}