# 7. OSS Release Prep (docs, CI, packaging)

Source: business/GA audit, /home/loop/.claude/plans/do-a-business-audit-cheeky-orbit.md (2026-07-11).
Context: non-code (or low-risk) work needed before telling strangers to self-host this and accepting
outside contributions. Independent items.

## Todo
- [ ] README rewrite - current `readme.md` is a personal dev log (curl test commands, Pi cross-compile
  notes), no setup guide. GA needs: what this is, screenshots, Docker quickstart, env var reference,
  upgrade/migration notes, and a link to the existing `deploy/https-duckdns/` HTTPS guide. Likely the
  single highest-leverage task - most users bounce at "how do I install this."
- [ ] Version/upgrade path docs - migrations are embedded in the binary via `sqlx::migrate!()` and run
  automatically, but there's no documented "how do I upgrade my Docker instance without losing data"
  story, nor a rollback story if a migration fails mid-upgrade on someone's Pi.
- [ ] Community/support channel (GitHub Discussions or Discord) - expect support load the moment
  install docs exist; not code but should be ready before announcing.
- [ ] Bare-metal release doc section (README or docs/bare-metal.md) - explain the release.yml
  tarballs (below): download for your arch, set JWT_SECRET/PWA_DIST_LOCATION=./pwa/etc per the
  packaged README.txt, install as systemd unit, link deploy/https-duckdns/ for TLS. Needs a
  generic (non-hardcoded-user) systemd unit template - `piconfig/else-wer.service` is this
  session's owner's personal Pi unit (User=pi, /home/pi paths), not meant to double as the public
  one.

## Done
- [x] LICENSE file (2026-07-11): AGPL-3.0 full text added at repo root, fetched from
  choosealicense.com's raw GitHub source (front-matter stripped, verbatim FSF license body kept).
- [x] Docker healthcheck (2026-07-11): docker-compose.yml `else-wer` service gained a `healthcheck`
  block hitting `wget -qO- http://localhost:3000/api/health` (30s interval/5s timeout/3 retries/5s
  start_period). Runtime image didn't have wget - added it to the Dockerfile's existing
  `apt-get install` line (ca-certificates/libssl3) alongside. Not committed (user reviews first).
- [x] Minimal CI - .github/workflows/ci.yml (2026-07-11): `rust` job builds a throwaway sqlx DB
  (mirrors Dockerfile's rust-build stage - query!/query_as! macros need a real schema at compile
  time) then runs cargo build/test/clippy against it; `pwa` job runs `tsc --noEmit` + `oxlint` in
  src/ui. No `-D warnings` on clippy - repo currently has ~33 pre-existing warnings on this branch,
  would red the very first PR. Verified locally end-to-end (throwaway db create+migrate, cargo
  build/test(27/27 pass)/clippy, tsc clean, oxlint exits 0 w/ 1 pre-existing warning) before
  writing the workflow file, so this should go green as-is on first push. Not committed.
- [x] Release binaries - .github/workflows/release.yml (2026-07-11, triggers on `v*` tags): `pwa`
  job builds once, uploads dist as an artifact; `build` job matrixes x86_64-unknown-linux-gnu +
  aarch64-unknown-linux-gnu via `cross` (same tool piconfig/Makefile already uses for Pi deploys -
  chosen over cargo-zigbuild to reuse a proven pattern rather than introduce an untested one),
  packages `else-wer-<target>.tar.gz` (binary + pwa/ + LICENSE + README.txt with the actual
  required/default env vars per src/config.rs - JWT_SECRET has no default, PWA_DIST_LOCATION must
  be overridden to ./pwa since its default `src/ui/dist` assumes a source checkout), uploads via
  softprops/action-gh-release. NOT YET TESTED against a real tag push/cross's docker-based cross-
  compilation (no CI runner access from this session) - first real tag push should be watched.
  Deferred/still open: generic systemd unit + bare-metal install doc section (see Todo above) -
  piconfig/ is intentionally left untouched as the owner's personal Pi deploy tooling, not the
  public release path.
