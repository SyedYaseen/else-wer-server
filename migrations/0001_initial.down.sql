-- Drop tables in reverse order of creation
DROP TRIGGER IF EXISTS update_audiobooks_timestamp;
DROP TABLE IF EXISTS progress;
DROP TABLE IF EXISTS users;
DROP TABLE IF EXISTS files;
DROP TABLE IF EXISTS audiobooks;