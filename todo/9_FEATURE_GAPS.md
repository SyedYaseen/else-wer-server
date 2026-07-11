# 9. Feature Gaps vs. Established Audiobook Apps

Source: business/GA audit, /home/loop/.claude/plans/do-a-business-audit-cheeky-orbit.md (2026-07-11).
Context: comparison against Audiobookshelf, Plex/Jellyfin, BookPlayer, Smart AudioBook Player,
commercial apps (Audible/Libro.fm). Grouped by likely adoption impact for a self-hosted OSS release.
Each is independent and can be picked up in its own session.

## Todo - likely to matter for adoption
- [ ] Bookmarks - save a specific timestamp with a note, independent of the existing single-position
  "resume" (`progress` table only tracks one position per user/book/file today).
- [ ] Casting/external playback - Chromecast, AirPlay, or at minimum an "open in system player"
  fallback. Player is a custom in-page `<audio>` engine only (`player/engine.ts`), no cast support.
- [ ] Multi-library/multi-folder support - single `AUDIOBOOKS_LOCATION` today. Useful once an install
  has >1 household member with distinct collections.
- [ ] Server-side backup/restore - no documented/scripted way to export+import the SQLite DB + covers,
  or snapshot before a Docker upgrade.
- [ ] User self-service - no "forgot password," no user-driven password change (only admin can reset
  any user's password via `PUT /api/user/change_password`), no open-registration toggle.
- [ ] Server-side search - `GET /api/list_books` returns everything, `SearchBar` filters client-side
  only. Fine at hundreds of books, won't scale to thousands.
- [ ] Metadata provider fallback chain - currently Audible-only (`src/services/audible.rs`). Comments
  mention Audnexus/Audimeta.de/Open Library as considered alternatives, none implemented; add a
  fallback chain for when Audible has no match.

## Explicit non-goals (document as "not planned" rather than silently missing)
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
