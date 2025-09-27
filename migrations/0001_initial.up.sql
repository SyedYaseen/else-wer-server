-- Create audiobooks table
CREATE TABLE IF NOT EXISTS audiobooks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    author TEXT NOT NULL,
    series TEXT,
    title TEXT NOT NULL,
    files_location TEXT NOT NULL,
    cover_art TEXT,
    metadata TEXT,
    duration INTEGER DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (author, title)
);

-- Create files table
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY,
    book_id INTEGER NOT NULL,
    file_id INTEGER NOT NULL,
    file_name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    duration INTEGER,
    channels INTEGER,
    sample_rate INTEGER,
    bitrate INTEGER,
    FOREIGN KEY (book_id) REFERENCES audiobooks (id) ON DELETE CASCADE,
    UNIQUE (book_id, file_id, file_path)
);

-- Create users table
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    is_admin BOOLEAN NOT NULL DEFAULT false,
    salt TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create progress table
CREATE TABLE IF NOT EXISTS progress (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    book_id INTEGER NOT NULL,
    file_id INTEGER NOT NULL,
    progress_ms INTEGER NOT NULL DEFAULT 0,
    complete BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (book_id) REFERENCES audiobooks (id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES files (id) ON DELETE CASCADE,
    UNIQUE (user_id, book_id, file_id)
);

CREATE TABLE IF NOT EXISTS file_scan_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER DEFAULT 1, -- multiple libraries later
    author TEXT, -- allow NULL in case we can't parse
    title TEXT,
    clean_title TEXT,
    file_path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    path_parent TEXT NOT NULL,
    series TEXT,
    clean_series TEXT,
    dramatized BOOLEAN NOT NULL DEFAULT FALSE,
    series_part INTEGER DEFAULT NULL, -- store series order if known
    cover_art TEXT, -- path or URL to extracted cover
    pub_year INTEGER,
    narrated_by TEXT,
    duration INTEGER DEFAULT 0, -- seconds
    track_number INTEGER DEFAULT NULL, -- if multi-part file
    disc_number INTEGER DEFAULT NULL, -- for multi-disc sets
    file_size INTEGER DEFAULT 0, -- for streaming/buffering
    mime_type TEXT, -- audio/mpeg, audio/m4b, etc.
    channels INTEGER,
    sample_rate INTEGER,
    bitrate INTEGER,
    extracts TEXT, -- extracts from series, author, title, filename as json
    raw_metadata TEXT NOT NULL, -- store full JSON dump
    resolve_status INTEGER,
    hash TEXT, -- optional: for duplicate detection
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (file_path)
);

CREATE INDEX idx_files_author_title ON file_scan_cache (author, title);

CREATE INDEX idx_files_series ON file_scan_cache (series);

CREATE INDEX idx_files_hash ON file_scan_cache (hash);

-- Create triggers for timestamps
CREATE TRIGGER update_audiobooks_timestamp
AFTER UPDATE ON audiobooks
FOR EACH ROW
BEGIN
    UPDATE audiobooks SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;