# Unified Audiobook File Serving — Revised Streaming Plan

## Context

Revisit of `todo/1_STREAMING_PLAN.md`. The old plan added a streaming endpoint but walled off `download_chunk` as untouchable. Investigation found `download_chunk`/`get_file_size` are genuinely broken and redundant:

- **Wrong-file bug**: both call `get_file_path` (`src/db/audiobooks.rs:171`) which queries `WHERE file_id = ? ORDER BY id` + `fetch_one`. `file_id` is a per-book track index (UNIQUE(book_id,file_id,file_path)), NOT unique — it silently serves the lowest-id match, potentially another book's file.
- Buffers each chunk fully into RAM (`vec![0; chunk_size]` + `read_exact`), hardcodes `audio/mpeg`, uses custom `?start=&end=` params (not RFC 7233), always 206, rejects instead of clamping ranges.
- Only consumer is the app's DownloadManager (`data/lib/download-manager.ts:198` — verified, sole reference).

**Decision (user-confirmed)**: one RFC-7233 endpoint `GET/HEAD /api/stream/{id}` (keyed on files PK `id`) serves BOTH streaming playback (expo-audio) and offline downloads (DownloadManager, minimal migration keeping its worker-pool architecture). Legacy `download_chunk`/`get_file_size`/`get_file_path` are removed — user self-hosts both server and app, updated in lockstep. The download-for-offline-listen flow is preserved: it becomes Range requests against `/stream/{id}` (Phase 4).

