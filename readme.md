# else-wer

A self-hosted audiobook server with a Rust backend and a PWA frontend. Point it at your audiobook
library, run it in Docker, and stream to any browser (installable as an app on iOS/Android via the
PWA).

Features: library scanning with metadata extraction, per-user playback progress sync, multi-user
accounts with admin roles, cover art, and file organization tools.

## Quickstart (Docker)

```bash
cd deploy/docker
cp .env.example .env
# edit .env: set JWT_SECRET (openssl rand -hex 32) and AUDIOBOOKS_DIR (your library)

docker compose up -d --build
```

The server is now available at `http://localhost:3000`. Or from the repo root:
`make env-init && make docker-deploy`.

Volumes (named volumes by default; set a path in `.env` for a bind mount):
- `DATA_DIR` → `/data` — database, JWT signing key, cover art
- `AUDIOBOOKS_DIR` (default `audiobooks/` at the repo root) → `/audiobooks` — your audiobook library

## Configuration

Set these in `deploy/docker/.env`. The full list is in [`deploy/docker/README.md`](deploy/docker/README.md):

| Variable | Required | Default | Description |
|---|---|---|---|
| `JWT_SECRET` | yes | — | Secret used to sign auth tokens. Generate with `openssl rand -hex 32`. |
| `HOST_PORT` | no | `3000` | Host port to publish. |
| `AUDIOBOOKS_DIR` | no | repo-root `audiobooks/` | Your audiobook library. |
| `PUID` / `PGID` | no | `0` | Run the container as this uid:gid. |
| `RUST_LOG` | no | `info` | Log verbosity. |
| `CORS_ALLOWED_ORIGINS` | no | — | Comma-separated allowed origins. Only needed if you're serving the frontend from a different origin than the API. |

## Make targets (development + our own hosts)

`make help` lists everything. Machine-specific settings (ssh hosts, the Pi's user and drive)
go in `.deploy.env` (see `.deploy.env.example`). Real env files are gitignored; `make env-init`
creates the missing ones from their `*.example` templates with a fresh `JWT_SECRET` and never
overwrites.

| Target | What it does |
|---|---|
| `make dev` | native local dev: `cargo run` + vite (uses `.env`) |
| `make docker-deploy` | docker, here or on `DOCKER_SSH` over ssh ([`deploy/docker/`](deploy/docker/README.md)) |
| `make pi-deploy` | Pi: cross-build, scp, systemd restart ([`deploy/pi/`](deploy/pi/README.md)) |
| `make pull` | `git pull` that backs up and restores local env files |

## HTTPS / installing as a PWA

iOS only registers a PWA service worker over a secure (HTTPS) context. If you want to install
else-wer as an app on your phone without port-forwarding or owning a domain, see
[`deploy/https-duckdns/`](deploy/https-duckdns/README.md) for a ~5 minute DuckDNS + Let's Encrypt +
Caddy setup.

## Upgrading

Database migrations run automatically on container startup — there's no manual migration step.
Upgrading is just pulling a new image and restarting:

```bash
git pull
make docker-deploy     # or: cd deploy/docker && docker compose up -d --build
```

Your data lives in `DATA_DIR` (the `else-wer-data` volume by default). Back it up before major version upgrades.

## License

[PolyForm Strict 1.0.0](LICENSE): free for personal, noncommercial use. Redistribution and
distributing modified versions are not permitted.
