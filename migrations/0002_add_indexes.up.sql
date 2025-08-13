-- Add indexes for better performance
CREATE INDEX IF NOT EXISTS idx_audiobooks_author ON audiobooks (author);

CREATE INDEX IF NOT EXISTS idx_audiobooks_title ON audiobooks (title);

CREATE INDEX IF NOT EXISTS idx_files_book_id ON files (book_id);

CREATE UNIQUE INDEX IF NOT EXISTS uq_audiobooks_author_title ON audiobooks (author, title);

CREATE UNIQUE INDEX IF NOT EXISTS uq_files_book_file_path ON files (book_id, file_id, file_path);

CREATE UNIQUE INDEX IF NOT EXISTS uq_progress_user_book_file ON progress (user_id, book_id, file_id);