First execution step: replace `todo/1_STREAMING_PLAN.md` content with this revised plan (it's the cross-session memory per project convention) and update `todo/CLAUDE_PROGRESS.md`.

Verified facts (do not re-derive):
- `get_file_path` callers: only the two deleted handlers (`src/api/audiobooks.rs:305,322`).
- App `FileRow` (`data/database/models.ts:15-27`) stores BOTH server PK `id` and `file_id`; `fetchFileMetaFromServer` returns both. Progress sync already keys on PK `id`.
- `audiobook-repo.ts` `updateFilePath`/`getFile` are dead code (zero callers) — the only fns that would conflate fileId with DB `file_id`.
- AsyncStorage `server` value already includes `/api`. Token in AsyncStorage `token`.
- `ApiError`: sqlx `RowNotFound` → 404 already mapped.
- axum 0.8 route syntax is `{id}`; `routing::get()` also matches HEAD and ServeFile handles HEAD itself — do NOT chain `.head(...)`.

---

## Phase 1 — Server: `/api/stream/{id}` (RFC 7233 via tower-http ServeFile)

### 1.1 `Cargo.toml`
```toml
tower = { version = "0.5", features = ["util"] }   # ServiceExt::oneshot
mime = "0.3"                                        # ServeFile::new_with_mime
```
(Both already transitive via tower-http 0.6.)

### 1.2 `src/db/audiobooks.rs` — lookup by PK
```rust
pub async fn get_file_path_by_id(db: &Pool<Sqlite>, id: i64) -> Result<String, ApiError> {
    let path: (String,) = sqlx::query_as(r#"SELECT file_path FROM files WHERE id = ?"#)
        .bind(id).fetch_one(db).await?;
    Ok(path.0)
}
```

### 1.3 `src/api/audiobooks.rs` — handler
```rust
use axum::extract::Request;
use tower::util::ServiceExt;
use tower_http::services::ServeFile;

// mime_guess (inside ServeFile) doesn't map .m4b; players are picky.
fn audio_mime(path: &str) -> &'static str {
    match std::path::Path::new(path).extension().and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase()).as_deref()
    {
        Some("mp3") => "audio/mpeg",
        Some("m4a") | Some("m4b") | Some("mp4") => "audio/mp4",
        Some("aac") => "audio/aac",
        Some("flac") => "audio/flac",
        Some("ogg") | Some("oga") | Some("opus") => "audio/ogg",
        Some("wav") => "audio/wav",
        _ => "application/octet-stream",
    }
}

// GET/HEAD /api/stream/{id} — id = files PK. Range/206/416/Accept-Ranges/If-Range,
// streamed body (no buffering).
pub async fn stream_file(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
    Path(id): Path<i64>,
    req: Request, // LAST arg (FromRequest) — forwards Range header to ServeFile
) -> Result<impl IntoResponse, ApiError> {
    let file_path = get_file_path_by_id(&state.db_pool, id).await?;
    if !PathBuf::from(&file_path).exists() {
        return Err(ApiError::NotFound("File not found".into()));
    }
    let mime = audio_mime(&file_path).parse::<mime::Mime>()
        .map_err(|_| ApiError::Internal("bad mime".into()))?;
    ServeFile::new_with_mime(&file_path, &mime)
        .oneshot(req).await
        .map_err(|e| ApiError::Internal(format!("stream error: {e}")))
        // if Service::Error = Infallible: use .map_err(|e| match e {}) — no unwraps
}
```

### 1.4 `src/api/mod.rs`
In the `// Files` group: `.route("/stream/{id}", get(stream_file))` (no `.head()`). Add `stream_file` to the audiobooks import.

### 1.5 `cargo build` (+ quick clippy). Fix only errors introduced here.

---

## Phase 2 — Server: remove legacy endpoints

- `src/api/audiobooks.rs`: delete `get_file_size` (~300-314), `download_chunk` (~316-352), `DownloadParams` (~294-298). Prune now-unused imports (`Query`, `AsyncSeekExt`, `SeekFrom`; keep `AsyncReadExt`/`AsyncWriteExt` — upload_handler uses them).
- `src/db/audiobooks.rs:171-186`: delete `get_file_path`; fix import at `src/api/audiobooks.rs:2`.
- `src/api/mod.rs`: delete `/download_chunk/{file_id}` route (~43-46) + imports.
- `http/req.http` (~108-119): replace download_head/download_chunk examples with `HEAD .../stream/2` and `GET .../stream/2` + `Range: bytes=0-1023`.
- Leave `download_book` (zip) alone.
- `cargo build`.

---

## Phase 3 — Server validation (manual curl)

Login per `http/req.http` (localhost:3000), get a PK id via `/api/file_metadata/<book_id>`. Against `/api/stream/<id>`:
1. Plain GET → 200, correct Content-Type (.mp3 → audio/mpeg, .m4b → audio/mp4), `Accept-Ranges: bytes`, Content-Length = size.
2. `Range: bytes=0-1023` → 206, `Content-Range: bytes 0-1023/<size>`, 1024-byte body.
3. `Range: bytes=<size+1000>-` → 416 with `Content-Range: bytes */<size>`.
4. `curl -I` (HEAD) → 200 + Content-Length, no body (download manager's size probe).
5. `Range: bytes=1000000-` → 206 at offset.
6. No/garbage auth → 400/401; id 999999 → 404.
7. **Bug-fix check**: two books each having a file with `file_id = 1` — stream by distinct PK ids, confirm different content.
8. Old `/api/download_chunk/1?start=0&end=10` → 404/405 (gone).

**RESULTS (2026-07-06, all PASS — Phases 1-3 DONE):**
1. GET id=1 (.mp3) → 200, `audio/mpeg`, `accept-ranges: bytes`, len 56838507; id=2 (.m4b) → 200, `audio/mp4`, len 684942445.
2. `Range: bytes=0-1023` → 206, `content-range: bytes 0-1023/56838507`, body exactly 1024 B.
3. `Range: bytes=<size+1M>-` → 416, `content-range: bytes */56838507`.
4. HEAD → 200, content-length + accept-ranges, no body.
5. `Range: bytes=1000000-` → 206, `bytes 1000000-56838506/56838507`.
6. No auth → 400; garbage token → 401; id 999999 → 404.
7. First bytes differ by PK: id=1 `ID3` (mp3) vs id=2 `ftypM4A`. (Dev DB has no duplicate file_id across books, so the old wrong-file scenario wasn't directly reproducible; PK lookup removes the bug class.)
8. `/api/download_chunk/1?start=0&end=10` → 404 (removed).

---

## Phase 4 — App: DownloadManager migration (minimal swap)

**STATUS (2026-07-06): CODE-COMPLETE.** All edits in 4.1–4.3 applied; `npx tsc --noEmit` clean. Remaining: on-device verify (4.5).

Keep worker pool (4×6MB chunks, retry, queue persistence). Identity: pass the PK id as the existing `fileId` payload field — verified safe (`item.fileId` consumers: manager URL+keys, download-store keys, a React `key` in `downloads.tsx:193`).

### 4.1 `app/(tabs)/book/[id].tsx:85`
`fileId: f.file_id` → `fileId: f.id`.

### 4.2 `data/lib/download-manager.ts`
- Line ~198: `` const fileUrlBase = `${baseUrl}/stream/${fileId}` `` (baseUrl already has `/api`).
- HEAD size probe (~201): URL change only.
- Chunk fetch (~279-283): drop `?size=&start=&end=`; send `Range: bytes=${c.start}-${c.end}` header and **require 206**:
```ts
if (res.status !== 206) throw new Error(`expected 206, got ${res.status}`)
```
(A 200 full body written at `fHandle.offset = c.start` would corrupt the file.)

### 4.3 Persistence version bumps (mandatory — old persisted items hold per-book file_id values that would hit `/stream/{file_id}` → wrong file/404)
- download-manager.ts (~28, 80): `download_queue_v1` → `download_queue_v2`.
- `components/store/download-store.ts:202`: `download-store-v1` → `download-store-v2`.
- Accepted: pre-migration in-flight items vanish from Downloads screen; completed books unaffected (local_path in SQLite + files on disk).

### 4.4 Accepted behavior notes (don't fix)
- On-disk name `${fileId}_${fileName}` now uses PK id; fully-downloaded books keep playing via DB local_path. Re-downloading a pre-migration book uses new names; old files removed by existing whole-dir delete.
- `download:${key}:localPath` AsyncStorage writes (~170) are write-only — leave.

### 4.5 Verify
`npx tsc --noEmit`; on device: download multi-file book → progress ring, files land, plays offline; kill mid-download → reopen → re-enqueues and completes; server logs show HEAD then 206s.

---

## Phase 5 — App: streaming playback

**STATUS (2026-07-06): CODE-COMPLETE.** All edits in 5.1–5.5 applied; `npx tsc --noEmit` clean. Remaining: on-device validation (Phase 6, user).

### 5.1 `data/api/api.ts` — source helpers
```ts
export async function getStreamSource(fileId: number) {  // fileId = server PK id
  const server = await AsyncStorage.getItem('server');    // already includes /api
  const token = await AsyncStorage.getItem('token');
  return { uri: `${server}/stream/${fileId}`, headers: { Authorization: `Bearer ${token}` } };
}
// local file wins; else stream when online; else null (caller skips)
export async function resolvePlaybackSource(file: FileRow) {
  if (file.local_path) return file.local_path;
  if (useNetworkState.getState().isOnline) return getStreamSource(file.id);
  return null;
}
```

### 5.2 `app/player/[id].tsx`
- `loadBookData` (~26-31): when `getFilesForBook` returns [] and online, fall back to `fetchFileMetaFromServer(bookId)` rows (FileRow shape, local_path undefined). Keep `noFiles` throw for offline/empty.
- `applyBook` (~137-139): `const src = await resolvePlaybackSource(next); if (!src) return; player.replace(src);` (`seekAfterLoad` unchanged — Range makes seek work).

### 5.3 `components/hooks/useProgressUpdate.ts` (~60-65, auto-advance; sync effect — wrap async)
```ts
const next = poppedQ[0];
(async () => {
  const src = await resolvePlaybackSource(next);
  if (!src) return;
  player.replace(src);
  player.play();
})().catch(console.error);
```

### 5.4 `components/player/secondary-controls/chapters.tsx`
- ~73: `if (fileRow.local_path)` → `if (fileRow.local_path || useNetworkState.getState().isOnline)`.
- ~91: `player.replace(newQueue[0].local_path!)` → resolve via `resolvePlaybackSource`, skip if null.

### 5.5 `app/(tabs)/book/[id].tsx` play button (~168-171)
`disabled={!isDownloaded}` → `disabled={!isDownloaded && !isOnline}` (`useNetworkState`).
Progress sync unchanged. Known limitation (note only): streamed playback has no offline cache.

---

## Phase 6 — App validation (device against dev server)

1. Non-downloaded book online: plays within seconds; scrubbing → 206 Range requests in server logs; auto-advance and chapter switch stream; progress syncs; reopen resumes.
2. Downloaded book still plays offline (regression).
3. Partially downloaded book: local files play locally, rest stream.
4. Offline + not downloaded: play disabled, no null-source crashes.
5. Re-run Phase 4.5 download regression.

Run `graphify update .` after server code changes.

---

## Out of scope
- Web UI / query-param stream token (old plan Phase 4) — deferred unchanged (short-lived scoped JWT `?token=`, separate `StreamAuthUser` extractor, CORS exposure).
- `download_book` (zip) endpoint; DownloadManager rewrite onto `createDownloadResumable`; transcoding/HLS; offline stream caching; CORS.

## Fallback (only if ServeFile can't be wired)
Hand-rolled single-range handler: parse `Range: bytes=a-b`, clamp end to `size-1`, 416 when `start >= size`, `tokio::fs::File` + seek + `take(len)` + `tokio_util::io::ReaderStream` → `Body::from_stream` with Content-Type/Length/Range + `Accept-Ranges: bytes`. Deps already present.
