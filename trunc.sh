#!/bin/bash

# Truncate all tables in SQLite database
# Usage: ./truncate_db.sh <database_path>
# Example: ./truncate_db.sh ./sqlite.db

# DB_PATH="${1:-.sqlite.db}"

DB_PATH="/home/loop/p/else-wer/else-wer-server/rustybookshelf.db"
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
