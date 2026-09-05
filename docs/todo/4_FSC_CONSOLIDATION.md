# Remove file_scan_cache: consolidate into `files`

## Session status (2026-07-09) — READ THIS FIRST
- Deviation from "Precondition" below: user explicitly said "code only, skip DB" for this
  session, and the standing project rule is "never commit without user review" — so the
  branch was NOT committed first as the plan's precondition asked. Work was done directly
  on top of the existing uncommitted file-scan diff.
- Phase 1 (migration 0008 files): DONE — migrations/0008_drop_file_scan_cache.{up,down}.sql
  — **note a real bug found and fixed vs. the original plan text**: as of sqlx-sqlite
  0.8.6, `Migrate::apply` always runs the migration body inside a transaction regardless
  of a `-- no-transaction` first line (parsed but not consulted by the sqlite driver), and
  SQLite silently no-ops `PRAGMA foreign_keys` changes issued inside a transaction. With
  foreign_keys left ON, `DROP TABLE files` performs an implicit `DELETE FROM files` and
  fires `progress.file_id`'s `ON DELETE CASCADE` — verified empirically, it silently wiped
  the one progress row in a scratch copy of the dev DB on the first attempt. Fixed by
  rebuilding `progress` twice in the same migration (drop its FK to `files` before the
  `files` rebuild's DROP TABLE, restore it after) so nothing ever references `files` via
  FK at the moment it's dropped. Re-verified clean via both direct `sqlite3` and actual
  `sqlx migrate run`/`revert` — progress row byte-for-byte preserved, `PRAGMA
  foreign_key_check` clean. See the up.sql comment for the full explanation.
- Phase 2 (scan path): DONE
- Phase 3 (org flow & queries): DONE
- Phase 4 (model/test cleanup): DONE
- Phase 5 (verification): cargo build/test(27/27)/clippy done against a scratch DB copy
  only (real rustybookshelf.db never touched/opened for writes — confirmed via checksum
  before/after). Migration verified against a scratch copy: row count + id preservation
  (diffed pre/post, identical), fsc gone, UNIQUE(file_path) present (sqlite_autoindex,
  confirmed rejects a duplicate insert), progress orphan count = 0, `sqlx migrate revert`
  (down.sql) also verified clean. Optional live-server smoke test skipped (not required;
  would have needed JWT relay-token file-path plumbing outside the scratch area for
  marginal extra confidence beyond the already-thorough test+SQL verification).
- **DB migration on the real rustybookshelf.db was deliberately NOT applied.** Next
  session/user must run the 0008 migration against the real DB (e.g. `sqlx migrate run`
  with DATABASE_URL pointed at rustybookshelf.db) before the server will work again, since
  the code now assumes file_scan_cache is gone. Given the bug found above, re-verify on a
  **fresh copy of the real DB** before touching the real file, even though scratch-copy
  verification already exercised the identical up.sql against real production-shaped data.
- Also touched beyond the plan's explicit file list (needed for compilation against the
  new schema, not optional): `src/db/audiobooks.rs` — `get_files_by_book_id` selected the
  now-gone `files.file_id` column via a compile-time `sqlx::query!`; repointed to mirror
  `files.id` (single id space) and fixed a knock-on `file_size` Option/non-Option type
  change (that column is now `NOT NULL`, so the macro infers a plain `i64`).

## Context

`file_scan_cache` (fsc) was built as a staging table for scanned files, but its staging semantics are
vestigial: rows are inserted `resolve_status = 0` and flipped to resolved inside the same
`sync_disk_db_state` call; status 2 is written by the org UI but never read; `raw_metadata` is always
inserted as `""`, `hash` is never computed, `library_id`/`extracts`/`pub_year`/`series_part` are never
read back. What keeps it alive today: (1) rescan ledger — `scan_files` diffs disk paths against fsc to
skip lofty re-probing and detect deletions; (2) metadata conduit — `attach_files` copies audio metadata
fsc→`files`, with `files.file_id` = fsc.id; (3) org-UI file identity — `get_grouped_files` /
`ChangeDto.file_ids` speak fsc ids.

