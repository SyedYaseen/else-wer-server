Status: planned, not executed (2026-09-06, revised)

Prerequisite: `14_POOCHI_MIGRATION.md` complete (else-wer running on poochi at
`http://192.168.1.18:3030`).

## Problem

else-wer should be reachable at `https://ew.syedyaseen.dev` — on the LAN now, and through
WireGuard when off-LAN later — instead of a bare IP and port. HTTPS is not cosmetic here: the
PWA's service worker, and therefore offline mode (`10_OFFLINE_MODE.md`), only works on a
secure origin.

`syedyaseen.dev` DNS now lives at **Cloudflare** (moved off Porkbun since this doc was first
written — the Porkbun-specific plan that used to live here is gone). Two other services need
a hostname on the same domain now that the Pi is being retired (poochi is the only always-on
box going forward): `vw.syedyaseen.dev` (Vaultwarden) and `jf.syedyaseen.dev` (Jellyfin,
which already runs natively on poochi per `14_POOCHI_MIGRATION.md`). That argues for a
`*.syedyaseen.dev` wildcard cert on poochi's Traefik rather than a single `ew`-only record —
one wildcard covers `ew`/`vw`/`jf` and whatever else lands on that domain later.

**The complication that actually matters**: poochi's Traefik has an existing `cfresolver`
that terminates TLS for `themizadah.com` — but checking the live config
(`/home/loop/p/mz/proxy/traefik/traefik.yaml`, a separate repo) shows `cfresolver` uses
`httpChallenge` (HTTP-01), not DNS-01. HTTP-01 cannot issue wildcard certs, so
`themizadah.com` gets ordinary per-host HTTP-01 certs, not a wildcard as this doc used to
claim. That means `cfresolver` cannot be reused for `*.syedyaseen.dev` either way — a new
DNS-01 resolver is required regardless of registrar.

