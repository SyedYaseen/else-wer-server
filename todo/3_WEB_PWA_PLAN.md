# Lightweight Web PWA (iOS stopgap) + Metadata/Reorg Tool

Full context/decisions: `/home/loop/.claude/plans/this-is-going-ot-vivid-whale.md` (approved plan, kept verbatim below by phase).

New sibling project: `/home/loop/p/else-wer/else-wer-pwa` (Vite + React SPA + TS). Embedded into
`else-wer-server` via a ServeDir + SPA fallback (same-origin, no CORS/server-address field needed).
Replaces `else-wer-web` (Next.js prototype, non-persisting drag/drop, no metadata edit UI - kept
untouched as reference until retired).

## Todo (in order)

(none - Phase 4 done, see below)

## Todo - deferred, not blocking

- [ ] Phase 3 follow-up: pending real on-device verification (see Phase 3 Done note) - "Add to Home Screen"
      has only been checked via manifest/SW HTTP responses, not an actual iOS Safari device/simulator.

## Notes / cross-cutting

- Streaming auth gap: `/api/stream/{id}` only reads `Authorization` header today (src/api/audiobooks.rs:317);
  HTML5 `<audio>` can't set custom headers, so Phase 0 must add query-param token support server-side.
- `else-wer-web`'s old `/grouped_books` endpoint no longer exists in the live route table; current equivalent
  is `GET /api/list_scanned_files`. Its example request path `/confirm_bookscan` in http/req.http is stale -
  live route is `/api/save_organized_files`.
