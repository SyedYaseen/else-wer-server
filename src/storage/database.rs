use rusqlite::{params, Connection, Result};
use crate::models::AudioBook;

pub struct Db {
    conn: Connection
}

impl Db {
    pub fn new() -> Result<Db> {
        Ok(Self {
            conn: Connection::open("rustybookshelf.db").unwrap(),
        })
    }

    pub fn init_db(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS audiobooks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                author TEXT NOT NULL,
                series TEXT,
                title TEXT NOT NULL,
                files_location TEXT NOT NULL,
                cover_art TEXT,
                metadata TEXT
            );

            CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY,
                book_id INTEGER,
                file_path TEXT NOT NULL,
                codec TEXT,
                duration INTEGER,
                channels INTEGER,
                sample_rate INTEGER
            );

            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                salt TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS progress (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                book_id INTEGER NOT NULL,
                progress_fname TEXT,
                progress_time_marker INTEGER,
                FOREIGN KEY(user_id) REFERENCES users(id),
                FOREIGN KEY(book_id) REFERENCES audiobooks(id)
            );
            "
        )?;
        Ok(())
    }

    pub fn insert_audiobook(&self, book: &AudioBook) -> Result<()> {
        self.conn.execute(
            "INSERT INTO audiobooks (author, series, title, files_location, cover_art, metadata)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (&book.author, &book.series, &book.title, &book.content_path, &book.cover_art, &book.metadata),
        )?;
        Ok(())
    }

//     pub fn insert_file_metadata(
//     conn: &Connection,
//     book_id: i32,
//     file_path: &str,
//     codec: Option<String>,
//     duration: Option<u64>,
//     channels: Option<u8>,
//     sample_rate: Option<u32>,
// ) -> Result<()> {
//     conn.execute(
//         "INSERT INTO files (book_id, file_path, codec, duration, channels, sample_rate)
//          VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
//         params![book_id, file_path, codec, duration, channels, sample_rate],
//     )?;
//     Ok(())
// }

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

}