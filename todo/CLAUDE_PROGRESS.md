## Seekers (line numbers as of last edit; re-grep if file has grown)
- Todo pipeline: L18
  - UI feedback initiative (5-phase): L19 | Phase1 Toast L21 | Phase2 Button feedback (open) L27 | Phase3 Settings page L30 | Phase4 Dark/light toggle L46 | Phase5 Offline indicator L64
  - Docker packaging: L80
  - PWA offline downloads+playback (open): L94
  - Unified file serving /api/stream (open): L102
  - Metadata reconcile (open): L103
  - Lightweight web PWA / Organize: L104
  - Business/GA audit (2026-07-11): split into todo/6_SECURITY_HARDENING.md,
    todo/7_OSS_RELEASE_DOCS.md, todo/8_FRONTEND_TESTING.md, todo/9_FEATURE_GAPS.md
- Done: L106
  - Organize author/book move+merge: L107
  - Manual chapter reordering: L121
  - file_scan_cache removal: L139
  - file-scan branch review+fixes: L148
  - Error-handling/middleware remediation (4 phases): L157 (sub-phases from L162)
- Todo - Deferred: L167
- Todo - out of scope: L171

## User todo/ feature/ bug fix pipeline
- [ ] UI feedback initiative (2026-07-11, uncommitted): 5-phase plan, full details in
  /home/loop/.claude/plans/add-feedback-for-all-glimmering-wind.md.
  - [x] Phase 1 - Toast system: new src/ui/src/lib/toast.ts (imperative pub/sub, showToast()/subscribeToast()) +
    src/ui/src/components/ui/Toaster.tsx, mounted in App.tsx alongside PlayerRoot/InstallBanner. Wired into
    offline/downloadStore.ts's startDownload() success/error paths (module-level store already had exactly one
    success/one error path, so no dedupe logic needed). Verified live: cargo run + vite dev, logged in as admin/admin,
    downloaded "The Hobbit" via real browser (playwright-core/firefox, no chromium-cli available in this env),
    success toast rendered top-center with correct book title, zero console errors. Screenshot confirmed visually.
  - [ ] Phase 2 - Button/interaction feedback (:hover/:active/:focus-visible on .btn/.tab/.action-menu-*) -
    NOTE: user appears to have already started this concurrently (ui.css got :hover/:active/:focus-visible rules
    on .btn and .tab mid-session, not authored by this session's agent) - check ui.css current state before redoing.
  - [x] Phase 3 - Settings page: new /settings route (src/ui/src/routes/SettingsPage.tsx + settings.css), authenticated
    but not admin-only (page itself gates "Manage users" on isAdmin). "Manage users" + "Log out" moved fully out of
    LibraryPage's ActionMenu (was 2 items, now a single "Settings" item w/ new SettingsIcon; LibraryPage's now-unused
    isAdmin/logout selectors + LogoutIcon import removed) per user's explicit choice (asked via AskUserQuestion - full
    move over keep-both-copies). Downloads section: offline/storage.ts gained listDownloadedBooks() (StoredBook now
    exported, reads the existing `books` IndexedDB store via withBooksStore) and clearAllDownloads() (direct .clear()
    on all 3 stores - O(1) vs looping deleteBookDownload's O(books x files) cursor walks); per-row delete reuses
    existing deleteBookDownload(). Size shown via new formatBytes() in lib/format.ts, summed from FileMetadata.file_size
    (on-device size, not server book_size). Delete/clear-all use inline BottomSheet confirm sheets (matches
    UserManagementPage's pattern) rather than reusing ConfirmActionSheet (its hardcoded "Confirm changes" title didn't
    fit a destructive single action - smallest-change call, not touched). New TrashIcon/SettingsIcon in ui/icons.tsx.
    Appearance section built as a static "Dark mode coming soon" stub so Phase 4 needs zero routing/layout work.
    Plan: /home/loop/.claude/plans/the-remaining-4-phases-gleaming-crane.md. Verified live: cargo run + vite dev,
    playwright-core/firefox (no chromium-cli in this env) - logged in as admin/admin, confirmed LibraryPage menu shows
    only "Settings", downloaded "The Hobbit", Settings page listed it (653 MB), deleted via trash icon + confirm sheet
    -> success toast + row disappeared + IndexedDB entry actually gone. tsc --noEmit clean. Not committed.
  - [x] Phase 4 - Dark/light toggle: 3-way System/Light/Dark control (user chose 3-way over a plain binary toggle via
    AskUserQuestion, to preserve today's OS-following behavior as the default). New src/ui/src/store/theme.ts
    (useThemeStore, mode: 'system'|'light'|'dark') - deliberately NOT wrapped in zustand's persist middleware like
    store/auth.ts; persists as a raw localStorage string (key elsewer-theme) instead, because a blocking inline
    <script> in index.html (added right after <meta charset>) must read the saved mode and stamp
    data-theme="light"|"dark" on <html> before React mounts, to avoid a flash of the wrong theme when the manual
    choice conflicts with the OS setting - a raw string is trivial for that script to read, whereas persist's
    versioned JSON envelope would require duplicating zustand's internal format in a dependency-free inline script.
    tokens.css gained :root[data-theme='dark'] / :root[data-theme='light'] blocks (exact copies of the existing
    :root and @media (prefers-color-scheme: dark) values) - attribute selectors beat prefers-color-scheme's bare
    :root on specificity so they override the OS setting when present; absent (mode=system) falls through to the
    unchanged existing media-query behavior. SettingsPage's Appearance stub replaced with the existing Tabs component
    (src/components/ui/Tabs.tsx) as the 3-way segmented control - no new Switch component built. Verified live:
    cargo run + vite dev, playwright-core/firefox - clicked Dark (data-theme=dark, bg flipped to #141210 immediately),
    reloaded (stayed dark, no flash - confirms the inline script works), clicked Light (data-theme=light, bg back to
    #FDFBF8), clicked System (data-theme attr removed, localStorage='system'), spot-checked Library page in dark mode
    (cards/tabs/search all render correctly). tsc --noEmit clean. Plan:
    /home/loop/.claude/plans/the-remaining-4-phases-gleaming-crane.md. Not committed.
  - [x] Phase 5 - Offline/online indicator: new unauthenticated GET /api/health (src/api/mod.rs, no DB access,
    Json({"status":"ok"})); client poller src/ui/src/lib/healthPoll.ts (recursive setTimeout, not setInterval -
    lets retry cadence tighten to 3s starting at the *first* failure rather than only after the visible state
    flips, so recovery detection doesn't lag behind the 15s online interval; 2 consecutive failures required to
    flip the visible "offline" state to avoid flicker on one blip, 1 success flips back instantly; 2.5s per-probe
    timeout independent of api/client.ts's 8s NETWORK_TIMEOUT_MS - deliberately not reusing api.get() since that
    wrapper's timeout/auth-header/JSON-parsing are all wrong-shaped for a liveness probe; document.hidden pauses
    polling, visibilitychange resumes with an immediate check instead of waiting out a stale interval); store
    src/ui/src/store/networkStatus.ts (optimistic reachable:true default, no flash on cold load); component
    src/ui/src/components/ui/NetworkBanner.tsx, renders null unless unreachable, mounted in App.tsx alongside
    InstallBanner/Toaster (module-level startHealthPolling() call, same "outside React lifecycle" pattern as
    engine.ts's Audio() singleton); CSS .network-banner in ui.css. No navigator.onLine/online/offline anywhere.
    Full design plan: /home/loop/.claude/plans/distributed-twirling-forest.md. Verified: cargo build clean, our
    files clippy-clean, our files tsc/oxlint-clean (LibraryPage.tsx has 2 pre-existing tsc errors from concurrent
    Phase 3 work-in-progress, unrelated to this change - not touched), live curl http://localhost:3000/api/health
    -> 200 {"status":"ok"} with no Authorization header. Not committed (user reviews first).
- [x] Docker packaging for else-wer-server (2026-07-11): Dockerfile (3-stage: node pwa-build, rust-build w/ sqlx-cli
  used only to `sqlx migrate run` a throwaway build.db to satisfy `sqlx::query!` macro type-checking at compile time,
  debian:bookworm-slim runtime), .dockerignore, docker-compose.yml (named volume `else-wer-data` -> /data for
  db+creds+covers, ./audiobooks -> /audiobooks bind mount, port 3000), .env.docker.example (JWT_SECRET required, rest
  defaulted in image). Migrations are embedded in the binary via `sqlx::migrate!()` (src/db/mod.rs:24) so runtime image
  doesn't need migrations/ copied in. covers/ symlinked to /data/covers since code does `current_dir().join("covers")`
  (src/file_ops/book_cover.rs:43). Verified live: `docker compose up --build` -> migrations ran, admin/admin created,
  GET / returned PWA (200), POST /api/login returned valid JWT (202). Not committed (user reviews first). Plan:
  /home/loop/.claude/plans/this-is-a-audiobookshelf-twinkling-summit.md
  REMAINING (next session, marketing/docs site): Astro+Starlight else-wer-site scaffold at
  /home/loop/p/else-wer/else-wer-site (theme tokens/fonts/logo ported from src/ui/src/styles/tokens.css +
  public/favicon.svg — full values captured in the plan file), marketing landing page, docs pages (quickstart, docker,
  https-duckdns [port from deploy/https-duckdns/README.md], wireguard-remote-access [new content], ios-offline-limitations,
  faq), AGPL-3.0 LICENSE + donate link, Cloudflare Pages deploy pointing else-wer.com.
- [ ] PWA offline downloads + playback (2026-07-09, uncommitted): IndexedDB storage (src/ui/src/offline/storage.ts, DB v3 —
  audio stored as 8MB segments to avoid iOS Safari memory-crash reloads that looked like "download resets"; book/file
  metadata snapshot store for offline cold-start; legacy v2 whole-blob records still readable), module-level download
  store (offline/downloadStore.ts — survives route unmounts, throttled progress, dupe-guard, visible error), engine.ts
  prefers local blob over streamUrl (iOS fires <audio> error late/never), localStorage progress fallback (playerResume.ts).
  REMAINING: on-device iOS verification by user. BLOCKER for offline cold-start: service worker needs HTTPS —
  deploy/https-duckdns/{setup.sh,duckdns-update.sh,README.md} added (DuckDNS + Let's Encrypt DNS-01 + Caddy, no
  domain/port-forward needed); user must run setup.sh on the server box and re-Add-to-Home-Screen from the HTTPS origin.
- [ ] Offline/online mode rework (2026-07-16, uncommitted): 30d JWT + /refresh_token + offline grace (no login bounce
  offline), 401→logout in client.ts, LibraryPage offline shelf from IndexedDB, server LWW progress upsert (client
  updated_at), SW precaches fonts/icons. CODE-COMPLETE (cargo check + npm build clean); REMAINING: on-device/two-device
  verification by user — full detail: todo/10_OFFLINE_MODE.md
- [ ] Unified file serving: /api/stream/{id} (RFC 7233) for streaming + downloads; legacy download_chunk/get_file_size removed; app DownloadManager migrated + streaming playback — plan: todo/1_STREAMING_PLAN.md (CODE-COMPLETE 2026-07-06: server Phases 1-3 validated, app Phases 4-5 done + tsc clean; REMAINING: on-device validation Phases 4.5/6 by user)
- [ ] Metadata reconcile: folder-identity book grouping (fixes 278-single-file-book regression) + series match API + per-folder metadata.json — full plan: todo/2_METADATA_RECONCILE.md (NOT STARTED in this checkout 2026-07-06: was implemented+verified once in an isolated cloud sandbox with no git remote, so that work never left the sandbox and is lost; needs redo here from the plan)
- [x] Lightweight web PWA (iOS stopgap: login/library/player) + metadata editor/reorganizer (replaces else-wer-web prototype) — new sibling project else-wer-pwa (Vite+React SPA), embedded in this server via ServeDir+SPA fallback — full plan: todo/3_WEB_PWA_PLAN.md (ALL PHASES 0-4 DONE 2026-07-08: scaffold/auth/SPA fallback + Library + full Player + PWA manifest/icons/SW + Organize (metadata match/rename/move/merge, selection+action-sheet UX instead of drag/drop since native HTML5 DnD doesn't work on iOS Safari), verified live via the actual Rust server; REMAINING for user: iOS "Add to Home Screen" on-device check, and exercising a real rename/move/merge/match through the Organize UI against the live library)

## Done
Author/book move + merge in Organize (2026-07-11, uncommitted): lets users move a book's or author's files to a
different (or brand-new) author/book, or merge them into an existing author/book, via a searchable picker + confirm
step. New src/ui/src/components/organize/SearchableCombobox.tsx (generic filter-as-you-type combobox: existing items
+ "Create new: <text>" row, so users pick the canonical entry instead of retyping a near-duplicate); PickAuthorSheet.tsx
(author-only picker wrapping SearchableCombobox, mirrors the existing PickBookSheet); ConfirmActionSheet.tsx (generic
confirm sheet listing new lib/describeChanges.ts's summary lines before any submitChanges() call — now sits in front
of rename/move/merge alike, not move/merge-specific). OrganizePage.tsx's sheet state gained
move-author/merge-author/move-book/merge-book kinds; move-author and merge-author both submit as
change_type: 'rename' (merging into an existing author is just renaming the source author's files to the target
author server-side); move-book submits 'rename' with new_author; move-selected/merge-book submit
'file-move'/'merge-title' with new_book_id: -1 when the target is a brand-new book. AuthorRow.tsx and BookRow.tsx
gained onMoveAuthor/onMergeAuthor and onMoveBook/onMergeBook icon-buttons (new MoveIcon/MergeIcon in ui/icons.tsx)
wired to these. Not committed (user reviews first).

Manual chapter reordering (2026-07-11): fixes wrong scan-derived chapter order. Backend: `reorder_files()` in
src/db/audiobooks.rs (txn UPDATE, validates file_ids matches book's existing files, flattens disc_number=0 +
sequential track_number — safe against rescans since files INSERT is OR IGNORE and no other path updates
track_number/disc_number on existing rows, confirmed by investigation, so no lock column/migration needed); handler
`reorder_files_handler` (OrganizeUser-gated) + route `POST /file_metadata/{book_id}/reorder` in src/api/audiobooks.rs
+ src/api/mod.rs. Frontend: new shared src/ui/src/components/ui/ReorderChaptersSheet.tsx (self-fetches via
GET /file_metadata/{id} rather than trusting caller data, since Organize's tree file order is scan/insertion order
not playback order; up/down move buttons per user's explicit choice over drag-and-drop — no dnd library exists in
this mobile-first PWA); wired into both player/ChaptersSheet.tsx (new reorder icon button; store/player.ts gained
`reorderFiles()` action that remaps the playing index by file id, not position, preserving currentTime/duration) and
organize/BookRow.tsx (new icon-action per book row). New icons ArrowUp/ArrowDown/Reorder in ui/icons.tsx. Plan:
~/.claude/plans/chapters-need-to-be-stateful-lark.md. Verified live: cargo build + clippy clean (no new warnings),
tsc --noEmit clean, oxlint clean; cargo run + vite dev + playwright-core/firefox — reordered "the lord of the rings -
the return of the king" (book id 7, 3 files) via both Organize and Player sheets, confirmed order persisted server-side
(GET /file_metadata/7 track_number order), confirmed player's currently-playing chapter correctly remapped position
after reorder ("Chapter 1 of 3" -> "Chapter 2 of 3" when the playing file moved to slot 2), zero console errors. Not
committed (user reviews first).

file_scan_cache removal / files-table consolidation (2026-07-09): dropped `file_scan_cache`; `files` is now the single
per-file ledger and `files.id` the single file-id space everywhere (org UI, progress, downloads). Full plan/status:
todo/4_FSC_CONSOLIDATION.md. Migration 0008 required rebuilding `progress` twice around the `files` rebuild to work
around an sqlx-sqlite 0.8.6 bug (`-- no-transaction` is not honored; PRAGMA foreign_keys no-ops inside the transaction
sqlx always wraps migrations in, so a naive rebuild silently cascade-deleted progress rows — caught via scratch-copy
testing, fixed, re-verified). Code: cargo build/test(27/27)/clippy clean against a scratch DB copy (real
rustybookshelf.db never touched, verified via checksum). DB migration deliberately NOT applied to the real dev DB this
session (user said "code only, skip DB") — next session must run it before the server works. Not committed.

file-scan branch review + fixes (2026-07-09): reviewed uncommitted scan/reconcile/series-grouping diff — design confirmed sound
(Audible auto-groups series via backfill, /assign_series manual override with series_locked, ASIN dedup, metadata.json restore).
Fixed 4 findings: (1) backfill_metadata now spawns detached + returns 202/409 (was: inline run leaked backfill_running flag on
client disconnect; unified as try_spawn_metadata_backfill, also used by scan endpoints); (2) new migration 0007 backfills
files.track_number/disc_number from file_scan_cache for pre-0006 libraries (attach_files is INSERT OR IGNORE so they'd stay NULL;
0006 left untouched — dev DB already applied it, checksum); (3) restore_series_link guard += AND series_locked = 0;
(4) main.rs bind uses lookup_host so HOST=localhost works. Verified: 27/27 tests, migration SQL against dev-DB copy,
live server 202→409→finished-log cycle. Not committed (user reviews first). Review notes: ~/.claude/plans/go-through-the-git-radiant-wind.md

Error-handling/middleware remediation (all 4 phases). Full plan: /home/loop/.claude/plans/crystalline-puzzling-sphinx.md
Verified live: cargo build + clippy clean, login enumeration fixed, HEAD auth bypass fixed, path traversal rejected,
404 vs 500 mapping fixed, invalid-token 401 fixed, change_password endpoint works. Server tested manually, not committed
per project convention (user reviews before commit).

- [x] Phase 1 - Security fixes (login enumeration, HEAD auth bypass, upload path sanitization, default admin/admin password change path)
- [x] Phase 2 - Crash-risk unwrap()/expect()/panic! -> proper ApiError propagation (also fixed unrelated clippy::unused_io_amount bug found in the same handler: f.write() -> f.write_all())
- [x] Phase 3 - Status-code & error-currency consistency (anyhow/ApiError unify, 404 vs 400 vs 500, logging levels, leak fix)
- [x] Phase 4 - Swallowed errors & minor cleanup (silent org-save failure, batch abort, debug artifact write, blocking fs call, println->tracing)

## Todo - Deferred
(none - both prior items resolved 2026-07-11 in todo/6_SECURITY_HARDENING.md: OrganizeUser gating
was already correct in code; salt column removed via migration 0003.)

## Todo - out of scope (superseded for GA, resolved 2026-07-11 - see todo/6_SECURITY_HARDENING.md)
- ~~Rate limiting on /login~~ - done (in-memory per-ip+username lockout)
- ~~CORS allow_origin(Any) in src/main.rs~~ - done (env-configurable allowlist, empty by default)