**Crossover analysis (why a second wildcard resolver is safe to add)**: TLS serving is
SNI-based and HTTP routing is `Host()`-based, both keyed to the exact hostname requested.
Which resolver originally fetched a cert is irrelevant once it's in the shared cert store
(`acme.json`) — Traefik picks the cert matching the connection's SNI and the backend matching
the request's `Host` header. A new DNS-01 resolver for `*.syedyaseen.dev` coexisting with the
existing HTTP-01 `cfresolver` for `themizadah.com` is a normal multi-domain Traefik setup;
there is no mechanism by which one domain's cert or routing could bleed into the other. The
real requirements are just: keep the new resolver fully separate from `cfresolver` (don't
touch it), scope the new Cloudflare API token to only the `syedyaseen.dev` zone (least
privilege — it must not also have edit rights on `themizadah.com`'s zone), and back up
`acme.json` before the first order on the new resolver.

The Pi-hosted alternative that used to be prior art for this — a `*.syedyaseen.dev` wildcard
on a Pi-hosted Caddy fronting `ew`/`vw`/`jf`, in `/home/loop/p/vw/docs/todo/multi-service-ingress.md`
— is now superseded, since the Pi is being retired. That doc/`deploy/pi/Caddyfile` need their
own status update in that repo (cross-repo follow-up, not tracked here).

## Approach

### 1. Cloudflare DNS, one-time (dashboard)

- Create a wildcard A record: host `*`, type A → **`192.168.1.18`** (poochi), in the
  `syedyaseen.dev` zone.

A private LAN IP in public DNS is intentional: the name resolves everywhere but only answers
on the LAN or through WireGuard. Nothing is exposed to the internet, and no split-horizon DNS
is needed — WireGuard peers resolve the same name to the same LAN IP over the tunnel.

### 2. Traefik on poochi — add a resolver, change nothing existing

**This edit happens in `/home/loop/p/mz/proxy` (a separate repo), not here.** In
`traefik/traefik.yaml`, add a **new** entry under `certificatesResolvers` alongside the
existing `cfresolver` (do not modify `cfresolver` itself — it stays HTTP-01, serving
`themizadah.com`, untouched):

```yaml
certificatesResolvers:
  cfresolver:        # unchanged — themizadah.com, HTTP-01
    acme:
      email: s.syedyaseen.s@gmail.com
      storage: /letsencrypt/acme.json
      httpChallenge:
        entryPoint: web
  cfdns:              # new — syedyaseen.dev wildcard, DNS-01
    acme:
      email: s.syedyaseen.s@gmail.com
      storage: /letsencrypt/acme.json
      dnsChallenge:
        provider: cloudflare
        resolvers:
          - "1.1.1.1:53"
          - "1.0.0.1:53"
      domains:
        - main: "syedyaseen.dev"
          sans: ["*.syedyaseen.dev"]
```

The `domains:` block pre-fetches one wildcard cert at startup, shared by every router that
sets `tls.certresolver=cfdns` — no per-service DNS-01 order needed.

Add a Cloudflare API token to poochi's `.env` (the file Traefik reads via `env_file`), scoped
to **only** the `syedyaseen.dev` zone (`Zone:DNS:Edit`) — exact env var name(s) depend on the
lego provider version bundled in the `traefik:latest` image (`CF_DNS_API_TOKEN` alone, or a
`CF_ZONE_API_TOKEN`+`CF_DNS_API_TOKEN` split); check when applying.

**Do not touch** `cfresolver`, the `websecure` entrypoint's default resolver, or any
`themizadah.com` router config.

Restart: `cd /home/poochi/mz/proxy && docker compose up -d` (a restart, not a recreate of
anything else). Watch `docker logs -f traefik` for the ACME order.

### 3. else-wer — apply the overlay

```sh
cd ~/projects/else-wer-server/deploy/poochi
docker compose -f docker-compose.yml -f docker-compose.traefik.yml up -d
```

`deploy/poochi/docker-compose.traefik.yml` flips `traefik.enable=true`, joins `mz-net` as a
*second* network (its own project network stays), pins `traefik.docker.network=mz-net` so
Traefik dials the right interface, and routes `Host(\`ew.syedyaseen.dev\`)` → container port
3000, with `tls.certresolver=cfdns`. Host port 3030 stays published, so direct LAN access
keeps working as a fallback.

### 4. WireGuard (separate follow-up)

`deploy/wireguard/` in this repo is Pi-shaped — it assumes the Pi is the endpoint, and
`add-client.sh` bakes `${WAN_DUCKDNS_SUBDOMAIN}.duckdns.org` into every client config. Since
the Pi is being retired, the endpoint moves to poochi (this closes the "Pi vs. poochi"
question the previous version of this doc left open). A poochi variant still needs:

- a WireGuard endpoint on poochi
- the endpoint hostname moved onto `syedyaseen.dev` (e.g. `wg.syedyaseen.dev` A → current WAN
  IP), refreshed by a timer against Cloudflare's API when the WAN IP changes
- a port forward on the router to poochi

Not started. `deploy/wireguard/` should not be assumed to work as-is against poochi.

## Open questions

- Exact Cloudflare API token env var name(s) for the `cfdns` resolver — depends on the lego
  provider version in the `traefik:latest` image; confirm when applying in the mz/proxy repo.
- Where Vaultwarden (`vw`) ends up running now that the Pi is retiring — that host decision
  belongs to `/home/loop/p/vw`, not else-wer, but its router will use the same `cfdns`
  wildcard once decided.
- `/home/loop/p/vw/docs/todo/multi-service-ingress.md` and `deploy/pi/Caddyfile` need a
  status update marking the Pi+Caddy wildcard approach retired — cross-repo follow-up.

## Verification

1. `docker logs -f traefik` on poochi during restart — the `cfdns` DNS-01 order for
   `*.syedyaseen.dev` completes, and `themizadah.com`'s existing HTTP-01 certs are **not**
   re-ordered or disturbed.
2. `dig +short ew.syedyaseen.dev` → `192.168.1.18`.
3. LAN browser → `https://ew.syedyaseen.dev` loads with a valid cert; the padlock shows a
   `syedyaseen.dev` wildcard cert, not a themizadah cert.
4. `https://themizadah.com` still loads with its own valid cert, unaffected.
5. `http://192.168.1.18:3030` still works.
6. PWA installs from `https://ew.syedyaseen.dev` and offline mode caches a book (the actual
   point of the HTTPS work — see `10_OFFLINE_MODE.md`).
