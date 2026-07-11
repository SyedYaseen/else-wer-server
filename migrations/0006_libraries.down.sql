DROP INDEX IF EXISTS idx_audiobooks_library_id;
ALTER TABLE audiobooks DROP COLUMN library_id;
DROP TABLE IF EXISTS libraries;
