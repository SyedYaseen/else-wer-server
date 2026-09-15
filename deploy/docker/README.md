# else-wer with Docker (x86_64)

A single compose file for any host. Everything host-specific (port, library path, bind
mounts, user) comes from `.env` in this directory, which compose reads by itself.

## First run

```sh
cd deploy/docker
cp .env.example .env    # set JWT_SECRET (openssl rand -hex 32) and anything else you need
docker compose up -d --build
```

Or from the repo root: `make env-init && make docker-deploy`.

First start creates a fresh SQLite DB, seeds a "Default" library from `/audiobooks`, and
scans it. It also creates an admin user **`admin` / `admin`** (see
`src/services/startup.rs`) — **change that password immediately**.

## Settings (`.env`)

| Variable | Default | Notes |
|---|---|---|
| `JWT_SECRET` | — | required |
| `HOST_PORT` | `3000` | host port mapped to the container's 3000 |
| `AUDIOBOOKS_DIR` | `../../audiobooks` (repo root) | your library; mounted read-write (organize/upload write here) |
| `DATA_DIR` | `else-wer-data` (named volume) | DB, jwt key, covers. A path makes it a bind mount |
| `LOGS_DIR` | `else-wer-logs` (named volume) | a path makes it a bind mount |
| `PUID` / `PGID` | `0` / `0` | run as this uid:gid. Use your user when the library is shared with other tools, so written files aren't root-owned |
| `COMPOSE_PROJECT_NAME` | `else-wer-server` | also prefixes the named volumes |
| `CONTAINER_NAME` | `else-wer` | |

Running as non-root with bind mounts: the bind masks the image's build-time mkdirs, so
`make docker-*` creates `DATA_DIR/covers`, `DATA_DIR/creds` and `LOGS_DIR` first (`mkdirs.sh`).
Named volumes are root-owned, so a non-root `PUID` needs bind mounts.

## Day to day (from the repo root)

```sh
make docker-deploy   # rebuild + restart
make docker-ps / docker-logs / docker-down / docker-config
make docker-up TRAEFIK=1   # add docker-compose.traefik.yml
```

## Deploying to another machine

Set `DOCKER_SSH=<ssh host>` and `DOCKER_SSH_DIR=<dir with that host's .env>` in the root
`.deploy.env`. `make docker-deploy` then builds the image here, streams it over ssh, and
starts it there with this checkout's compose file. The remote needs only Docker, its `.env`
and its data. It doesn't need an up-to-date checkout. On the remote itself, leave
`DOCKER_SSH` empty and the same targets run locally.
