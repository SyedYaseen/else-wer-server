-- Explicit default-library flag, replacing the fragile "literal name == 'Default'"
-- fallback in db::libraries::get_default_library. Partial unique index enforces at
-- most one default at the DB level (not just app-level discipline).
ALTER TABLE libraries ADD COLUMN is_default INTEGER NOT NULL DEFAULT 0;

CREATE UNIQUE INDEX idx_libraries_single_default ON libraries (is_default) WHERE is_default = 1;

-- Backfill: whichever library is currently named "Default" (or, failing that, the
-- earliest-created one) becomes the flagged default, matching the old fallback's
-- behavior for existing installs so nothing changes on upgrade.
UPDATE libraries SET is_default = 1 WHERE id = (
    SELECT id FROM libraries ORDER BY (name = 'Default') DESC, id ASC LIMIT 1
);
