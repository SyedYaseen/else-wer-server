-- Folder-identity model: a book is identified by its folder (files_location) plus a
-- title partition within it. UNIQUE(author, title) was a relic of tag-identity and
-- wrongly merged e.g. an m4b edition and an MP3/ chapter folder of the same book,
-- so audiobooks is rebuilt with UNIQUE(files_location, title) instead.

-- Real-world series (saga) table; audiobooks.series remains the tag/album-derived display value
CREATE TABLE IF NOT EXISTS series (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    provider TEXT,
    provider_id TEXT
);

PRAGMA defer_foreign_keys = ON;

CREATE TABLE audiobooks_new (
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
    series_id INTEGER REFERENCES series(id),
    -- TEXT: sequences like "1.5" exist
    series_sequence TEXT,
    asin TEXT,
    narrated_by TEXT,
    -- Set when the user edits a book (org UI / match apply); rescans never overwrite locked rows
    user_locked INTEGER NOT NULL DEFAULT 0,
    UNIQUE (files_location, title)
);

INSERT INTO audiobooks_new (id, author, series, title, files_location, book_size, cover_art, metadata, duration, created_at, updated_at)
SELECT id, author, series, title, files_location, book_size, cover_art, metadata, duration, created_at, updated_at
FROM audiobooks;

DROP TABLE audiobooks;

ALTER TABLE audiobooks_new RENAME TO audiobooks;

-- Recreate what dropped with the old table (0001 trigger, 0002 indexes)
CREATE TRIGGER update_audiobooks_timestamp
AFTER UPDATE ON audiobooks
FOR EACH ROW
BEGIN
    UPDATE audiobooks SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

CREATE INDEX IF NOT EXISTS idx_audiobooks_author ON audiobooks (author);

CREATE INDEX IF NOT EXISTS idx_audiobooks_title ON audiobooks (title);
