# Chapter support for single-file audiobooks

Status: planned, NOT STARTED (2026-07-11). Do not begin implementation until picked up explicitly.

## Context

Multi-file audiobooks already have a de-facto "chapter" concept: each row in the
`files` table (one audio file) is treated as a chapter, with `ChaptersSheet.tsx`
listing `FileMetadata[]` and clicking one calling `switchToFile(i)`. Single-file
audiobooks (one `.m4b`/`.mp3` containing the whole book) have **no chapter
breakdown at all** today, even when the file itself has chapter markers embedded
by the encoder.

Verified directly against the vendored crate sources that neither dependency
already in `Cargo.toml` can read this data:
- `lofty` 0.22.4 — zero chapter support (grepped source, only hit is an unrelated
  Musepack `// TODO: Support chapter packets?`).
- `symphonia` 0.5.4 — `symphonia-format-isomp4`'s demuxer always returns
  `cues: Default::default()` (never populated); `symphonia-metadata`'s ID3v2
  reader has no CHAP/CTOC frame handling either. (Also, `symphonia` is presently
  unused/dead code — only imported from `src/file_ops/file_ops.rs`, which isn't
  declared as a submodule in `src/file_ops/mod.rs`.)

Confirmed feasibility against a real fixture already on disk: `data/JRRTolkien/.../
The Children of Hurin - ....m4b` contains both a Nero-style `chpl` atom and a
QuickTime `chap` track reference — `ffprobe` (present on this dev machine, not
a project dependency) confirms `ffprobe -show_chapters` on that file returns real
chapters (e.g. "Chapter 1 - The Childhood of Túrin", 0–2348.203s, time_base 1/1000).
This is a byte-for-byte real-world sample to validate the parser against later.

**Decision (user-selected):** implement chapter extraction as **pure-Rust,
hand-rolled parsing** — no `ffmpeg`/`ffprobe` shell-out, no Docker/runtime
dependency changes. Two formats, covering the two audiobook container types this
project already scans (mp3/m4b/m4a per `scan_files.rs`'s extension filter):
1. **MP4 `chpl` atom** (Nero-style chapters) for `.m4b`/`.m4a` — hand-write a
   minimal box walker (moov → udta → chpl), since no maintained pure-Rust crate
   parses this.
2. **ID3v2 `CHAP`/`CTOC` frames** for `.mp3` — add the `id3` crate (new
   dependency) since neither `lofty` nor `symphonia` read these frames, and the
   frame format is fiddly enough (sub-frames, unsynchronization) to prefer a
   maintained parser over hand-rolling.

QuickTime chapter-text-track style (the `chap`-referenced text track, distinct
from `chpl`) is explicitly **out of scope** for the first pass — `chpl` alone
already covers the local fixture and is what most non-Apple encoders (e.g.
`m4b-tool`) write. Known gap for a later session if real-world files turn up
without `chpl`.

This is a multi-part change (schema + scan pipeline + API + UI). Phase 1
(backend extraction+storage) can land and be verified independently before
Phase 2 (API+UI) starts.

## Phase 1 — Backend: parse and store chapters

**New module `src/file_ops/chapters.rs`:**
- `pub struct ChapterInfo { pub idx: i64, pub title: Option<String>, pub start_ms: i64, pub end_ms: i64 }`
- `parse_mp4_chpl(path: &Path, total_duration_ms: i64) -> Vec<ChapterInfo>` —
  walk top-level ISO-BMFF boxes to `moov > udta > chpl` (32-bit size + 4cc box
  format — no existing helper in this codebase to reuse, per feasibility check
  above). Nero `chpl` layout: 1 byte version, 3 bytes flags, 1 byte chapter
  count, then per chapter: 8-byte start time (100ns units, convert to ms via
  `/10_000`), 1-byte title length, title bytes. `end_ms` of chapter *n* =
  `start_ms` of chapter *n+1* (or `total_duration_ms` for the last one — reuse
  the duration `lofty` already extracts in `extract_metadata`, don't re-derive
  it).
- `parse_id3_chapters(path: &Path) -> Vec<ChapterInfo>` — use the new `id3`
  crate to read `CHAP` frames (start/end time already in ms per spec) and
  `CTOC` for ordering; map to `ChapterInfo`. Verify the crate's actual
  `Content::Chapter`/`Content::TableOfContents` API shape at implementation
  time (not yet vendored locally to confirm field names) — the local mp3
  fixtures checked during planning don't happen to contain CHAP frames (they're
  pre-split one-file-per-chapter), so this path needs either a synthetic ID3
  tag in a unit test or a real Audible-style single-file mp3 to validate
  against.
- `pub fn extract_chapters(path: &Path, ext: &str, total_duration_ms: i64) -> Vec<ChapterInfo>`
  dispatches by extension (`m4b`/`m4a` → mp4 parser, `mp3` → id3 parser, else
  empty). **Must never fail the scan** — malformed/absent chapter atoms return
  `vec![]`, only log at debug/warn, mirroring how the rest of `scan_files.rs`
  treats metadata extraction as best-effort.

**Wire into scan (`src/file_ops/scan_files.rs::extract_metadata`, ~line 121):**
call `extract_chapters` after the existing `lofty` tag extraction (duration is
already known by then), store on a new field.

