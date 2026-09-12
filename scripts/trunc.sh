#!/bin/bash

# Truncate all tables in SQLite database
# Usage: scripts/trunc.sh [database_path]   (or `make db-truncate DB=...`)

DB_PATH="${1:-./else-wer.db}"
if [ ! -f "$DB_PATH" ]; then
    echo "Error: Database file not found: $DB_PATH"
    exit 1
fi

echo "Truncating all tables in: $DB_PATH"

sqlite3 "$DB_PATH" << EOF
PRAGMA foreign_keys = OFF;
DELETE FROM sqlite_sequence;

$(sqlite3 "$DB_PATH" "SELECT 'DELETE FROM \"' || name || '\";' FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")

PRAGMA foreign_keys = ON;
EOF

echo "✓ All tables truncated successfully"
