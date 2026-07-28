-- Server-wide library roots (no per-user ownership; matches audiobooks/files/series
-- already being global). AUDIOBOOKS_LOCATION becomes a one-time seed for a "Default"
-- library row (see services/startup.rs::ensure_default_library) rather than the sole
-- root; libraries are otherwise fully DB-managed via the admin API.
CREATE TABLE libraries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    path TEXT NOT NULL UNIQUE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Nullable: existing rows are backfilled at startup once a "Default" library exists
-- (can't be done in pure SQL — the seed path comes from the AUDIOBOOKS_LOCATION env
-- var at startup time, not migration time). ON DELETE CASCADE matches delete_book's
-- existing semantics: deleting a library drops its catalog rows, never touches disk.
ALTER TABLE audiobooks ADD COLUMN library_id INTEGER REFERENCES libraries (id) ON DELETE CASCADE;

CREATE INDEX idx_audiobooks_library_id ON audiobooks (library_id);
