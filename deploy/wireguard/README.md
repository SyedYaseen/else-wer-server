# WireGuard remote access

Real remote access to the Pi (and, if you want, the rest of the LAN) from
outside the house — iPhone, Android, Arch laptop — without exposing
else-wer's port (3000), Caddy's port (443), or anything else directly to
the internet.

## Why WireGuard instead of just forwarding a port

Port-forwarding 3000 or 443 puts a service in front of anyone on the
internet who finds it: it completes TCP handshakes, answers HTTP requests,
shows up in scans. WireGuard is different by design — it's a UDP protocol
that only replies to a packet if it's cryptographically signed by a key it
already knows about. An unauthenticated scanner sends a packet and gets
*nothing back*, not even a "connection refused." The Pi looks offline to
everyone except your own devices.

This folder adds a second, separate DuckDNS subdomain that tracks your
home network's **public** IP (the existing `deploy/https-duckdns/` setup
and its subdomain, which tracks the Pi's **LAN** IP, are untouched — that
one keeps serving the LAN as before).

## What's here

| File | Purpose |
|---|---|
| `setup.sh` | Run once, as root, on the Pi. Installs WireGuard, creates the server keypair and `wg0.conf`, sets up the WAN-tracking DuckDNS subdomain, enables IP forwarding, locks down the firewall. |
| `wan-duckdns-update.sh` | Installed to `/usr/local/bin/`. Keeps the WAN DuckDNS record pointed at your current public IP. Run hourly by a systemd timer. |
| `add-client.sh` | Run once per device (phone, laptop, ...). Generates a client keypair, appends a `[Peer]` block to `wg0.conf`, and prints/saves a config + QR code for that device. |

## Setup

```
sudo ./setup.sh
```

You'll be asked for:
- **A new DuckDNS subdomain + token** — pick a subdomain you're not already
  using for the LAN-only site (same DuckDNS account/token is fine, just a
  different name). This one will resolve to your home's public IP.
- **Your LAN HTTPS DuckDNS subdomain, if you have one** (from
  `deploy/https-duckdns/`) — lets `setup.sh` configure a DNS override so
  tunnel clients resolve it correctly over cellular data (see "DNS over
  cellular" below). Leave blank if you don't run that setup.
- **WireGuard ListenPort** — default `51820` is fine. WireGuard drops
  unauthenticated packets on any port, so a non-default port doesn't add
  real security — it just cuts down on automated scan noise hitting your
  logs.

At the end, `setup.sh` prints your server's public key and reminds you to:

1. **Port-forward UDP `51820`** (or whatever port you chose) on your router
   to the Pi's LAN IP (`192.168.1.10`). This is the *only* port forward this
   setup needs — everything else stays LAN/tunnel-only. You said you'd add
   this yourself since you have router admin access.
2. Run `add-client.sh` for each device.

## Adding a device

```
sudo ./add-client.sh
```

You'll be asked for a name (`iphone`, `android`, `arch-laptop`, ...) and a
scope, chosen **per client**:

- **Pi-only** — the device can only reach this box over the tunnel. This is
  what you want for "let me hit else-wer / SSH into the Pi from my phone
  while I'm out."
- **whole-LAN** — the device routes all `192.168.1.0/24` traffic through
  the Pi, so it can also reach other things on your home network (a NAS,
  another box, a printer). The Pi does NAT for this; `add-client.sh` turns
  that on automatically the first time any client asks for it (uncommenting
  the NAT rules in `wg0.conf`, and flipping ufw's forward policy from its
  default `DROP` to `ACCEPT`, since otherwise ufw silently drops the
  routed traffic even with NAT in place). Existing Pi-only clients aren't
  affected either way, and if you never add a whole-LAN client this never
  happens — ufw's default-deny posture stays untouched.

Mix and match freely — your phone can be Pi-only while your laptop is
whole-LAN, for example.

**iPhone / Android:** install the official WireGuard app, then scan the QR
code `add-client.sh` prints in the terminal (also saved as a PNG at
`/etc/wireguard/clients/<name>/<name>.png`).

**Arch Linux laptop:** copy the generated `.conf` off the Pi and bring the
tunnel up with `wg-quick` (or NetworkManager's WireGuard support, or the
`wireguard-tools`/`networkmanager` combo — whichever you already use):

```
scp pi@192.168.1.10:/etc/wireguard/clients/arch-laptop/arch-laptop.conf .
sudo install -m 600 arch-laptop.conf /etc/wireguard/arch-laptop.conf
sudo systemctl enable --now wg-quick@arch-laptop
```

## DNS over cellular (why it matters)

If you also run `deploy/https-duckdns/` for a trusted-HTTPS LAN hostname
(e.g. `yourapp.duckdns.org` → the Pi's LAN IP), tunnel clients need that
hostname to resolve correctly while connected. On home WiFi it just works
because you're on the LAN directly. On cellular data it often silently
breaks: many carrier DNS resolvers filter out "DNS rebinding" responses — a
public hostname resolving to a private (RFC1918) address, which is exactly
what that LAN hostname looks like from the outside. The request never even
reaches the Pi; the app just goes into offline mode.

`setup.sh` fixes this by installing `dnsmasq`, bound only to the `wg0`
interface, with a static override for the LAN hostname (if you gave it one)
and everything else forwarded to a public resolver. `add-client.sh` points
every client at it with `DNS = 10.10.0.1`, so tunnel clients ask the Pi's
resolver instead of the carrier's.

## What the firewall looks like afterwards

`setup.sh` configures `ufw` so that, from the public internet, the *only*
thing that ever gets a response is WireGuard itself:

- `51820/udp` (or your chosen port) — open to anyone (this is the point).
- `22/tcp` (SSH), `3000/tcp` (else-wer), `443/tcp` (Caddy/HTTPS) — open
  from `192.168.1.0/24` (the LAN) **and** `10.10.0.0/24` (the tunnel
  subnet). Both are needed: a device on home WiFi shows up with a LAN
  source address, but a connected WireGuard client's decrypted traffic
  shows up with a *tunnel* source address instead, even when it's headed
  for the Pi's own LAN IP — so the tunnel subnet needs its own rule, not
  just the LAN one.
- `53/tcp+udp` — open only from `10.10.0.0/24`, for the dnsmasq override
  above.

## Removing a client

There's no `remove-client.sh` here — it's a two-line manual job if you ever
need it: delete the matching `[Peer]` block from `/etc/wireguard/wg0.conf`,
then `sudo wg syncconf wg0 <(sudo wg-quick strip wg0)` to apply it live
(no restart needed).
