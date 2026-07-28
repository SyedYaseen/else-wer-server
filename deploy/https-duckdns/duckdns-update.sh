#!/usr/bin/env bash
# Points the DuckDNS record at this box's current LAN IP. DuckDNS happily
# stores private addresses, and Let's Encrypt DNS-01 never connects to the
# IP — so the cert works while everything stays LAN-only.
set -euo pipefail

# shellcheck source=/dev/null
[[ -n "${DUCKDNS_SUBDOMAIN:-}" ]] || source /etc/caddy/duckdns.env

IP=$(ip route get 1.1.1.1 2>/dev/null | awk '{ for (i=1;i<NF;i++) if ($i=="src") print $(i+1) }')
if [[ -z "$IP" ]]; then
  echo "Could not determine LAN IP (offline?); skipping" >&2
  exit 0
fi

RESULT=$(curl -fsS "https://www.duckdns.org/update?domains=${DUCKDNS_SUBDOMAIN}&token=${DUCKDNS_TOKEN}&ip=${IP}")
if [[ "$RESULT" != "OK" ]]; then
  echo "DuckDNS update failed: ${RESULT}" >&2
  exit 1
fi
echo "DuckDNS: ${DUCKDNS_SUBDOMAIN}.duckdns.org -> ${IP}"
