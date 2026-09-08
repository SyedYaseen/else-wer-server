#!/usr/bin/env bash
# Keeps the WAN DuckDNS record pointed at this network's current public IP,
# so the WireGuard endpoint (<subdomain>.duckdns.org:<port>) stays reachable
# after the ISP hands out a new address.
#
# Unlike deploy/https-duckdns/duckdns-update.sh (which explicitly reports
# this box's LAN IP, since that record needs to resolve to a LAN address),
# this one leaves the "ip" parameter blank. DuckDNS then uses the source IP
# of the update request itself -- which, since the request leaves the LAN
# and crosses the internet to reach DuckDNS, is this network's public IP as
# seen from the outside. That's simpler and more reliable than querying a
# third-party "what's my IP" service.
#
# Reads WAN_DUCKDNS_SUBDOMAIN / WAN_DUCKDNS_TOKEN from the environment
# (systemd unit supplies these via EnvironmentFile=/etc/wireguard/wan-duckdns.env).
set -euo pipefail

: "${WAN_DUCKDNS_SUBDOMAIN:?WAN_DUCKDNS_SUBDOMAIN not set}"
: "${WAN_DUCKDNS_TOKEN:?WAN_DUCKDNS_TOKEN not set}"

RESPONSE=$(curl -fsS "https://www.duckdns.org/update?domains=${WAN_DUCKDNS_SUBDOMAIN}&token=${WAN_DUCKDNS_TOKEN}&ip=")

if [[ "${RESPONSE}" != OK* ]]; then
  echo "DuckDNS update failed: ${RESPONSE}" >&2
  exit 1
fi

echo "DuckDNS WAN record for ${WAN_DUCKDNS_SUBDOMAIN}.duckdns.org updated (${RESPONSE})"
