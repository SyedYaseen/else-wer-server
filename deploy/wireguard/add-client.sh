#!/usr/bin/env bash
# Adds one WireGuard client (iPhone, Android phone, Arch laptop, whatever)
# to the tunnel set up by setup.sh. Run once per device.
#
# Each client picks its own tunnel scope at add-client time:
#   Pi-only    - the client can only reach this box (10.10.0.1) over the
#                tunnel. Nothing else on the LAN is exposed. Right choice
#                for "I just need else-wer/SSH from my phone."
#   whole-LAN  - the client routes all LAN traffic (192.168.1.0/24) through
#                this box, so it can reach other LAN devices too (printers,
#                NAS, other boxes). Requires NAT on the Pi; this script
#                turns that on automatically the first time it's needed.
#
# Run as root on the Pi, after setup.sh.
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
  echo "Run as root: sudo $0" >&2
  exit 1
fi

WG_CONF=/etc/wireguard/wg0.conf
CLIENTS_DIR=/etc/wireguard/clients
ENV_FILE=/etc/wireguard/wan-duckdns.env
PI_LAN_IP=192.168.1.10

if [[ ! -f "${WG_CONF}" ]]; then
  echo "No ${WG_CONF} found -- run setup.sh first." >&2
  exit 1
fi

read -rp "Client name (letters/numbers/dashes, e.g. iphone, android, arch-laptop): " CLIENT_NAME
CLIENT_NAME=$(echo "${CLIENT_NAME}" | tr -cd 'A-Za-z0-9_-')
if [[ -z "${CLIENT_NAME}" ]]; then
  echo "Empty/invalid client name." >&2
  exit 1
fi
if grep -q "# client: ${CLIENT_NAME} " "${WG_CONF}"; then
  echo "A client named '${CLIENT_NAME}' already exists in ${WG_CONF}." >&2
  exit 1
fi

echo
echo "Tunnel scope for ${CLIENT_NAME}:"
echo "  1) Pi-only   -- this client can reach this box (over the tunnel address"
echo "                  AND its real LAN IP, so the https-duckdns site works too)"
echo "  2) whole-LAN -- this client routes all LAN traffic through this box"
read -rp "Choice [1]: " SCOPE_CHOICE
SCOPE_CHOICE=${SCOPE_CHOICE:-1}
case "${SCOPE_CHOICE}" in
  # Pi-only includes both the WireGuard tunnel address (10.10.0.1) AND the
  # Pi's real LAN IP. The https-duckdns hostname publicly resolves to the
  # LAN IP, not the tunnel address -- without that /32 route, a client
  # resolving that hostname has nowhere to send the traffic and it never
  # reaches the tunnel at all.
  1) SCOPE="pi-only";   CLIENT_ALLOWED_IPS="10.10.0.1/32,${PI_LAN_IP}/32" ;;
  2) SCOPE="whole-lan"; CLIENT_ALLOWED_IPS="10.10.0.1/32,192.168.1.0/24" ;;
  *) echo "Invalid choice." >&2; exit 1 ;;
esac

echo "==> Picking a free tunnel IP"
USED_OCTETS=$(grep -oP '(?<=AllowedIPs = 10\.10\.0\.)[0-9]+(?=/32)' "${WG_CONF}" || true)
NEXT=2
while echo "${USED_OCTETS}" | grep -qx "${NEXT}"; do
  NEXT=$((NEXT + 1))
done
CLIENT_IP="10.10.0.${NEXT}"
echo "    ${CLIENT_IP}"

echo "==> Generating client keypair"
CLIENT_DIR="${CLIENTS_DIR}/${CLIENT_NAME}"
umask 077
mkdir -p "${CLIENT_DIR}"
wg genkey | tee "${CLIENT_DIR}/privatekey" | wg pubkey > "${CLIENT_DIR}/publickey"
CLIENT_PRIVKEY=$(cat "${CLIENT_DIR}/privatekey")
CLIENT_PUBKEY=$(cat "${CLIENT_DIR}/publickey")

echo "==> Appending peer to ${WG_CONF}"
cat >> "${WG_CONF}" <<EOF

