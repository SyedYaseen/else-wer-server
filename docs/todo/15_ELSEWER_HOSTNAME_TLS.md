Status: planned, not executed (2026-09-05)

Prerequisite: `14_POOCHI_MIGRATION.md` complete (else-wer running on poochi at
`http://192.168.1.18:3030`).

## Problem

else-wer should be reachable at `https://elsewer.syedyaseen.dev` — on the LAN now, and
through WireGuard when off-LAN later — instead of a bare IP and port. HTTPS is not
cosmetic here: the PWA's service worker, and therefore offline mode
(`10_OFFLINE_MODE.md`), only works on a secure origin.

The complication: poochi's Traefik already terminates TLS for `themizadah.com` using a
**Cloudflare** DNS-01 wildcard, but `syedyaseen.dev` is registered and DNS-hosted at
**Porkbun**. The existing `cfresolver` cannot issue a cert for it. This was already worked
out once for the Pi in `/home/loop/p/vw/docs/todo/multi-service-ingress.md` — same domain,
same registrar, different host.

## Approach

### 1. Porkbun, one-time (manual, their dashboard)

- Domain Management → `syedyaseen.dev` → enable **API Access** (off by default, per-domain).
- Account → API Access → generate an API key + secret key.
- Create an A record: host `elsewer`, type A → **`192.168.1.18`**.

A private IP in public DNS is intentional: the name resolves everywhere but only answers on
the LAN or through the tunnel. Nothing is exposed to the internet, and no split-horizon DNS
is needed — WireGuard peers resolve the same name to the same LAN IP over the tunnel.

A single-host record rather than a wildcard, since poochi's Traefik already answers for a
different domain on the same entrypoint; add another record per service later if wanted.

### 2. Traefik on poochi — add a resolver, change nothing existing

In `/home/poochi/mz/proxy/traefik/traefik.yaml`, add a **second** entry under
`certificatesResolvers` alongside `cfresolver`:

```yaml
certificatesResolvers:
  cfresolver:        # unchanged
    ...
  porkbun:
    acme:
      email: s.syedyaseen.s@gmail.com
      storage: /letsencrypt/acme.json
      dnsChallenge:
        provider: porkbun
        resolvers:
          - "1.1.1.1:53"
          - "1.0.0.1:53"
```

Add `PORKBUN_API_KEY` and `PORKBUN_SECRET_API_KEY` to `/home/poochi/mz/proxy/.env` (the
file Traefik already reads via `env_file`, currently holding `CF_DNS_ZONE`/`CF_DNS_API_TOKEN`).
Those are lego's exact variable names for the `porkbun` provider.

**Do not touch** the `websecure` entrypoint's default `certResolver: cfresolver` or its
`domains:` wildcard for `themizadah.com`. The else-wer router sets its own resolver at the
router level — which is exactly what the `mzfe` container already does today, despite the
comment in `traefik.yaml` warning against it.

Restart: `cd /home/poochi/mz/proxy && docker compose up -d` (a restart, not a recreate of
anything else). Watch `docker logs -f traefik` for the ACME order.

### 3. else-wer — apply the overlay

```sh
cd ~/projects/else-wer-server/deploy/poochi
docker compose -f docker-compose.yml -f docker-compose.traefik.yml up -d
```

`deploy/poochi/docker-compose.traefik.yml` is already written: it flips
`traefik.enable=true`, joins `mz-net` as a *second* network (its own project network stays),
pins `traefik.docker.network=mz-net` so Traefik dials the right interface, and routes
`Host(\`elsewer.syedyaseen.dev\`)` → container port 3000. Host port 3030 stays published, so
direct LAN access keeps working as a fallback.

### 4. WireGuard (separate follow-up)

`deploy/wireguard/` in this repo is Pi-shaped — it assumes the Pi is the endpoint, and
`add-client.sh` bakes `${WAN_DUCKDNS_SUBDOMAIN}.duckdns.org` into every client config. A
poochi variant needs:

- a WireGuard endpoint on poochi (or the Pi kept alive purely as the endpoint, routing to
  poochi — decide which)
- the endpoint hostname moved onto `syedyaseen.dev` (`wg.syedyaseen.dev` A → current WAN IP,
  refreshed by a timer against Porkbun's `retrieveByNameType`/`editByNameType` endpoints —
  the script design in `/home/loop/p/vw/docs/todo/multi-service-ingress.md` is directly reusable)
- a port forward on the router to whichever host ends up holding the endpoint

Not started. `deploy/wireguard/` should not be assumed to work as-is against poochi.

## Open questions

- Whether the Pi stays alive as the WireGuard endpoint or poochi takes that over too.
- Whether other services (Jellyfin, a future Vaultwarden) get the same treatment, which
  would argue for a `*.syedyaseen.dev` wildcard record and cert instead of one per host.
- Cert storage shares `acme.json` with `cfresolver` — fine (lego namespaces by resolver),
  but worth a backup of that file before the first Porkbun order.

## Verification

1. `docker logs -f traefik` during restart — the Porkbun DNS-01 order completes, and the
   existing `themizadah.com` wildcard is **not** re-ordered.
2. `dig +short elsewer.syedyaseen.dev` → `192.168.1.18`.
3. LAN browser → `https://elsewer.syedyaseen.dev` loads with a valid cert; the padlock shows
   a `syedyaseen.dev` cert, not the themizadah wildcard.
4. `https://themizadah.com` still loads with its own valid cert.
5. `http://192.168.1.18:3030` still works.
6. PWA installs and offline mode caches a book (the actual point of the HTTPS work).
