PRAGMA defer_foreign_keys = ON;

CREATE TABLE audiobooks_old (
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
    UNIQUE (author, title)
);

INSERT INTO audiobooks_old (id, author, series, title, files_location, book_size, cover_art, metadata, duration, created_at, updated_at)
SELECT id, author, series, title, files_location, book_size, cover_art, metadata, duration, created_at, updated_at
FROM audiobooks;

DROP TABLE audiobooks;

ALTER TABLE audiobooks_old RENAME TO audiobooks;

CREATE TRIGGER update_audiobooks_timestamp
AFTER UPDATE ON audiobooks
FOR EACH ROW
BEGIN
    UPDATE audiobooks SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

CREATE INDEX IF NOT EXISTS idx_audiobooks_author ON audiobooks (author);

CREATE INDEX IF NOT EXISTS idx_audiobooks_title ON audiobooks (title);

CREATE UNIQUE INDEX IF NOT EXISTS uq_audiobooks_author_title ON audiobooks (author, title);

DROP TABLE IF EXISTS series;
