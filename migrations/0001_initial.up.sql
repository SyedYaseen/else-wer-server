-- Squashed initial schema (was 0001-0008; see git history for the incremental steps).
-- No ALTER TABLE anywhere: every table is created directly in its final shape.

CREATE TABLE series (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    provider TEXT,
    provider_id TEXT
);

CREATE UNIQUE INDEX idx_series_provider_id
    ON series (provider, provider_id) WHERE provider_id IS NOT NULL;

-- Folder-identity model: a book is identified by its folder (files_location) plus a
-- title partition within it.
CREATE TABLE audiobooks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    author TEXT NOT NULL,
    series TEXT,
    title TEXT NOT NULL,
    files_location TEXT NOT NULL,
    book_size INTEGER DEFAULT 0,
    cover_art TEXT,
    metadata TEXT,
    duration INTEGER DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    series_id INTEGER REFERENCES series (id),
    -- TEXT: sequences like "1.5" exist
    series_sequence TEXT,
    asin TEXT,
    narrated_by TEXT,
    description TEXT,
    -- Set when the user edits a book (org UI / match apply); rescans never overwrite locked rows
    user_locked INTEGER NOT NULL DEFAULT 0,
    -- Set when the user manually assigns/clears a book's series; the automatic
    -- metadata pass never touches series fields on locked rows
    series_locked INTEGER NOT NULL DEFAULT 0,
    UNIQUE (files_location, title)
);

CREATE INDEX idx_audiobooks_author ON audiobooks (author);

CREATE INDEX idx_audiobooks_title ON audiobooks (title);

CREATE TRIGGER update_audiobooks_timestamp
AFTER UPDATE ON audiobooks
FOR EACH ROW
BEGIN
    UPDATE audiobooks SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- `files` is the single per-file ledger; `id` is the single file-id space used
-- everywhere (org UI, progress, downloads, streaming).
CREATE TABLE files (
    id INTEGER PRIMARY KEY,
    book_id INTEGER NOT NULL,
    file_name TEXT NOT NULL,
    file_path TEXT NOT NULL UNIQUE,
    file_size INTEGER NOT NULL DEFAULT 0,
    duration INTEGER,
    channels INTEGER,
    sample_rate INTEGER,
    bitrate INTEGER,
    track_number INTEGER,
    disc_number INTEGER,
    FOREIGN KEY (book_id) REFERENCES audiobooks (id) ON DELETE CASCADE
);

CREATE INDEX idx_files_book_id ON files (book_id);

CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    is_admin BOOLEAN NOT NULL DEFAULT false,
    salt TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE progress (
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

CREATE UNIQUE INDEX uq_progress_user_book_file ON progress (user_id, book_id, file_id);