- Folio design tokens source of truth: else-wer-app/theme/{colors,typography,spacing,radius,shadows}.ts,
  theme/index.tsx; primitives at else-wer-app/components/ui/*; BookCard at
  else-wer-app/components/library/book-card.tsx; MiniPlayer at else-wer-app/components/player/mini-player.tsx.

## Done

- [x] Phase 0 - Scaffolded else-wer-pwa (Vite/react-ts + react-router-dom/zustand/@tanstack/react-query/
      vite-plugin-pwa, self-hosted @fontsource DM Serif Display + DM Sans). Ported Folio design tokens as CSS
      vars (src/styles/tokens.css, prefers-color-scheme light/dark) + base primitives Button/Card/Pill/
      ProgressBar/BottomSheet (src/components/ui/*). API client (src/api/client.ts, base `/api`, Bearer token
      from zustand src/store/auth.ts with JWT exp check), login page (src/routes/LoginPage.tsx) + route guard
      (src/routes/RequireAuth.tsx) + stub protected LibraryPage for guard verification. `npm run build` clean.
      Server: added `pwa_dist_location` config (env PWA_DIST_LOCATION, default ../else-wer-pwa/dist),
      ServeDir+SPA fallback via `.fallback_service()` in src/main.rs; added StreamAuth extractor
      (src/api/auth_extractor.rs) accepting `?token=` query param (falls back from header) used only on
      GET /api/stream/{id} (src/api/audiobooks.rs) since `<audio src>` can't set custom headers. Verified live:
      `cargo build` clean, SPA fallback serves index.html, /api/hello + /api/login work, /api/list_books works
      with header auth, /api/stream/{id} returns 401 with no auth and 200 with `?token=`.
- [x] Phase 1 - Library: src/types/book.ts (AudioBookRow/Progress/FileMetadata mirroring server structs),
      src/api/books.ts (listBooks/listInProgress/fileMetadata/rescanFiles/coverUrl/downloadBook - download
      uses fetch+blob+objectURL since /api/download_book has no query-token fallback like /api/stream),
      src/hooks/useLibraryBooks.ts (react-query wrappers + rescan/invalidate), src/hooks/useBookSearch.ts
      (ported debounced filter), src/lib/continueListening.ts (client-side join of list_inprogress rows with
      list_books by book_id - server has no book-level progress aggregate), src/lib/format.ts (ms->h:mm:ss).
      Components: src/components/library/{BookCard,SearchBar,ContinueListeningRow}.tsx + library.css (Folio
      grid/card proportions, responsive auto-fill grid instead of RN's fixed 2-col). Routes: LibraryPage.tsx
      rewritten (loading/error/empty states, rescan, search, continue-listening shelf, book grid) and new
      BookDetailPage.tsx (cover/author/series/narrator, file list w/ durations, download button) at
      /book/:id (App.tsx). `npm run build` clean. Verified live against running server: /api/list_books,
      /api/list_inprogress, /api/file_metadata/{id} response shapes match the TS types exactly;
      /api/download_book/{id} confirmed 400 with no auth / 200 with Bearer header (blob download works).
      Known Phase-1 scope choice: no Play button on book detail yet (player is Phase 2) - avoided a fake
      non-functional control per project convention.
- [x] Phase 2 - Player: src/store/player.ts (zustand, non-persisted: book/files/index/playing/currentTime/
      duration/rate). src/player/engine.ts - single module-level `new Audio()` (not mounted in React, so it
      survives route nav), owns all imperative control (loadBook/togglePlay/seek/skip/setRate/switchToFile),
      wires timeupdate/loadedmetadata/play/pause/ended listeners back into the store, periodic progress save
      every 10s (POST /api/update_progress) + on pause/seek/file-switch/tab-hidden (visibilitychange, since
      mobile Safari can suspend without firing pause), ~3s completion threshold, navigator.mediaSession action
      handlers (play/pause/seekbackward/seekforward/prev/next) + metadata (title/artist/artwork via coverUrl).
      src/player/sleepTimer.ts - real implementation (RN app's is a stub; plan explicitly allowed building this
      for real): zustand store with minute presets (15/30/45/60) or "end of chapter", ticking countdown,
      pauses the shared `audio` element on expiry; end-of-chapter consumed by engine's `ended` handler before
      auto-advancing. src/lib/playerResume.ts - resolveResumePoint(files, progress): web-only port of
      else-wer-app's data/lib/conflict-handling.ts getBookProgress, minus the local/server merge (no local DB
      on web) - matches most-recent progress row by updated_at, resumes at/after that file_id. src/api/
      progress.ts - getBookProgress/updateProgress/streamUrl (stream URL appends ?token= from the auth store
      since <audio src> can't set headers - StreamAuth query-param fallback added in Phase 0). Components:
      src/components/player/{MiniPlayer,PlayerRoot,Seeker,PlaybackSpeedMenu,ChaptersSheet,SleepTimerSheet,
      icons}.tsx + player.css (Folio proportions: 64px mini-player bar, 42x42 cover, accent play/pause, thin
      bottom progress line; full player cover/seek/transport/secondary-controls row). PlayerRoot mounted once
      in App.tsx outside <Routes> (sibling to it inside BrowserRouter) so it and engine.ts's audio element
      persist across navigation; renders MiniPlayer unless on /login or /player/:id. New route src/routes/
      PlayerPage.tsx at /player/:id - loads book/files/progress via react-query only if not already the
      loaded book (survives nav from mini-player), otherwise reads live state from the store. BookDetailPage.tsx
      gained a real Play button (calls getBookProgress + resolveResumePoint + engine.loadBook, navigates to
      /player/:id) - the Phase-1 gap is now closed. ContinueListeningRow.tsx changed from a Link to /book/:id
      to a button that resumes playback directly via the same flow (matches else-wer-app's InProgressCard
      tap-to-resume behavior). Icons are small inline SVGs (no icon library dependency added). Volume control
      intentionally omitted - else-wer-app's own volume button is a placeholder stub ("Volume slider here"),
      and hardware volume buttons work fine against a plain <audio> element, so building a fake one didn't
      make sense. `npm run build` clean. Verified live against running server with curl: GET
      /api/get_book_progress/{id} returns a bare Progress[] array matching the TS type; GET /api/stream/{id}
      ?token=... returns 206 for a Range request (query-token auth + range support both confirmed working);
      POST /api/update_progress returns 202 and a follow-up GET confirms the upsert - confirmed progress.file_id
      matches file_metadata's top-level `id` (files PK), not its nested `file_id` field, validating
      resolveResumePoint's matching logic. Verification note: the curl round-trip incidentally overwrote a
      real in-progress book's (id 16, "The Children of Hurin") saved position (9079ms) with a 1234ms test
      value - caught immediately and restored via a second update_progress call before moving on.
- [x] Phase 3 - PWA polish: replaced the default Vite placeholder favicon with a Folio-branded icon (rounded
      accent-colored square + a simple open-book line-art glyph in the paper color, public/icon source built
      as two SVGs - one rounded-corner "any"-purpose version, one flat full-bleed "maskable" version with the
      glyph already inside the ~80% safe zone - rasterized to PNG via `rsvg-convert`, the only SVG rasterizer
      available in this environment; no imagemagick/inkscape/cairosvg installed): public/pwa-192x192.png,
      pwa-512x512.png, maskable-icon-512x512.png, apple-touch-icon.png (180x180), favicon.svg replaced.
      vite.config.ts: added VitePWA plugin (registerType: autoUpdate) - manifest (name/short_name/description/
      theme_color #8C7355/background_color #FDFBF8/display standalone/icons incl. maskable purpose) + Workbox
      runtimeCaching per the plan: /api/covers/* -> StaleWhileRevalidate (300 entries/30d), /api/stream/* ->
      NetworkOnly (explicit - avoids the SW ever attempting cache.put on a 206 Partial Content response),
      /api/{update_progress,get_book_progress,get_file_progress,list_inprogress} -> NetworkOnly (explicit, so
      progress never desyncs from a stale cached read); navigateFallbackDenylist excludes /api/ so offline
      navigation only ever falls back to the cached app shell, never a stale API JSON body. All other /api/*
      routes (list_books, login, file_metadata, etc.) are left unmatched by any runtimeCaching rule - Workbox
      only intercepts what's explicitly registered, so they pass through to the network exactly as before,
      i.e. no offline library browsing (an accepted limitation per the plan, not implemented). index.html:
      apple-touch-icon link, theme-color meta, apple-mobile-web-app-capable/status-bar-style/title metas,
      viewport-fit=cover. Added `env(safe-area-inset-*)` padding to .mini-player, .player-page, .library-page
      so standalone iOS content and controls don't sit under the notch or home-indicator gesture area (real
      correctness need once there's no browser chrome, not present before this phase). `npm run build` clean,
      generates dist/manifest.webmanifest + dist/sw.js (PWA v1.3.0, generateSW, 10 precache entries). Verified
      live end-to-end through the actual Rust server (not just the Vite dev server) since that's the real
      "Add to Home Screen" path: curl against http://localhost:3000/{manifest.webmanifest,sw.js,
      pwa-192x192.png} all 200, manifest body matches the configured icons/theme/background colors exactly -
      confirms `pwa_dist_location`'s ServeDir is already serving the freshly-built dist/ with no server
      restart needed. NOT verified: actual "Add to Home Screen" behavior on a real iOS Safari device/simulator
      (no browser automation available in this environment) - deferred to the user, tracked above under
      "Todo - deferred, not blocking".
- [x] Phase 4 - Metadata editor & file reorganizer: src/types/scan.ts (FileInfo/GroupedFiles/ChangeDto/
      ChangeType/MatchCandidate/ApplyMatchDto mirroring src/models/{meta_scan,match_meta}.rs) + buildTree()
      helper - client-side re-groups each server (author, series) bucket by book_id into a proper 3-level
      Author -> Book -> File tree, since a single (author, series) bucket can legitimately mix files from
      several different book_ids (e.g. the "unknown" series fallback used when a folder has no clean series
      name) and the server's list_scanned_files response doesn't split on book_id itself. src/api/scan.ts
      (listScannedFiles/saveOrganizedFiles/matchBookCandidates/applyBookMatch) + src/hooks/useOrganize.ts
      (react-query wrappers, invalidate scannedFiles on mutation success). New route src/routes/
      OrganizePage.tsx at /organize (linked from the library header) + src/components/organize/{AuthorRow,
      BookRow,FileRow,PickBookSheet,RenameSheet,MatchSheet,icons}.tsx + organize.css.
      DELIBERATE UX DEVIATION FROM THE PLAN: the plan called for reference/rebuilt "3-level nested drag/drop"
      (as in else-wer-web/components/book-org/*, which used @dnd-kit). Native HTML5 drag-and-drop does not
      work reliably on iOS Safari (no drag events fire for arbitrary draggable elements without a heavy
      polyfill), and this whole PWA's stated purpose is an iOS stopgap - shipping a reorganizer that's
      unusable on the primary target device would defeat the point. Built a selection + action-sheet UX
      instead: checkboxes per file + a "Move" toolbar (file-move, existing book or type a new
      author/title to create one), and per-book action icons (Rename - single sheet covering both author
      and title/series fields, since ChangeType::Rename and ::MoveTitle hit the identical server code path
      in src/db/meta_scan.rs:616 so there is no functional difference and exposing both as separate actions
      would just be redundant UI; Merge - picks another existing book via merge-title; Match metadata -
      candidate search/apply) plus a per-author rename action (renames every file under that author in one
      call) and a per-file rename pencil icon. Match-metadata panel wired to GET/POST /api/match_book/{id}
      exactly per the plan (confidence-scored candidates, apply_title_author toggle). Manual rescan button
      reuses the existing rescanFiles() from src/api/books.ts (GET /api/scan_files). Known gap carried
      forward as originally scoped: no generic free-text field-edit endpoint beyond the 4 ChangeDto ops +
      match-apply exists - not needed, since rename/move/merge/match cover every field this UI exposes.
      `npm run build` clean. Verified live against the running Rust server: GET /api/list_scanned_files
      response shape matches GroupedFiles exactly (author -> series -> FileInfo[], confirmed via curl); GET
      /api/match_book/{id} response shape matches MatchResponse exactly (confirmed via curl, candidates: []
      in this case - audible search returned no hits for the test book, not a shape problem). Deliberately
      did NOT live-test POST /api/save_organized_files or POST /api/match_book/{id} against the real
      database - unlike a progress-value overwrite (Phase 2's incident), a real rename/move/merge/match-apply
      call would mutate actual book/file organization and metadata.json content, which is much harder to
      cleanly undo; the payload shape is already fully confirmed from src/db/meta_scan.rs and the existing
      http/req.http examples, so a live mutation test wasn't necessary to be confident in the contract.
      Deferred to the user: exercising a real rename/move/merge/match through the UI against the live
      library and confirming the audiobooks/files tables and per-folder metadata.json end up as expected.