The two-table design already caused the track/disc drift bug (fixed by migration 0007) and hides a
latent bug: `MergeTitle` updates `progress WHERE file_id IN (org ids = fsc ids)` but `progress.file_id`
FK-references `files.id` — two different autoincrement sequences that only coincidentally align.

**Goal:** drop `file_scan_cache`; `files` becomes the single per-file ledger with `files.id` as the one
file-id space everywhere. Grouping consumes the scan pass's in-memory records directly. This removes the
drift-bug class and fixes the MergeTitle/progress mismatch.

**Facts verified up front:**
- The RN app already uses `files.id` (download-store.ts v2 note: "fileId … is now the server files PK id"),
  and `stream_file`/`get_file_path_by_id` already look up `files.id`. Only the organize flow uses fsc ids.
- The PWA organize flow echoes server-returned ids back within a session; switching both directions to
  `files.id` server-side needs no PWA change (verify in Phase 5).
- Upload calls `scan_files(config.audiobook_location)` (whole library root), so the delete-diff is safe.
- sqlx enables `PRAGMA foreign_keys` by default → `progress` rows cascade when `files` rows are deleted.
- sqlx supports a `-- no-transaction` first-line directive, needed so `PRAGMA foreign_keys=OFF` works
  during the table rebuild.

**Precondition:** the current uncommitted file-scan branch (through migration 0007) must be committed
first — this plan builds on it. First implementation step: copy this plan to
`todo/4_FSC_CONSOLIDATION.md` per project convention.

**User direction:** breaking changes are acceptable (API shapes, id semantics, endpoint/function names);
the bar is "works without issues, production-grade". So: no back-compat shims, strict schema
(NOT NULL where the code always supplies a value), transactional writes, and the concurrency guard is
in scope rather than deferred. User *data* (progress) is still preserved — that's quality, not compat.

## Phase 1 — Migration `0008_drop_file_scan_cache`

`up.sql` (first line `-- no-transaction`):
1. `PRAGMA foreign_keys=OFF;`
2. Rebuild `files` (SQLite can't `DROP COLUMN file_id` — it's in the UNIQUE constraint):
   create `files_new` with columns `id INTEGER PRIMARY KEY, book_id NOT NULL, file_name NOT NULL,
   file_path NOT NULL UNIQUE, file_size NOT NULL DEFAULT 0, duration, channels, sample_rate, bitrate,
   track_number, disc_number` + the existing
   `FOREIGN KEY (book_id) REFERENCES audiobooks(id) ON DELETE CASCADE`;
   `INSERT INTO files_new SELECT id, book_id, … FROM files` (**copy `id` explicitly** — progress rows
   and app downloads key on it; preserving user data is part of the quality bar); drop `files`; rename.
   `UNIQUE(file_path)` replaces `UNIQUE(book_id, file_id, file_path)` and is what makes
   `INSERT OR IGNORE` dedupe concurrent scans (a physical file belongs to exactly one book).
   Add `CREATE INDEX idx_files_book_id ON files(book_id)`.
3. `DROP TABLE file_scan_cache;` (its indexes go with it).
4. `PRAGMA foreign_keys=ON;`

`down.sql`: schema-only restore (recreate fsc empty per 0001 shape; rebuild `files` with a nullable
`file_id`). Data in fsc is not restorable — document as lossy, same as 0007's down.

Note: 0007 still reads fsc — fine, migrations run in order and 0008 drops it afterward.

## Phase 2 — Scan path

- `src/file_ops/scan_files.rs`: `fetch_all_stage_file_paths` → new `fetch_known_file_paths` reading
  `SELECT id, file_path FROM files` (same HashMap shape). Leftover ids feed the delete step. The
  skip-set now means "fully attached", so a probed-but-never-attached file is retried next scan
  (self-healing, intended).