[Peer]
# client: ${CLIENT_NAME} (${SCOPE})
PublicKey = ${CLIENT_PUBKEY}
AllowedIPs = ${CLIENT_IP}/32
EOF

NEED_RESTART=0
if [[ "${SCOPE}" == "whole-lan" ]] && grep -q '^#PostUp' "${WG_CONF}"; then
  echo "==> First whole-LAN client -- enabling NAT (PostUp/PostDown) on wg0"
  sed -i 's/^#PostUp/PostUp/; s/^#PostDown/PostDown/' "${WG_CONF}"
  NEED_RESTART=1

  # ufw ships with its own FORWARD-chain default policy (DROP), tracked
  # separately from "ufw default allow/deny incoming/outgoing" and from
  # wg-quick's PostUp/PostDown rules above. Left at DROP, it silently
  # swallows the *return* traffic for whole-LAN clients even though NAT
  # and the PostUp ACCEPT rule are both in place -- symptom is "the tunnel
  # connects but I can't reach anything on the LAN." Flip it to ACCEPT.
  # Only touched here, once, the first time whole-LAN is actually used --
  # Pi-only-only setups never hit the FORWARD chain and keep ufw's
  # default-deny posture untouched.
  echo "==> Setting ufw's forward policy to ACCEPT (needed to route whole-LAN traffic)"
  sed -i 's/^DEFAULT_FORWARD_POLICY=.*/DEFAULT_FORWARD_POLICY="ACCEPT"/' /etc/default/ufw
  ufw reload
fi

echo "==> Applying config to the running interface"
if [[ "${NEED_RESTART}" -eq 1 ]]; then
  echo "    (restarting wg0 to pick up the new NAT rules -- brief blip for any"
  echo "     already-connected peers)"
  systemctl restart wg-quick@wg0
else
  wg syncconf wg0 <(wg-quick strip wg0)
fi

SERVER_PUBKEY=$(cat /etc/wireguard/server_public.key)
LISTEN_PORT=$(awk -F'= ' '/^ListenPort/{print $2; exit}' "${WG_CONF}")
WAN_SUBDOMAIN=$(grep '^WAN_DUCKDNS_SUBDOMAIN=' "${ENV_FILE}" | cut -d= -f2)

echo "==> Writing client config"
CLIENT_CONF="${CLIENT_DIR}/${CLIENT_NAME}.conf"
cat > "${CLIENT_CONF}" <<EOF
[Interface]
PrivateKey = ${CLIENT_PRIVKEY}
Address = ${CLIENT_IP}/32
# Routes DNS through the Pi's dnsmasq (set up by setup.sh) instead of
# whatever resolver the client's network hands out. Needed so the LAN
# HTTPS hostname resolves correctly over cellular data, where carrier
# resolvers commonly filter out a public name -> private IP answer.
DNS = 10.10.0.1

[Peer]
PublicKey = ${SERVER_PUBKEY}
Endpoint = ${WAN_SUBDOMAIN}.duckdns.org:${LISTEN_PORT}
AllowedIPs = ${CLIENT_ALLOWED_IPS}
PersistentKeepalive = 25
EOF
chmod 600 "${CLIENT_CONF}"
qrencode -o "${CLIENT_DIR}/${CLIENT_NAME}.png" < "${CLIENT_CONF}"

echo
echo "Done. Client '${CLIENT_NAME}' added with ${SCOPE} scope (${CLIENT_IP})."
echo
echo "For iPhone / Android: open the WireGuard app -> Add tunnel -> Scan QR,"
echo "and scan this:"
echo
qrencode -t ansiutf8 < "${CLIENT_CONF}"
echo
echo "For the Arch laptop (or any manual import), copy the config off the Pi:"
echo "  scp pi@192.168.1.10:${CLIENT_CONF} ."
echo "  sudo install -m 600 ${CLIENT_NAME}.conf /etc/wireguard/${CLIENT_NAME}.conf"
echo "  sudo systemctl enable --now wg-quick@${CLIENT_NAME}"
echo
echo "A PNG of the QR code is also saved at: ${CLIENT_DIR}/${CLIENT_NAME}.png"
