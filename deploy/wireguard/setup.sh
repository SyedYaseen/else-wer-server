#!/usr/bin/env bash
# Real remote access to this box from outside the LAN, without exposing
# else-wer (or anything else) to the internet directly.
#
# WireGuard listens on one UDP port and answers ONLY to peers that already
# hold a valid cryptographic key. Unauthenticated traffic — port scans,
# random internet noise, whatever — gets no response at all: not a TCP
# RST, not an ICMP error, nothing. That's a materially different exposure
# than forwarding 3000 or even 443 straight through, where any listener
# on the internet can at least complete a handshake and start probing.
#
# This script only sets up the tunnel + server-side plumbing:
#   - installs wireguard-tools, qrencode, ufw, iptables
#   - creates the server keypair and /etc/wireguard/wg0.conf (no peers yet)
#   - stands up a NEW duckdns subdomain that tracks this network's WAN
#     (public) IP -- separate from any existing LAN-IP-tracking subdomain,
#     so deploy/https-duckdns/ is untouched
#   - turns on IP forwarding and leaves (commented) NAT rules ready for
#     the whole-LAN tunnel option
#   - locks the box down with ufw so WireGuard is the only thing the
#     public internet can ever get a response from
#
# Run add-client.sh afterwards to actually add a phone/laptop.
#
# Run as root on the Pi (Raspberry Pi OS / Debian; uses apt).
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
  echo "Run as root: sudo $0" >&2
  exit 1
fi

WG_DIR=/etc/wireguard
WG_CONF="${WG_DIR}/wg0.conf"

echo "==> Installing wireguard-tools, qrencode, ufw, dnsmasq"
# wireguard-tools, not the wireguard metapackage: current Debian / Raspberry
# Pi OS kernels (6.x, mainline since 5.6) already ship the WireGuard kernel
# module, so the metapackage's only remaining job is pulling in
# wireguard-tools anyway. Installing it directly skips the redundant
# transitional package.
apt-get update -y
apt-get install -y wireguard-tools qrencode ufw iptables dnsmasq

echo
echo "This is a NEW DuckDNS subdomain, separate from any subdomain already"
echo "used for LAN-only access -- it will track this network's public"
echo "(WAN) IP, not this box's LAN IP, so the existing LAN-only Caddy setup"
echo "is left alone."
read -rp "New WAN DuckDNS subdomain (the part before .duckdns.org): " WAN_SUBDOMAIN
read -rp "DuckDNS token (same account, shown at https://www.duckdns.org after login): " WAN_TOKEN

echo
echo "If you also run deploy/https-duckdns/ (a LAN-only HTTPS hostname for"
echo "else-wer itself, e.g. yourapp.duckdns.org -> this box's LAN IP), tunnel"
echo "clients need to resolve that hostname while connected. Cellular carrier"
echo "DNS resolvers commonly block/null out a public hostname that resolves to"
echo "a private address (\"DNS rebinding\" protection) -- that silently breaks"
echo "the app over mobile data even though the tunnel itself is up. Leave this"
echo "blank to skip if you don't have that LAN HTTPS setup."
read -rp "LAN HTTPS DuckDNS subdomain, if any (the part before .duckdns.org): " LAN_SUBDOMAIN

echo
echo "WireGuard ListenPort. Default 51820 is fine -- WireGuard silently"
echo "drops unauthenticated packets on any port, so this is NOT a real"
echo "security boundary. Picking a non-default port is just a cheap way to"
echo "cut down on automated internet background-scan log noise."
read -rp "WireGuard ListenPort [51820]: " LISTEN_PORT
LISTEN_PORT=${LISTEN_PORT:-51820}

echo "==> Generating server keypair"
umask 077
mkdir -p "${WG_DIR}"
wg genkey | tee "${WG_DIR}/server_private.key" | wg pubkey > "${WG_DIR}/server_public.key"
SERVER_PRIVKEY=$(cat "${WG_DIR}/server_private.key")
SERVER_PUBKEY=$(cat "${WG_DIR}/server_public.key")

echo "==> Detecting outbound interface (for the whole-LAN NAT option)"
OUT_IF=$(ip route show default | awk '{print $5; exit}')
if [[ -z "${OUT_IF}" ]]; then
  echo "Could not auto-detect the default outbound interface." >&2
  echo "Edit the PostUp/PostDown lines in ${WG_CONF} by hand later." >&2
  OUT_IF="eth0"
fi
echo "    using ${OUT_IF}"
PI_LAN_IP=$(ip -4 addr show "${OUT_IF}" | awk '/inet /{print $2}' | cut -d/ -f1 | head -1)

echo "==> Writing ${WG_CONF}"
cat > "${WG_CONF}" <<EOF
[Interface]
Address = 10.10.0.1/24
ListenPort = ${LISTEN_PORT}
PrivateKey = ${SERVER_PRIVKEY}

# Whole-LAN routing for clients that want it. Commented out by default --
# add-client.sh uncomments this automatically the first time any client
# is added with whole-LAN scope, then restarts wg0. Pi-only clients never
# need this; the tunnel works without it.
#PostUp   = iptables -A FORWARD -i wg0 -j ACCEPT; iptables -t nat -A POSTROUTING -o ${OUT_IF} -j MASQUERADE
#PostDown = iptables -D FORWARD -i wg0 -j ACCEPT; iptables -t nat -D POSTROUTING -o ${OUT_IF} -j MASQUERADE

