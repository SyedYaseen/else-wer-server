-- Per-user/book/day listened-time aggregate. Deliberately NOT a raw per-event
-- log (unbounded growth); the client sends small wall-clock deltas on its
-- existing periodic/pause/ended save cadence and the server folds each delta
-- into the day's running total.
CREATE TABLE listening_stats_daily (
    user_id INTEGER NOT NULL,
    book_id INTEGER NOT NULL,
    -- ISO date (YYYY-MM-DD), UTC day boundary -- a bucket key, not a point in time.
    day TEXT NOT NULL,
    ms_listened INTEGER NOT NULL DEFAULT 0,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (book_id) REFERENCES audiobooks (id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, book_id, day)
);

-- Drives "total listened per day across all books for user X, last N days".
CREATE INDEX idx_listening_stats_daily_user_day ON listening_stats_daily (user_id, day);

-- One row per user/book; times_finished increments only on a not-all-complete
-- -> all-complete transition (re-listens count, but re-saving progress on an
-- already-finished book doesn't).
CREATE TABLE finished_books (
    user_id INTEGER NOT NULL,
    book_id INTEGER NOT NULL,
    times_finished INTEGER NOT NULL DEFAULT 1,
    finished_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (book_id) REFERENCES audiobooks (id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, book_id)
);

-- Drives "list finished books for user X ordered by most-recently finished".
CREATE INDEX idx_finished_books_user_finished_at ON finished_books (user_id, finished_at DESC);
