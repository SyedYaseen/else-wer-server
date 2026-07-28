# 9. Feature Gaps vs. Established Audiobook Apps

Source: business/GA audit, /home/loop/.claude/plans/do-a-business-audit-cheeky-orbit.md (2026-07-11).
Context: comparison against Audiobookshelf, Plex/Jellyfin, BookPlayer, Smart AudioBook Player,
commercial apps (Audible/Libro.fm). Grouped by likely adoption impact for a self-hosted OSS release.
Each is independent and can be picked up in its own session.

## Todo - likely to matter for adoption
- [ ] Casting/external playback - Chromecast, AirPlay, or at minimum an "open in system player"
  fallback. Player is a custom in-page `<audio>` engine only (`player/engine.ts`), no cast support.
- [ ] Server-side backup/restore - no documented/scripted way to export+import the SQLite DB + covers,
  or snapshot before a Docker upgrade.
- [ ] User self-service - no user-driven password change (only admin can reset any user's password via
  `PUT /api/user/change_password`), no open-registration toggle. (Forgot-password via email is a
  separate non-goal - see below.)
- [ ] Metadata provider fallback chain - currently Audible-only (`src/services/audible.rs`). Comments
  mention Audnexus/Audimeta.de/Open Library as considered alternatives, none implemented; add a
  fallback chain for when Audible has no match.

## Explicit non-goals (document as "not planned" rather than silently missing)
- Forgot-password (email-based recovery) - requires SMTP infra, not worth it for a self-hosted app.
  (Self-service change-own-password while logged in is a separate, planned item - see Todo.)
- Multi-server support - the PWA is origin-bound by platform design (relative `BASE_URL` in
  `src/ui/src/api/client.ts`, manifest `start_url`/`scope`, per-origin `localStorage` JWT, origin-scoped
  service worker). No code path lets one installed PWA reach multiple server IPs, and building one would
  fight the PWA model. Users add each server as a separate "Add to Home Screen" install - already works
  today with zero code changes.
- Podcast support (Audiobookshelf's other pillar) - decide explicitly and note in README if staying
  audiobook-only.
- Ratings/reviews, cross-book playlists/queue, family/parental content restrictions, closed
  captions/transcripts, volume normalization/boost, pitch-corrected variable speed beyond native
  `playbackRate`. Differentiators, not blockers - only build if a specific user asks.

## Done
- [x] Listening stats/history - per-user/book/day `listening_stats_daily` aggregate table +
  `finished_books` table (`migrations/0004_listening_stats.up.sql`), populated via a
  wall-clock-based `listened_delta_ms` sent on the existing `POST /update_progress` payload
  (`src/ui/src/player/engine.ts`). New `GET /api/stats/daily` and `GET /api/stats/finished`
  endpoints (`src/api/stats.rs`), `/stats` page (`src/ui/src/routes/StatsPage.tsx`) linked from
  Settings. See plan: `/home/loop/.claude/plans/listening-quiet-pine.md`.
- [x] Bookmarks - new `bookmarks` table (`migrations/0005_bookmarks.up.sql`, N-per-file, unlike
  `progress`'s one-per-file uniqueness), `src/models/bookmarks.rs`, `src/db/bookmarks.rs`,
  `src/api/bookmarks.rs` (`POST /bookmarks`, `GET /bookmarks/book/{book_id}`,
  `DELETE /bookmarks/{bookmark_id}`). Player UI: `BookmarksSheet.tsx` (add/jump/delete from the
  now-playing screen); also listed per-book on `BookDetailPage.tsx`. See plan:
  `/home/loop/.claude/plans/gentle-watching-hearth.md`.
- [x] Server-side search - `GET /list_books?q=` now filters server-side (`LIKE`-based, matching
  title/author/series/narrated_by; see `src/db/audiobooks.rs::search_books`) instead of shipping the
  full table and filtering client-side. `useBookSearch.ts` hits the endpoint for non-empty debounced
  queries via react-query; empty query still returns the already-fetched list with no extra request.
  See plan: `/home/loop/.claude/plans/gentle-watching-hearth.md`.
- [x] Multi-library/multi-folder support - new `libraries` table + `audiobooks.library_id`
  (`migrations/0006_libraries.up.sql`; `ON CONFLICT(files_location, title)` deliberately kept
  library-agnostic since roots can't collide in practice - see migration comment). `AUDIOBOOKS_LOCATION`
  now only seeds a one-time "Default" library backfill at startup
  (`services/startup.rs::ensure_default_library`); libraries are otherwise fully DB-managed.
  Scan pipeline (`file_ops/scan_files.rs`, `db/meta_scan.rs`) threads `library_id` through per-call so
  scanning one library can't mark another's files as removed (`fetch_known_file_paths` scoped
  per-library - verified live: scanning a 2nd library added its book without touching the first
  library's 21). Admin CRUD in `src/api/libraries.rs` (`GET/POST /libraries`, `PUT/DELETE
  /libraries/{id}`, `POST /libraries/{id}/scan`); `GET /list_books?library_id=` filter. Frontend:
  library selector on `LibraryPage.tsx`, admin screen `LibrariesAdminPage.tsx` linked from Settings.
  Follow-up closed: `upload_handler` now accepts an optional `libraryId` multipart field (falls back
  to `get_default_library` - the literal "Default" library, or the earliest-created one if renamed/
  missing - when omitted, so existing callers with no `libraryId` are unaffected); post-upload rescan
  now scans only the target library instead of all of them. Organize screen's scanned-files tree
  (`get_grouped_files`) now takes an optional `library_id` filter (`GET /list_scanned_files?library_id=`);
  `OrganizePage.tsx` gets the same library-selector `Tabs` row as the Library page when >1 library
  exists. Verified live: uploaded to a 2nd library via `libraryId`, file landed in the right folder;
  filtered `/list_scanned_files` by library and confirmed no cross-library leakage. No frontend upload
  UI exists in this PWA today (`/upload` is used by the separate else-wer-web app per CLAUDE.md) - the
  backend change is purely additive/backward-compatible, no client-side upload flow to wire up here.
  See plan: `/home/loop/.claude/plans/gentle-watching-hearth.md`.
