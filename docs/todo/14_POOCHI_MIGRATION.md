Status: in progress (2026-09-05)

## Problem

The Pi Zero 2 W that hosts else-wer keeps falling over. Everything on it hangs off a
single dwc_otg root port — the library drive and the RTL8153 ethernet adapter share one
Genesys Logic hub — so a hub reset re-enumerates both, systemd unmounts the library, and
the service goes with it. `piconfig/99-else-wer-library-drive.rules`,
`piconfig/else-wer.service`'s `RequiresMountsFor=`/`WantedBy=home-pi-drv.mount`, and
`13_USB_HUB_STABILITY.md` are all scar tissue from that one root cause. Rather than keep
patching it, else-wer moves to `poochi` (192.168.1.18), the always-on Ubuntu 24.04 x86_64
box already on the LAN.

Goal for this doc: else-wer serving the merged library over plain HTTP on the LAN, with
zero disruption to what poochi already runs. The `elsewer.syedyaseen.dev` hostname and TLS
are a separate doc — `15_ELSEWER_HOSTNAME_TLS.md`.

## What poochi already runs (must not break)

| Thing | Detail |
|---|---|
| Traefik | `/home/poochi/mz/proxy`, owns :80/:443, Cloudflare DNS-01 wildcard for `themizadah.com`, `exposedByDefault: false` |
| Cloudflare Tunnel | `cloudflared` on `mz-net` |
| gluetun + qbittorrent | `/home/poochi/projects/qbt`, own `qbt_default` net, `traefik.enable=false`, binds `${LAN_IP}:8080` |
| Jellyfin | `/home/poochi/projects/media`, host port 8096, runs `user: 1000:1000` |
| Storage | `/home/poochi/hdd` — 916G, ~449G free before the merge. **qbittorrent bind-mounts all of it as `/data`** |
| Ports | 3000 taken by `mzbe`; 3030 free |
| sudo | requires a password — mount steps are manual |

## Approach

### Library merge (done manually + rsync)

The Pi's library drive (`/dev/sdc1`, UUID `27cca5db-fbf5-420d-8abe-7c2ffb84d369` — the same
UUID as the udev rule in `piconfig/`) is physically attached to poochi and mounted read-only
at `/mnt/elsewer-src`. Library source is `/mnt/elsewer-src/AudioBooks` (matching `.env.pi`'s
`AUDIOBOOKS_LOCATION=/home/pi/drv/AudioBooks`).

Destination `/home/poochi/hdd/bookshelf/audiobooks` already held ~25 author dirs, so this is
a merge. Dry run showed **1,106 new files / 33G / zero overwrites** — the six source authors
(ChristopherRuocchio, JeffersonMays, JRRTolkien, Khenal, RebeccaYarros, RFKuang) are all new.

```sh
rsync -a --no-perms --no-owner --no-group --chmod=D755,F644 \
  /mnt/elsewer-src/AudioBooks/ /home/poochi/hdd/bookshelf/audiobooks/
```

`--no-owner/--no-group/--chmod` and never `--delete`: the tree is inside qbittorrent's
`/data` mount, so ownership and modes have to stay sane, and nothing on the destination may
be removed.

The drive also carries unrelated `backup/` (4.4G) and `docs/` (personal PDFs) directories —
left alone. The drive stays mounted as the fallback copy until verification passes; it is
never wiped by this work.

### Deploy shape

`deploy/poochi/` — see its README. Own compose project, own bridge network,
`traefik.enable=false`, `user: 1000:1000`, host port 3030, `./data` bind mount for
DB/creds/covers, `/home/poochi/hdd/bookshelf/audiobooks` bind-mounted rw. Built on poochi
from the pulled repo via the existing root `Dockerfile` (x86_64 native; symphonia/lofty are
pure Rust, so there are no external runtime deps).

Fresh DB — no migration of the Pi's listening progress, by choice. `ensure_admin_user`
creates `admin`/`admin`; change it at first login.

## Files

- `deploy/poochi/docker-compose.yml` — the stack
- `deploy/poochi/docker-compose.traefik.yml` — hostname overlay, not applied (see `15_…`)
- `deploy/poochi/.env.example`, `deploy/poochi/README.md`
- `piconfig/` and `deploy/https-duckdns/` stay untouched until poochi is proven, then a
  cleanup pass removes them.

## Open questions

- Whether `/home/poochi/hdd/bookshelf/{config,metadata,podcasts}` (Audiobookshelf-shaped
  leftovers) are live data. Ignored either way by this work.
- Whether the old library drive gets wiped and repurposed after verification.
- Whether the Pi gets decommissioned entirely or kept as a WireGuard endpoint.

## Verification

1. `docker compose ps` healthy; `curl -s http://192.168.1.18:3030/api/health`.
2. `docker compose logs -f` through the first scan; book count matches the merged tree.
3. LAN browser at `http://192.168.1.18:3030` — log in, change the admin password, library
   renders, covers load, a book streams and seeks.
4. Non-disruption: gluetun/qbittorrent/traefik/cloudflared/jellyfin all still up;
   `https://themizadah.com` still loads; qBittorrent WebUI still answers on
   `192.168.1.18:8080`; `mz-net`/`qbt_default` unchanged.
