# HTTPS for else-wer with a free DuckDNS domain (no port forwarding)

iOS only registers a PWA's service worker on a **secure context** (HTTPS with a
trusted certificate). Without it, the app shell can't load offline at all —
downloads and offline playback need this, it's not optional polish.

This setup gets a real, trusted Let's Encrypt certificate **without owning a
domain, without port forwarding, and without exposing anything to the
internet**:

- [DuckDNS](https://www.duckdns.org) gives you a free `yourname.duckdns.org`
  subdomain, pointed at the server's **LAN IP** (private IPs are fine).
- Let's Encrypt validates via **DNS-01** (a TXT record set through the DuckDNS
  API), so it never needs to reach your server.
- Caddy (with the DuckDNS plugin) terminates TLS on port 443, auto-renews the
  cert, and reverse-proxies to the else-wer server on port 3000.

## Setup (once, ~5 minutes)
<!-- valvasban.duckdns.org token:
      82a72c4d-436e-4500-acbe-649d27e721b8-->
1. Sign in at https://www.duckdns.org (Google/GitHub login), create a
   subdomain, and copy your **token** from the top of the page.
2. On the server box:

   ```sh
   sudo ./setup.sh
   ```

   It asks for the subdomain, the token, and the upstream address
   (default `127.0.0.1:3000`), then installs and starts everything.
3. Watch the first certificate issue (~30–90 s):

   ```sh
   journalctl -u caddy -f
   ```
4. On the iPhone: open `https://yourname.duckdns.org`, log in, and
   **Add to Home Screen** again (the HTTPS origin is a new app as far as iOS
   is concerned — old HTTP-origin downloads don't carry over, re-download
   books once).

## What gets installed

- `/usr/local/bin/caddy` — Caddy built with `caddy-dns/duckdns`
- `/etc/caddy/Caddyfile`, `/etc/caddy/duckdns.env` (token, `chmod 600`)
- `caddy.service` — TLS + reverse proxy
- `duckdns-update.{service,timer}` — hourly refresh of the DNS record with the
  box's current LAN IP (use a DHCP reservation to make this a no-op)

## Caveats

- **DNS during an internet outage:** resolving `*.duckdns.org` needs public
  DNS. A previously installed PWA still cold-starts offline (the service
  worker serves the cached shell, downloaded books play from IndexedDB), but
  *streaming* from the server during an outage won't resolve. If you want
  LAN streaming to survive outages, add a local DNS override for the name on
  your router or Pi-hole pointing at the server's LAN IP.
- Renewal needs internet (runs automatically, certs last 90 days, Caddy renews
  at ~60).
- Keep the DuckDNS token private — anyone with it can repoint your subdomain.
