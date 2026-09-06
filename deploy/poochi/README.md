# else-wer on poochi

Replaces the Pi Zero 2 W deploy in `../../piconfig/`. poochi is `192.168.1.18`,
Ubuntu 24.04, x86_64 — so this builds natively, no `cross`.

The repo is cloned on poochi at `~/projects/else-wer-server`.

## Isolation

poochi already runs Traefik (:80/:443, `themizadah.com`), a Cloudflare Tunnel,
gluetun+qbittorrent, and Jellyfin. This stack touches none of them:

- own compose project, own implicit bridge network — not `mz-net`, not `qbt_default`
- `traefik.enable=false`
- host port **3030** (3000 is taken by `mzbe`)
- runs as `1000:1000` so files written into `/home/poochi/hdd/bookshelf/audiobooks`
  stay `poochi`-owned — qbittorrent bind-mounts that whole tree as `/data` and
  root-owned files there break its hardlinking

## First deploy

```sh
cd ~/projects/else-wer-server/deploy/poochi

# Required: the ./data bind mount masks the image's build-time mkdir, so these
# must exist on the host or the server can't write its DB, jwt key, or covers.
# ./logs is likewise required -- init_logging() writes to a relative "logs" dir
# (/app/logs in the container), root-owned in the image otherwise.
mkdir -p data/covers data/creds logs

cp .env.example .env
sed -i "s/^JWT_SECRET=.*/JWT_SECRET=$(openssl rand -hex 32)/" .env

docker compose up -d --build
```

First start creates a fresh SQLite DB at `data/else-wer.db`, seeds a "Default"
library from `/audiobooks`, and scans it. It also creates an admin user with
**`admin` / `admin`** (see `src/services/startup.rs`) — **change that password
immediately** after first login.

Reach it at `http://192.168.1.18:3030`.

## Day to day

```sh
docker compose ps
docker compose logs -f
docker compose up -d --build     # redeploy after a git pull
docker compose down
```

The library lives on the host at `/home/poochi/hdd/bookshelf/audiobooks`; the
DB, jwt key, and generated covers live in `./data/` (gitignored).

## Hostname + TLS (not applied yet)

`docker-compose.traefik.yml` puts this behind the existing Traefik at
`https://elsewer.syedyaseen.dev`. It needs a Porkbun certResolver added to
poochi's Traefik and a DNS record first — see
`../../docs/todo/15_ELSEWER_HOSTNAME_TLS.md`.