# [Peer] blocks are appended below by add-client.sh. Don't hand-edit below
# this line -- add-client.sh parses this file to pick the next free
# tunnel IP.
EOF
chmod 600 "${WG_CONF}"

echo "==> Writing ${WG_DIR}/wan-duckdns.env"
cat > "${WG_DIR}/wan-duckdns.env" <<EOF
WAN_DUCKDNS_SUBDOMAIN=${WAN_SUBDOMAIN}
WAN_DUCKDNS_TOKEN=${WAN_TOKEN}
EOF
chmod 600 "${WG_DIR}/wan-duckdns.env"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
install -m 755 "${SCRIPT_DIR}/wan-duckdns-update.sh" /usr/local/bin/wan-duckdns-update

echo "==> Pointing ${WAN_SUBDOMAIN}.duckdns.org at this network's WAN IP"
# The systemd service (installed below) supplies WAN_DUCKDNS_SUBDOMAIN /
# WAN_DUCKDNS_TOKEN via EnvironmentFile=. This first run happens before
# that service exists and isn't going through systemd at all, so those
# vars have to be passed in explicitly here.
WAN_DUCKDNS_SUBDOMAIN="${WAN_SUBDOMAIN}" WAN_DUCKDNS_TOKEN="${WAN_TOKEN}" /usr/local/bin/wan-duckdns-update

echo "==> Installing systemd units for the WAN DuckDNS refresh"
cat > /etc/systemd/system/wg-wan-duckdns-update.service <<'EOF'
[Unit]
Description=Update WAN DuckDNS record with current public IP (WireGuard endpoint)
[Service]
Type=oneshot
EnvironmentFile=/etc/wireguard/wan-duckdns.env
ExecStart=/usr/local/bin/wan-duckdns-update
EOF
cat > /etc/systemd/system/wg-wan-duckdns-update.timer <<'EOF'
[Unit]
Description=Hourly WAN DuckDNS refresh for the WireGuard endpoint
[Timer]
OnBootSec=2min
OnUnitActiveSec=1h
[Install]
WantedBy=timers.target
EOF

echo "==> Enabling IP forwarding"
sysctl -w net.ipv4.ip_forward=1 >/dev/null
cat > /etc/sysctl.d/99-wireguard.conf <<'EOF'
net.ipv4.ip_forward = 1
EOF

echo "==> Writing DNS override for tunnel clients (/etc/dnsmasq.d/wireguard.conf)"
# Bound to wg0 only (not the LAN) -- tunnel clients get DNS = 10.10.0.1 in
# their config (add-client.sh) and ask this instead of the carrier's
# resolver, which sidesteps carrier DNS-rebinding filters (see the prompt
# above). Everything except the LAN HTTPS hostname forwards to a public
# resolver.
{
  echo "interface=wg0"
  echo "bind-interfaces"
  echo "no-dhcp-interface=wg0"
  echo "no-resolv"
  echo "server=1.1.1.1"
  echo "server=1.0.0.1"
  if [[ -n "${LAN_SUBDOMAIN}" ]]; then
    echo "address=/${LAN_SUBDOMAIN}.duckdns.org/${PI_LAN_IP}"
  fi
} > /etc/dnsmasq.d/wireguard.conf
systemctl enable --now dnsmasq
systemctl restart dnsmasq

echo "==> Starting WireGuard"
systemctl daemon-reload
systemctl enable --now wg-wan-duckdns-update.timer
systemctl enable --now wg-quick@wg0

echo "==> Locking down the firewall (ufw)"
# The box is now internet-reachable on one UDP port. Make WireGuard the
# only thing that can ever answer the public internet; everything else
# (including SSH) stays LAN/tunnel-only.
#
# Both the LAN subnet (for devices on the home WiFi) and the tunnel subnet
# (10.10.0.0/24, for connected WireGuard clients -- their decrypted traffic
# arrives with a 10.10.0.x source, not a 192.168.1.x one, so it needs its
# own rule) get access to SSH/else-wer/Caddy. Port 53 is only ever reachable
# via the tunnel subnet since dnsmasq above is bound to wg0.
ufw default deny incoming
ufw default allow outgoing
ufw allow "${LISTEN_PORT}/udp"
for SRC in 192.168.1.0/24 10.10.0.0/24; do
  ufw allow from "${SRC}" to any port 22 proto tcp
  ufw allow from "${SRC}" to any port 3000 proto tcp
  ufw allow from "${SRC}" to any port 443 proto tcp
done
ufw allow from 10.10.0.0/24 to any port 53
ufw --force enable

echo
echo "Done."
echo "Server public key: ${SERVER_PUBKEY}"
echo
echo "TWO THINGS LEFT:"
echo "  1. On your router, port-forward UDP ${LISTEN_PORT} to this Pi's LAN IP"
echo "     (192.168.1.10). This is the one port the internet can reach."
echo "  2. Run add-client.sh to add your iPhone, Android phone, and laptop."
