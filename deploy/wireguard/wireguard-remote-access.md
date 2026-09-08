# WireGuard remote access

`deploy/https-duckdns/` gets the Pi a trusted HTTPS certificate for the
LAN, no port-forwarding involved. It's great at home, but it doesn't help
once you leave the house — the Caddy setup there deliberately only
resolves to the Pi's LAN IP.

This page covers the other half: reaching the Pi (and, optionally, the
rest of the home LAN) from an iPhone, an Android phone, or a laptop while
out and about, without putting else-wer, Caddy, or anything else directly
on the internet.

## The idea

Instead of forwarding else-wer's port (3000) or Caddy's port (443) through
the router — which would let anyone on the internet complete a connection
to those services and start probing — this setup forwards exactly one UDP
port to a [WireGuard](https://www.wireguard.com/) instance on the Pi.

WireGuard only responds to packets signed with a key it already trusts.
Everything else — port scanners, opportunistic bots, anyone poking at
random addresses — gets silence back. No handshake, no error, nothing to
even confirm the port is open. Once a device (your phone, your laptop)
authenticates over WireGuard, it gets a private tunnel IP and can reach the
Pi (and, if you chose that scope for it, the rest of the LAN) exactly as if
it were sitting on your home network.

A second DuckDNS subdomain, separate from the one `https-duckdns` uses,
tracks your home network's **public** IP so the WireGuard endpoint stays
reachable as your ISP-assigned address changes. The LAN-only subdomain and
Caddy setup are unaffected.

## What gets exposed, and what doesn't

After setup, from the open internet:

- **UDP 51820** (or whichever port you chose) is reachable — and answers
  only to already-trusted peers.
- **SSH (22), else-wer (3000), Caddy/HTTPS (443), DNS (53)** are firewalled
  to the LAN subnet and the tunnel subnet only. They're never reachable
  directly from the internet — only from LAN addresses, or, once
  connected, from the tunnel.

Only one router port-forward is needed: UDP 51820 (or your chosen port) to
the Pi's LAN IP. This is a manual step in your router's admin UI, done
once.

## Setup

Run once, on the Pi, as root:

```
sudo deploy/wireguard/setup.sh
```

This installs WireGuard, generates the server's keypair, sets up the new
WAN-tracking DuckDNS record, turns on the plumbing needed for routing
(disabled by default, only switched on if a client asks for it — see
below), and locks the firewall down so WireGuard is the only thing the
public internet can talk to.

Then, per device:

```
sudo deploy/wireguard/add-client.sh
```

## Pi-only vs. whole-LAN

Every device gets to choose its own scope when it's added — this isn't a
global setting:

- **Pi-only.** The device can reach the Pi over the tunnel and nothing
  else. This is the right default if all you want is else-wer or SSH
  access from your phone.
- **Whole-LAN.** The device routes all home-network traffic
  (`192.168.1.0/24`) through the Pi, so it can also reach other devices at
  home — a NAS, another server, a printer — as if it were physically
  there. The Pi does NAT for this, and it's switched on automatically the
  first time any client requests it.

You can mix scopes freely across devices — e.g. Pi-only on your phone,
whole-LAN on your laptop.

## DNS over cellular

If you also run `deploy/https-duckdns/` for a trusted-HTTPS LAN hostname,
`setup.sh` installs a small DNS resolver (dnsmasq, bound only to the tunnel
interface) that answers that hostname with the Pi's LAN IP directly, and
every client gets pointed at it (`DNS = 10.10.0.1` in its config). Without
this, phones on cellular data often fail to resolve the hostname at all —
carrier DNS resolvers commonly filter out a public name resolving to a
private address ("DNS rebinding" protection), which silently breaks the app
even though the tunnel itself is connected fine. Home WiFi doesn't hit this
since it resolves the hostname over the LAN directly, so it's easy to test
"working" on WiFi and still be broken on cellular — test off your home
network before considering this done.

## Connecting each device

**iPhone / Android** — install the official WireGuard app (App
Store / Play Store), then scan the QR code `add-client.sh` prints when you
add that device.

**Arch Linux laptop** — copy the generated config off the Pi and bring it
up with `wg-quick` (or NetworkManager's WireGuard integration, if you
prefer a GUI toggle):

```
scp pi@192.168.1.10:/etc/wireguard/clients/<name>/<name>.conf .
sudo install -m 600 <name>.conf /etc/wireguard/<name>.conf
sudo systemctl enable --now wg-quick@<name>
```

See `deploy/wireguard/README.md` for the full script reference, firewall
rule details, and how to remove a client.
