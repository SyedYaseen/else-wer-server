-- Add indexes for better performance
CREATE INDEX idx_audiobooks_author ON audiobooks(author);
CREATE INDEX idx_audiobooks_title ON audiobooks(title);
CREATE INDEX idx_files_book_id ON files(book_id);
CREATE INDEX idx_progress_user_book ON progress(user_id, book_id);