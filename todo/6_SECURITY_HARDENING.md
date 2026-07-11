# 6. Security Hardening (pre-GA)

Source: business/GA audit, /home/loop/.claude/plans/do-a-business-audit-cheeky-orbit.md (2026-07-11).
Context: findings from that audit that are must-fix before this can be a public self-hosted OSS
release (someone other than the author exposing an instance to the internet). Independent items,
can be done in any order or split across sessions.

## Todo
(none - all items resolved 2026-07-11, see Done. Not committed - user reviews first.)

## Done
- [x] Rate limit `/api/login` (2026-07-11): in-memory per-(ip,username) tracker, new
  `src/api/rate_limit.rs` (`LoginRateLimiter`, `AppState.rate_limiter`). 5 failures within a 15 min
  window locks the key out for 15 min, doubling on further failures while locked (capped at 1h);
  success clears the entry. No new crate - `std::sync::Mutex`/`HashMap`/`Instant` only. Needed
  `ConnectInfo<SocketAddr>` wired into `main.rs`'s `axum::serve` via
  `into_make_service_with_connect_info`, and a new `ApiError::TooManyRequests` -> 429 variant.
  Verified live: 5 bad-password POSTs to `/api/login` -> 401,401,401,401,429 (locked), success
  login clears the counter.
- [x] Restrict CORS (2026-07-11): `src/config.rs` gained `cors_allowed_origins: Vec<String>`
  (`CORS_ALLOWED_ORIGINS` env, comma-separated, empty by default). `src/main.rs`'s CORS layer now
  builds an exact-match `Vec<HeaderValue>` allowlist instead of `.allow_origin(Any)` - empty list
  means no cross-origin site gets `Access-Control-Allow-Origin` back (same-origin PWA is
  unaffected, since same-origin requests never need CORS headers). Documented in
  `.env.docker.example`. Verified live: OPTIONS preflight with `Origin: https://evil.example`
  returns 200 but no `access-control-allow-origin` header.
- [x] Force admin password change on first login - **decision: document only, no code
  enforcement** (see todo/7_OSS_RELEASE_DOCS.md item 1, README rewrite - add an explicit "change
  the default admin/admin password immediately via `PUT /api/user/change_password`" callout there;
  the startup `tracing::warn!` in `src/services/startup.rs` already covers the runtime-log side).
  A hard-block middleware flow was considered and explicitly deferred by the user.
- [x] JWT revocation story (2026-07-11): added `token_version` (i64, default 0) to `users` via
  migration `0003_token_version_drop_salt`; embedded in `Claims` and checked against a fresh DB
  read on every request in both `AuthUser` and `StreamAuth` extractors
  (`src/api/auth_extractor.rs`'s `verify_token_version`, backed by new
  `src/db/user.rs::get_token_version`). `update_user_password` now bumps `token_version` in the
  same UPDATE as the password hash, so any previously-issued JWT for that user is rejected (401
  "Token revoked") even before its 24h `exp`. Costs one indexed PK lookup per authenticated
  request - accepted tradeoff for real revocation. Verified live: login -> use token (200) ->
  change password -> same token now 401 -> fresh login token works (200).
- [x] Dead `salt` column on `users` table (2026-07-11): removed via the same migration
  (`0003_token_version_drop_salt.up.sql`, `ALTER TABLE users DROP COLUMN salt`), plus all
  read/write references in `src/models/user.rs` (`User` struct) and `src/db/user.rs`
  (create/get/list/update queries - `create_user`/`update_user_password` signatures dropped their
  `salt`/`new_salt` params accordingly). Argon2 still generates and embeds its own salt in the PHC
  `password_hash` string via `SaltString::generate` at hash time - only the redundant separate
  column and its callers went away.
- [x] `OrganizeUser` vs `AdminUser` gating on `/scan_files`, `/list_scanned_files`,
  `/save_organized_files`, `/upload` - **already resolved in code, no change needed**: all four
  routes already require the `OrganizeUser` extractor (`src/api/audiobooks.rs`), which requires
  `role == "admin" || can_organize` (`src/api/middleware.rs`). Confirmed via exploration
  2026-07-11; the `todo/CLAUDE_PROGRESS.md` "Todo - Deferred" note about this is now stale/removed.

## Verification (all items)
`cargo build` + `cargo clippy` clean. Live-tested end-to-end against a scratch copy of the dev DB
(migration applied there first, real `rustybookshelf.db` untouched) via `cargo run` + curl: CORS
preflight rejection, rate-limit lockout sequence, and full login -> change-password ->
old-token-401 -> new-token-200 revocation flow. Not committed - user reviews before commit.
