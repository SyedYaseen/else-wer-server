## User todo/ feature/ bug fix pipeline
- [ ] Unified file serving: /api/stream/{id} (RFC 7233) for streaming + downloads; legacy download_chunk/get_file_size removed; app DownloadManager migrated + streaming playback — plan: todo/1_STREAMING_PLAN.md (CODE-COMPLETE 2026-07-06: server Phases 1-3 validated, app Phases 4-5 done + tsc clean; REMAINING: on-device validation Phases 4.5/6 by user)
- [ ] Metadata reconcile: folder-identity book grouping (fixes 278-single-file-book regression) + series match API + per-folder metadata.json — full plan: todo/2_METADATA_RECONCILE.md (NOT STARTED in this checkout 2026-07-06: was implemented+verified once in an isolated cloud sandbox with no git remote, so that work never left the sandbox and is lost; needs redo here from the plan)
- [x] Lightweight web PWA (iOS stopgap: login/library/player) + metadata editor/reorganizer (replaces else-wer-web prototype) — new sibling project else-wer-pwa (Vite+React SPA), embedded in this server via ServeDir+SPA fallback — full plan: todo/3_WEB_PWA_PLAN.md (ALL PHASES 0-4 DONE 2026-07-08: scaffold/auth/SPA fallback + Library + full Player + PWA manifest/icons/SW + Organize (metadata match/rename/move/merge, selection+action-sheet UX instead of drag/drop since native HTML5 DnD doesn't work on iOS Safari), verified live via the actual Rust server; REMAINING for user: iOS "Add to Home Screen" on-device check, and exercising a real rename/move/merge/match through the Organize UI against the live library)

## Done
Error-handling/middleware remediation (all 4 phases). Full plan: /home/loop/.claude/plans/crystalline-puzzling-sphinx.md
Verified live: cargo build + clippy clean, login enumeration fixed, HEAD auth bypass fixed, path traversal rejected,
404 vs 500 mapping fixed, invalid-token 401 fixed, change_password endpoint works. Server tested manually, not committed
per project convention (user reviews before commit).

- [x] Phase 1 - Security fixes (login enumeration, HEAD auth bypass, upload path sanitization, default admin/admin password change path)
- [x] Phase 2 - Crash-risk unwrap()/expect()/panic! -> proper ApiError propagation (also fixed unrelated clippy::unused_io_amount bug found in the same handler: f.write() -> f.write_all())
- [x] Phase 3 - Status-code & error-currency consistency (anyhow/ApiError unify, 404 vs 400 vs 500, logging levels, leak fix)
- [x] Phase 4 - Swallowed errors & minor cleanup (silent org-save failure, batch abort, debug artifact write, blocking fs call, println->tracing)

## Todo - Deferred
- Whether /scan_files, /list_scanned_files, /save_organized_files, /upload should require AdminUser instead of AuthUser (Phase 1 item 5) - needs user's call, not changed without confirmation.
- Dead `salt` column on users table (Phase 4 item 6) - flagged only, no DB migration planned.

## Todo - out of scope
- Rate limiting on /login
- CORS allow_origin(Any) in src/main.rs
