#!/usr/bin/env bash
# Adds one WireGuard client (iPhone, Android phone, Arch laptop, whatever)
# to poochi's existing tunnel. Run once per device.
#
# Ported from ../wireguard/add-client.sh (written for the retired Pi + DuckDNS).
# Differences: poochi's LAN IP, wg.syedyaseen.dev as a fixed Cloudflare-tracked
# endpoint instead of a DuckDNS env file, and the server pubkey is read live via
# `wg show` instead of a possibly-missing /etc/wireguard/server_public.key (this
# tunnel wasn't originally brought up by a setup.sh in this repo).
#
# Each client picks its own tunnel scope at add-client time:
#   poochi-only - the client can only reach this box (10.10.0.1, and its real
#                 LAN IP 192.168.1.18) over the tunnel. Right choice for
#                 "I just need else-wer/Jellyfin/SSH from my phone."
#   whole-LAN   - the client routes all LAN traffic (192.168.1.0/24) through
#                 this box, so it can reach other LAN devices too (printers,
#                 NAS, other boxes). Requires NAT on poochi; this script turns
#                 that on automatically the first time it's needed.
#
# Run as root on poochi.
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
  echo "Run as root: sudo $0" >&2
  exit 1
fi

WG_CONF=/etc/wireguard/wg0.conf
CLIENTS_DIR=/etc/wireguard/clients
POOCHI_LAN_IP=192.168.1.18
WG_HOSTNAME=wg.syedyaseen.dev

if [[ ! -f "${WG_CONF}" ]]; then
  echo "No ${WG_CONF} found." >&2
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
echo "  1) poochi-only -- this client can reach this box (over the tunnel address"
echo "                    AND its real LAN IP, so ew/jf.syedyaseen.dev work too)"
echo "  2) whole-LAN   -- this client routes all LAN traffic through this box"
read -rp "Choice [1]: " SCOPE_CHOICE
SCOPE_CHOICE=${SCOPE_CHOICE:-1}
case "${SCOPE_CHOICE}" in
  # poochi-only includes both the WireGuard tunnel address (10.10.0.1) AND
  # poochi's real LAN IP. ew.syedyaseen.dev/jf.syedyaseen.dev publicly resolve
  # to the LAN IP, not the tunnel address -- without that /32 route, a client
  # resolving those hostnames has nowhere to send the traffic and it never
  # reaches the tunnel at all.
  1) SCOPE="poochi-only"; CLIENT_ALLOWED_IPS="10.10.0.1/32,${POOCHI_LAN_IP}/32" ;;
  2) SCOPE="whole-lan";   CLIENT_ALLOWED_IPS="10.10.0.1/32,192.168.1.0/24" ;;
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
  # poochi-only-only setups never hit the FORWARD chain and keep ufw's
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

SERVER_PUBKEY=$(wg show wg0 public-key)
LISTEN_PORT=$(awk -F'= ' '/^ListenPort/{print $2; exit}' "${WG_CONF}")

echo "==> Writing client config"
CLIENT_CONF="${CLIENT_DIR}/${CLIENT_NAME}.conf"
cat > "${CLIENT_CONF}" <<EOF
[Interface]
PrivateKey = ${CLIENT_PRIVKEY}
Address = ${CLIENT_IP}/32
# Routes DNS through poochi's dnsmasq (bound to wg0) instead of whatever
# resolver the client's network hands out. Needed so ew/jf.syedyaseen.dev
# resolve correctly over cellular data, where carrier resolvers commonly
# filter out a public name -> private IP answer.
DNS = 10.10.0.1

[Peer]
PublicKey = ${SERVER_PUBKEY}
Endpoint = ${WG_HOSTNAME}:${LISTEN_PORT}
AllowedIPs = ${CLIENT_ALLOWED_IPS}
PersistentKeepalive = 25
EOF
chmod 600 "${CLIENT_CONF}"
qrencode -o "${CLIENT_DIR}/${CLIENT_NAME}.png" < "${CLIENT_CONF}"

# /etc/wireguard is root-only (700), so a non-root scp can never read CLIENT_CONF
# in place. Drop a user-owned copy in the invoking user's home for the transfer.
COPY_USER="${SUDO_USER:-root}"
COPY_PATH="$(getent passwd "${COPY_USER}" | cut -d: -f6)/${CLIENT_NAME}.conf"
install -m 600 -o "${COPY_USER}" -g "$(id -gn "${COPY_USER}")" "${CLIENT_CONF}" "${COPY_PATH}"

echo
echo "Done. Client '${CLIENT_NAME}' added with ${SCOPE} scope (${CLIENT_IP})."
echo
echo "For iPhone / Android: open the WireGuard app -> Add tunnel -> Scan QR,"
echo "and scan this:"
echo
qrencode -t ansiutf8 < "${CLIENT_CONF}"
echo
echo "For a laptop (or any manual import), run on the laptop:"
echo "  scp ${COPY_USER}@192.168.1.18:${COPY_PATH} ."
echo "  ssh ${COPY_USER}@192.168.1.18 rm ${COPY_PATH}"
echo "  sudo install -m 600 ${CLIENT_NAME}.conf /etc/wireguard/${CLIENT_NAME}.conf"
echo "  sudo systemctl enable --now wg-quick@${CLIENT_NAME}"
echo
echo "A PNG of the QR code is also saved at: ${CLIENT_DIR}/${CLIENT_NAME}.png"
