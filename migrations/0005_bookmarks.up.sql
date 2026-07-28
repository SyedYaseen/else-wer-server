-- Saved timestamp + optional note within a book/file, independent of `progress`
-- (which tracks exactly one resume position per user/book/file). Bookmarks are
-- N-per-file, so unlike progress this needs a surrogate PK, not a composite one.
CREATE TABLE bookmarks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    book_id INTEGER NOT NULL,
    file_id INTEGER NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    note TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (book_id) REFERENCES audiobooks (id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES files (id) ON DELETE CASCADE
);

-- Drives "list bookmarks for user X in book Y".
CREATE INDEX idx_bookmarks_user_book ON bookmarks (user_id, book_id);
