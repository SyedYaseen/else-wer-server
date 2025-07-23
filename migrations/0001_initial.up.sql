-- Create audiobooks table
CREATE TABLE IF NOT EXISTS audiobooks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    author TEXT NOT NULL,
    series TEXT,
    title TEXT NOT NULL,
    files_location TEXT NOT NULL,
    cover_art TEXT,
    metadata TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create files table
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY,
    book_id INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    codec TEXT,
    duration INTEGER,
    channels INTEGER,
    sample_rate INTEGER,
    FOREIGN KEY(book_id) REFERENCES audiobooks(id) ON DELETE CASCADE
);

-- Create users table
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    salt TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create progress table
CREATE TABLE IF NOT EXISTS progress (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    book_id INTEGER NOT NULL,
    progress_fname TEXT,
    progress_time_marker INTEGER DEFAULT 0,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY(book_id) REFERENCES audiobooks(id) ON DELETE CASCADE,
    UNIQUE(user_id, book_id)
);

-- Create triggers for timestamps
CREATE TRIGGER update_audiobooks_timestamp
AFTER UPDATE ON audiobooks
FOR EACH ROW
BEGIN
    UPDATE audiobooks SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;