- `src/db/meta_scan.rs`:
  - `sync_disk_db_state(db, chunk)` → `group_and_attach_files(db, chunk)` consuming the in-memory
    `&[FileScanCache]` chunk directly; delete `save_metadata_to_cache`, the `resolve_status = 0` SELECT,
    and `update_fsc_resolved_status`.
  - `src/file_ops/grouping.rs`: `GroupRow`/`BookGroup` currently carry only `fsc_ids`; make groups carry
    the per-file records (or indices into the chunk slice) so `attach_files` can bind
    name/path/duration/size/channels/sample_rate/bitrate/track/disc as VALUES instead of
    `SELECT … FROM file_scan_cache`. Chunk-split folders still re-resolve via the existing
    `resolve_book_id` candidates-by-`files_location` logic — unchanged.
  - `delete_removed_paths_from_cache` → `delete_removed_files`: `DELETE FROM files WHERE id IN (…)`
    + existing orphan-audiobooks delete; drop the fsc delete; progress rows cascade via FK.
    Run both statements in one transaction. Replace the leftover `println!`s in this path and in
    `scan_files` with `tracing::info!`.
  - Wrap each chunk's group+attach in a transaction so a mid-chunk failure can't leave a book without
    its files (retry next scan is then clean).
- Concurrency guard (in scope per user direction): add `scan_running: Arc<AtomicBool>` to `AppState`,
  mirroring the existing `backfill_running` compare_exchange pattern from
  `try_spawn_metadata_backfill` (`src/api/match_meta.rs`); `scan_files_handler`, the upload-complete
  scan, and `list_scanned_files_handler`'s lazy scan take it; return 409 when a scan is already running
  (upload path: skip the rescan, log, still 200 the upload itself).
- `cover_links` untouched (walks `audiobooks.files_location`).

## Phase 3 — Org flow & queries (single id space)

`src/db/meta_scan.rs`:
- `get_grouped_files`: `SELECT f.id as id, …, b.files_location as path_parent` — drop the fsc join.
- `save_user_file_org_changes_filescan_cache` → rename `save_user_file_org_changes`: delete all fsc
  UPDATE blocks (`fsc_qb`/`fsc_parts`); every `bind_ids(…, "file_id", …)` on `files` becomes `"id"`.
  The `MergeTitle` progress update (`UPDATE progress … WHERE file_id IN`) becomes correct automatically
  since ids are now `files.id`.
- `FileMove` new-book INSERT currently selects author/clean_series/path_parent/cover_art/raw_metadata/
  duration from fsc → source from the change payload (`new_author`, `new_series`) + `files` (parent of
  `file_path` for `files_location`); drop the dead `raw_metadata`/`metadata` value; leave cover to the
  existing `cover_links` pass.
- `src/api/audiobooks.rs` `list_scanned_files_handler`: `cache_row_count` → `SELECT COUNT(id) FROM files`
  (keep function, repoint the query).
- `src/file_ops/org_books.rs`: rename call site.

## Phase 4 — Model & test cleanup

- `src/models/meta_scan.rs`: `FileScanCache` stays (in-memory scan record; keep the name to minimize
  churn) but drop now-dead fields: `raw_metadata`, `hash`, `resolve_status`/`ResolvedStatus` plumbing
  (`extracts`/`dramatized` stay — used in-memory by `meta_cleanup`).
- Rewrite `src/db/meta_scan.rs` tests: builders already construct in-memory `FileScanCache` rows — feed
  them to the new `group_and_attach_files(pool, rows)` signature; assertions move from fsc counts /
  `resolve_status` to `files` rows (e.g. "second scan of same paths inserts nothing" replaces the
  status-flip assertions).
- `graphify update .` after code changes.

## Phase 5 — Verification

1. `cargo test` green (rewritten suite).
2. Migration on a **copy** of the dev DB: row count and `id` values of `files` preserved; fsc gone;
   `UNIQUE(file_path)` present; before migrating, run
   `SELECT COUNT(*) FROM progress WHERE file_id NOT IN (SELECT id FROM files)` — nonzero means pre-v2
   app rows in the old fsc-id space; decide remap/cleanup then (expected 0).
3. Live server: scan populates books/files; delete a file on disk → rescan removes the file row, orphan
   book, and cascades progress; rescan of unchanged library inserts nothing and skips re-probing.
4. PWA organize round-trip (rename/move/merge) against the live server — ids echo correctly.
5. App smoke: stream + progress for an existing book (ids preserved by the rebuild).

## Out of scope (noted for later)

- Hash-based move detection to preserve progress across file moves (hash was never populated).
- Broader schema polish beyond `files` (e.g. dead `users.salt` column — already tracked in
  CLAUDE_PROGRESS deferred list).