**Model (`src/models/meta_scan.rs::FileScanCache`, ~line 8-31):** add
`pub chapters: Vec<ChapterInfo>` (default `vec![]` in the existing constructor
around line 57).

**New DB table** — migration `migrations/0003_chapters.up.sql` /
`.down.sql` (next number after `0002_add_user_permissions`):
```sql
CREATE TABLE chapters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    idx INTEGER NOT NULL,
    title TEXT,
    start_ms INTEGER NOT NULL,
    end_ms INTEGER NOT NULL,
    UNIQUE(file_id, idx)
);
CREATE INDEX idx_chapters_file_id ON chapters(file_id);
```

**Insert wiring (`src/db/meta_scan.rs::attach_files`, line 274-302):** the
existing bulk `INSERT OR IGNORE INTO files (...)` doesn't return generated ids.
After that insert, for just the rows in this chunk that have non-empty
`chapters`, run a follow-up `SELECT id, file_path FROM files WHERE book_id = ?
AND file_path IN (...)` to map `file_path → file_id`, then bulk-insert into
`chapters` via `QueryBuilder` (same pattern as `attach_files` itself). No
special rescan handling needed — confirmed the existing scanner already skips
re-probing files it's seen before (`fetch_known_file_paths`, meta_scan.rs:332),
so chapters are extracted once at first-scan time just like every other tag
field; this needs no new logic, just piggybacking on the current control flow.

**Model (`src/models/audiobooks.rs`):** add, next to `FileMetadata`:
```rust
pub struct Chapter { pub id: i64, pub file_id: i64, pub idx: i64, pub title: Option<String>, pub start_ms: i64, pub end_ms: i64 }
```

**Verification for Phase 1:** unit tests in `chapters.rs` — one against the raw
`chpl` bytes extracted from the local `Children of Hurin` fixture (or a small
synthetic box buffer) asserting parsed titles/offsets roughly match the
`ffprobe` reference captured during planning; one synthetic-bytes test for the
ID3 CHAP path. Then `cargo run` a real scan against `data/JRRTolkien/...` and
confirm rows land in `chapters` via `sqlite3 rustybookshelf.db "select * from chapters limit 5"`.

## Phase 2 — API + UI

**API (`src/api/audiobooks.rs::file_metadata_handler`, ~line 424):** extend the
`FileMetadata` JSON response with `chapters: Vec<Chapter>`. New DB fn in
`src/db/audiobooks.rs` (near `get_files_by_book_id`), e.g.
`get_chapters_by_file_ids(db, file_ids: &[i64]) -> Result<HashMap<i64, Vec<Chapter>>, ApiError>`,
called once per `file_metadata` request and merged into the response — same
`{ message, count, data }` envelope convention already used by `api/books.ts`.
No new endpoint needed; this is additive to an existing response shape.

**Frontend types (`src/ui/src/types/book.ts`):**
```ts
export interface Chapter { id: number; file_id: number; idx: number; title: string | null; start_ms: number; end_ms: number; }
```
add `chapters: Chapter[]` to `FileMetadata`.

**Player store (`src/ui/src/store/player.ts`):** no new persisted state —
derive current chapter index from `files[index].chapters` + `currentTime` via a
plain helper (e.g. `getCurrentChapterIndex(chapters, currentTimeMs)`), keeping
the store's existing shape (file-index-based) as the source of truth.

**`ChaptersSheet.tsx`:** branch on `files[index].chapters.length > 0` —
when present, render those chapters instead of the file list; clicking one
seeks within the current file (reuse `engine.ts`'s existing `seek(sec)`, convert
`start_ms/1000`) rather than calling `switchToFile`. When absent, keep today's
file-list-as-chapters behavior unchanged (multi-file books untouched).

**`PlayerPage.tsx`** (~line 96-97): the hardcoded `Chapter {index+1} of
{fileList.length}` label needs the same branch — use the embedded chapter
index/count when the current file has them.

No changes needed in `engine.ts` (embedded chapters are offsets within the
already-loaded file, so the existing `seek()` is sufficient) or in
`ReorderChaptersSheet.tsx` (unrelated — that's file-order reordering, confirmed
during exploration to have no overlap with embedded in-file chapters).

**Verification for Phase 2:** `cargo build`, `tsc --noEmit`; live-test via
`cargo run` + `vite dev` against the `Children of Hurin` single-file `.m4b` in
`data/JRRTolkien/...` — open its player page, confirm the chapters sheet lists
real chapter titles/offsets (cross-check a couple against the `ffprobe`
reference captured during planning) and that tapping one seeks correctly;
spot-check an existing multi-file book still shows its normal file-list
behavior unchanged.

## Deferred / out of scope
- QuickTime chapter-text-track style (`chap`-referenced text track, as opposed
  to `chpl`) — not parsed in this pass.
- Ogg/Opus `CHAPTERxxx` Vorbis-comment-style chapters — not covered (no `.ogg`/
  `.opus` in the current scan extension filter anyway).
- Editing/reordering embedded chapters from the UI — read-only for this pass,
  mirroring how `reorder_files` only reorders whole files today.
