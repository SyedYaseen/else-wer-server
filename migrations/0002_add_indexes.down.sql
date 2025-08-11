-- Drop indexes
DROP INDEX IF EXISTS idx_audiobooks_author;

DROP INDEX IF EXISTS idx_audiobooks_title;

DROP INDEX IF EXISTS idx_files_book_id;

DROP INDEX IF EXISTS uq_progress_user_book_file;