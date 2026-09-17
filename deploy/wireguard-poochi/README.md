# WireGuard on poochi

The actual WireGuard setup running today, at `wg.syedyaseen.dev`. Unlike `../wireguard/`
(Pi-shaped, DuckDNS-based, retired — the Pi it targeted is gone), this one runs on poochi and
tracks its WAN IP via the Cloudflare API instead of DuckDNS, since `syedyaseen.dev` lives at
Cloudflare.

This directory documents what's live; it doesn't set it up from scratch (that was done ad hoc
directly on poochi before this was written back into the repo). Treat it as reference for
rebuilding the box, not a run-once installer.

## What's running

- `wg0` interface, `10.10.0.1/24`, listening on `51820/udp` (`wg-quick@wg0.service`, enabled).
- `wg0.conf` lives at `/etc/wireguard/wg0.conf` (not committed — has private keys). Structure:
  ```
  [Interface]
  Address = 10.10.0.1/24
  ListenPort = 51820
  # PostUp/PostDown for whole-LAN NAT are commented out unless a whole-LAN client exists.

  [Peer]
  # client: <name> (poochi-only | whole-LAN)
  PublicKey = ...
  AllowedIPs = 10.10.0.2/32   # one /32 per client, or 0.0.0.0/0 for whole-LAN
  ```
- Per-client configs + QR codes saved under `/etc/wireguard/clients/<name>/` (private keys —
  not committed). Re-scan the saved `.conf`/`.png` to reconnect a device instead of
  regenerating; only make a new peer if the client dir doesn't exist.
- `wg-wan-cloudflare-update.{service,timer}` (this dir has copies) — hourly, keeps the
  Cloudflare A record for `wg.syedyaseen.dev` pointed at poochi's current public IP.
  `EnvironmentFile=/etc/wireguard/wg-cloudflare.env` supplies `CF_ZONE_ID`, `CF_API_TOKEN`
  (scoped to the `syedyaseen.dev` zone, `Zone:DNS:Edit`), `WG_HOSTNAME=wg.syedyaseen.dev` — see
  `wg-cloudflare.env.example`.
- `dnsmasq`, bound to `wg0` only, overriding `ew.syedyaseen.dev` (and any other
  `*.syedyaseen.dev` host that should resolve to poochi's LAN IP rather than a carrier's
  filtered public-DNS view) to `192.168.1.18` for tunnel clients — see
  `/etc/dnsmasq.d/wireguard.conf` on poochi. Client configs point `DNS = 10.10.0.1` at it.
- `ufw`: `51820/udp` open to anyone; `22/tcp`, `3030/tcp` (else-wer), `443/tcp`, `53` open from
  `10.10.0.0/24` (the tunnel subnet) in addition to the LAN. Any other poochi-hosted service a
  tunnel client needs (e.g. Jellyfin `8096/tcp`) needs its own
  `ufw allow from 10.10.0.0/24 to any port <port> proto tcp` rule — ufw doesn't infer this from
  Docker's own port publishing.

## Known gotcha (2026-09-16 triage)

A client (peer already in `wg0.conf`) can go months with **zero handshakes** and look totally
fine from `wg0.conf`/`ufw`/`dnsmasq` alone — none of that tells you whether the tunnel ever
actually connects. The one thing that does: `sudo wg show wg0` — no `latest handshake:` /
`transfer:` line under a peer means no packet has ever arrived from it, full stop, regardless
of what ufw/dnsmasq/routing look like downstream. In this case the cause was a missing router
port-forward (`51820/udp` → poochi's LAN IP) — check that first if handshakes are absent.

## Adding a client

```
sudo ./add-client.sh
```

Ported from `../wireguard/add-client.sh` (Pi/DuckDNS-shaped) — same keygen / next-free-IP /
whole-LAN NAT-toggle logic, adapted for poochi: `Endpoint = wg.syedyaseen.dev` (fixed, no
DuckDNS env file), poochi's LAN IP (`192.168.1.18`), and the server pubkey read live via
`wg show wg0 public-key` instead of a `server_public.key` file (this tunnel wasn't originally
brought up by a `setup.sh` in this repo, so that file may not exist).

Prompts for a client name and scope (poochi-only vs whole-LAN, same tradeoff as the Pi
version), appends the peer, applies it live, and prints/saves a `.conf` + QR code under
`/etc/wireguard/clients/<name>/`.
