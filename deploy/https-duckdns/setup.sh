#!/usr/bin/env bash
# Trusted HTTPS for a LAN-only else-wer server, with no owned domain and no
# port forwarding: a free DuckDNS subdomain pointed at the box's LAN IP +
# a Let's Encrypt cert issued via DNS-01 (validated through the DuckDNS API,
# so nothing is ever exposed to the internet).
#
# Uses Caddy built with the caddy-dns/duckdns plugin: it issues the cert,
# renews it automatically, redirects HTTP->HTTPS, and reverse-proxies to the
# else-wer server. Run as root on the server box (Raspberry Pi OS / Debian /
# Arch all fine — only needs curl + systemd).
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
  echo "Run as root: sudo $0" >&2
  exit 1
fi

read -rp "DuckDNS subdomain (the part before .duckdns.org): " SUBDOMAIN
read -rp "DuckDNS token (shown at https://www.duckdns.org after login): " TOKEN
read -rp "else-wer server upstream [127.0.0.1:3000]: " UPSTREAM
UPSTREAM=${UPSTREAM:-127.0.0.1:3000}

case "$(uname -m)" in
  x86_64)  DL="os=linux&arch=amd64" ;;
  aarch64) DL="os=linux&arch=arm64" ;;
  armv7l)  DL="os=linux&arch=arm&arm=7" ;;
  armv6l)  DL="os=linux&arch=arm&arm=6" ;;
  *) echo "Unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

echo "==> Downloading Caddy with the DuckDNS DNS plugin"
curl -fsSL -o /usr/local/bin/caddy \
  "https://caddyserver.com/api/download?${DL}&p=github.com/caddy-dns/duckdns"
chmod 755 /usr/local/bin/caddy

echo "==> Creating caddy user and directories"
id -u caddy &>/dev/null || useradd --system --home /var/lib/caddy --create-home --shell /usr/sbin/nologin caddy
mkdir -p /etc/caddy /var/lib/caddy
chown caddy:caddy /var/lib/caddy

echo "==> Writing config"
cat > /etc/caddy/duckdns.env <<EOF
DUCKDNS_SUBDOMAIN=${SUBDOMAIN}
DUCKDNS_TOKEN=${TOKEN}
XDG_DATA_HOME=/var/lib/caddy
EOF
chmod 600 /etc/caddy/duckdns.env

cat > /etc/caddy/Caddyfile <<EOF
{\$DUCKDNS_SUBDOMAIN}.duckdns.org {
	tls {
		dns duckdns {\$DUCKDNS_TOKEN}
	}
	reverse_proxy ${UPSTREAM}
}
EOF

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
install -m 755 "${SCRIPT_DIR}/duckdns-update.sh" /usr/local/bin/duckdns-update

echo "==> Pointing ${SUBDOMAIN}.duckdns.org at this box's LAN IP"
/usr/local/bin/duckdns-update

echo "==> Installing systemd units"
cat > /etc/systemd/system/caddy.service <<'EOF'
[Unit]
Description=Caddy HTTPS reverse proxy for else-wer
After=network-online.target
Wants=network-online.target

[Service]
User=caddy
Group=caddy
EnvironmentFile=/etc/caddy/duckdns.env
ExecStart=/usr/local/bin/caddy run --config /etc/caddy/Caddyfile
ExecReload=/usr/local/bin/caddy reload --config /etc/caddy/Caddyfile --force
AmbientCapabilities=CAP_NET_BIND_SERVICE
NoNewPrivileges=true
Restart=on-failure

[Install]
WantedBy=multi-user.target
EOF

# Keeps the DuckDNS record tracking the box's LAN IP (DHCP can move it).
cat > /etc/systemd/system/duckdns-update.service <<'EOF'
[Unit]
Description=Update DuckDNS record with current LAN IP

[Service]
Type=oneshot
EnvironmentFile=/etc/caddy/duckdns.env
ExecStart=/usr/local/bin/duckdns-update
EOF

cat > /etc/systemd/system/duckdns-update.timer <<'EOF'
[Unit]
Description=Hourly DuckDNS LAN-IP refresh

[Timer]
OnBootSec=2min
OnUnitActiveSec=1h

[Install]
WantedBy=timers.target
EOF

systemctl daemon-reload
systemctl enable --now duckdns-update.timer caddy

echo
echo "Done. Caddy is requesting the certificate now (takes ~30-90s the first time)."
echo "Watch progress with:  journalctl -u caddy -f"
echo "Then open:            https://${SUBDOMAIN}.duckdns.org"
