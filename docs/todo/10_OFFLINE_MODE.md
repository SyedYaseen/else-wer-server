# Offline/Online Mode Rework (2026-07-16)

Plan: /home/loop/.claude/plans/offline-online-mode-needs-enumerated-lark.md
Symptoms fixed: offline cold-start dead-end, spurious login redirects offline, daily
re-login, stale-device progress clobbering. Deployment is HTTPS (valvasban.duckdns.org
→ LAN), so the SW/secure-context blocker from todo/CLAUDE_PROGRESS.md L101 is resolved.

## Phases (all CODE-COMPLETE, uncommitted; cargo check + npm run build clean)

- [x] Phase 1 — Auth: 30-day JWT + silent refresh + offline grace
  - src/api/user.rs: Duration::days(30); issue_jwt() extracted from auth_and_issue_jwt;
    new refresh_token handler (AuthUser extractor → re-fetch user by id → fresh token;
    self_hosted only). Route POST /refresh_token in src/api/mod.rs.
  - src/ui/src/routes/RequireAuth.tsx: guards on token presence only (no local exp check).
  - src/ui/src/api/client.ts: 401 on authenticated paths → logout() (RequireAuth redirects).
  - src/ui/src/api/auth.ts: refreshToken(); new src/ui/src/lib/authRefresh.ts
    startTokenRefresh() (refresh on start + reachable false→true), wired in App.tsx.
- [x] Phase 2 — Offline shelf on LibraryPage
  - src/ui/src/routes/LibraryPage.tsx: on useLibraryBooks error, listDownloadedBooks()
    (IndexedDB) renders BookCard grid + ContinueListeningRow built from getLocalProgress;
    header actions hidden, "Offline — showing downloaded books" hint. BookDetailPage/
    PlayerPage/engine offline fallbacks already existed (unchanged).
- [x] Phase 3 — Server-side last-write-wins progress
  - src/models/user.rs: ProgressUpdate.updated_at: Option<DateTime<Utc>> (serde default).
  - src/db/sync.rs upsert_progress: VALUES(..., COALESCE(?6, CURRENT_TIMESTAMP)),
    DO UPDATE ... WHERE excluded.updated_at > progress.updated_at. Client ts formatted
    "%Y-%m-%d %H:%M:%S%.3f" (lexicographically comparable with CURRENT_TIMESTAMP rows).
  - Client sends updated_at everywhere: engine.ts saveProgress (single shared ISO ts for
    server + localStorage row), playerResume.ts reconcileProgress push-back + flush replay
    original local.updated_at.
- [x] Phase 4 — SW precache: vite.config.ts globPatterns adds ico/png/svg/woff2
    (fonts/icons now precached; 25 entries, 576 KiB).
- [x] Phase 5 — Cold-start hardening (iPhone report: offline cold start showed
    "no audiobooks / scan" instead of shelf). LibraryPage now keys the shelf on
    serverDown = isError || !reachable (health poll flips in ~3-6s, vs 30s+ for
    query retries + 8s timeouts on dead wifi); "Loading" hidden once offlineMode;
    "No audiobooks yet / Scan" gated on !serverDown so offline never invites a
    scan; error state keyed on serverDown. NOTE: user's phone was running the
    OLD deployed build (9f0b692, no shelf at all) — after deploying, the phone
    must load the app ONCE while online so the SW precaches the new bundle;
    only then will offline cold starts work.

## Verified live (2026-07-16, local server on :3210, admin/admin)

- POST /login → token exp = +30 days; POST /refresh_token → fresh token; garbage token → 401.
- LWW upsert via API: fresh client-ts save stored; stale 2020 replay REJECTED (row
  unchanged); newer server-stamped save applied. Test progress row deleted afterwards.

## REMAINING (user/on-device verification)

1. Deploy fresh dist + rebuilt server. On phone over HTTPS: confirm SW active, download a
   book, airplane mode, kill + reopen PWA → shell loads, offline shelf lists the book,
   playback from IndexedDB blob.
2. Expired-token offline: hand-edit elsewer-auth exp in localStorage (or wait), offline
   reload → must NOT redirect to /login.
3. Online 401: change password (token_version bump) → next API call logs out to /login.
4. Silent refresh: reload online → token exp in localStorage advances to ~30 days out.
5. Two-device progress: A offline listens → reconnect flushes (watch /update_progress);
   B sees new position. Reverse-stale case: B listened later while A offline → A's flush
   must be dropped by the SQL guard and A resumes from B's position.

## Notes / deferred

- Clock skew between devices now matters for LWW (all writes carry client timestamps);
  same-user devices are NTP-synced in practice. If it ever bites, consider server-stamping
  live saves and only trusting client ts for replays.
- Offline shelf covers rely on the SW covers-cache (StaleWhileRevalidate) having seen them;
  cold cache shows the letter placeholder. Deferred: store cover blob in the IndexedDB book
  snapshot at download time.
- Offline Continue-Listening progressMs is the single cached row (current file), an
  underestimate of whole-book progress — cosmetic only.
