// fn get_all_books(conn: &Connection) -> Result<Vec<AudioBook>> {
//     let mut stmt = conn.prepare("SELECT id, author, series, title, files_location, cover_art, metadata FROM audiobooks")?;
//     let rows = stmt.query_map([], |row| {
//         Ok(AudioBook {
//             id: row.get(0)?,
//             author: row.get(1)?,
//             series: row.get(2)?,
//             title: row.get(3)?,
//             files_location: row.get(4)?,
//             cover_art: row.get(5)?,
//             metadata: row.get(6)?,
//         })
//     })?;

//     rows.collect()
// }

// fn update_progress(conn: &Connection, user_id: i32, book_id: i32, fname: &str, time_marker: i32) -> Result<()> {
//     conn.execute(
//         "INSERT INTO progress (user_id, book_id, progress_fname, progress_time_marker)
//         VALUES (?1, ?2, ?3, ?4)
//         ON CONFLICT(user_id, book_id) DO UPDATE SET
//         progress_fname = excluded.progress_fname,
//         progress_time_marker = excluded.progress_time_marker",
//         (user_id, book_id, fname, time_marker),
//     )?;
//     Ok(())
// }
