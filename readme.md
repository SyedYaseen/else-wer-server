# else-wer

A self-hosted audiobook server with a Rust backend and a PWA frontend. Point it at your audiobook
library, run it in Docker, and stream to any browser (installable as an app on iOS/Android via the
PWA).

Features: library scanning with metadata extraction, per-user playback progress sync, multi-user
accounts with admin roles, cover art, and file organization tools.

## Quickstart (Docker)

```bash
cp .env.docker.example .env.docker
# edit .env.docker and set JWT_SECRET, e.g.:
openssl rand -hex 32

mkdir -p audiobooks   # put your audiobook library here

docker compose up -d
```

The server is now available at `http://localhost:3000`.

Volumes:
- `else-wer-data` (named volume) → `/data` — database, JWT signing key, cover art
- `./audiobooks` (bind mount) → `/audiobooks` — your audiobook library

## Configuration

Set these in `.env.docker` (see `.env.docker.example`):

| Variable | Required | Default | Description |
|---|---|---|---|
| `JWT_SECRET` | yes | — | Secret used to sign auth tokens. Generate with `openssl rand -hex 32`. |
| `PORT` | no | `3000` | Port the server listens on. |
| `RUST_LOG` | no | `info` | Log verbosity. |
| `CORS_ALLOWED_ORIGINS` | no | — | Comma-separated allowed origins. Only needed if you're serving the frontend from a different origin than the API. |

## HTTPS / installing as a PWA

iOS only registers a PWA service worker over a secure (HTTPS) context. If you want to install
else-wer as an app on your phone without port-forwarding or owning a domain, see
[`deploy/https-duckdns/`](deploy/https-duckdns/README.md) for a ~5 minute DuckDNS + Let's Encrypt +
Caddy setup.

## Upgrading

Database migrations run automatically on container startup — there's no manual migration step.
Upgrading is just pulling a new image and restarting:

```bash
docker compose pull
docker compose up -d
```

Your data lives in the `else-wer-data` volume. Back it up before major version upgrades.

## License

Not yet licensed.
