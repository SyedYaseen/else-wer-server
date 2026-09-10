# 16. Remove the `/hello` endpoint

Status: **done** — 2026-09-09, alongside else-wer-app Phase 2

## What it was

`/hello` was a scratch/debug endpoint returning `Html("<h1>Hello</h1>")` over a body of
commented-out debris. Its only caller was `else-wer-app`, in two places:
- `app/_layout.tsx` — cold-start check (authenticated, doubled as a token check)
- `components/hooks/useNetworkToggle.ts` — manual online/offline toggle ping

## Resolution

`else-wer-app` Phase 2 (`todo/pwa-parity/02_auth_network.md`) moved both callers off it:
- `GET /health` for reachability (both the cold-start probe and the manual toggle)
- `POST /refresh_token` for the token check — `/health` is unauthenticated and can't 401 on a dead
  token, whereas `/refresh_token` both validates and returns a fresh token in one request

Re-ran the cross-client grep before deleting — no hits in `src/ui/src`, `../else-wer-web/app`, or
`../else-wer-app`:
```
grep -rn "hello" --include=*.ts --include=*.tsx src/ui/src ../else-wer-web/app ../else-wer-app
```

Deleted: the route and the `hello` handler in `src/api/mod.rs` (plus the now-unused `Html` import and
the "deliberately separate from /hello" aside on `health_handler`), and the `http/req.http` entry.

`cargo build` clean.

**Note for old clients:** a build that predates Phase 2 still pings `/hello` and now gets a 404. Both
of its call sites treated `404` as "reachable", so an un-updated app keeps working.
