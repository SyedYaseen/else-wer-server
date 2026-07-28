DROP INDEX IF EXISTS idx_libraries_single_default;
ALTER TABLE libraries DROP COLUMN is_default;
