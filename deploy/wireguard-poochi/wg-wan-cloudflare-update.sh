#!/usr/bin/env bash
# Keeps wg.syedyaseen.dev pointed at this network's current public IP, so the
# WireGuard endpoint (wg.syedyaseen.dev:<port>) stays reachable after the ISP
# hands out a new address.
#
# Same GET-then-PUT pattern as /home/poochi/mz/proxy/cfip.sh (which does this
# for themizadah.com), but scoped to its own env vars/token so the two never
# share credentials -- the syedyaseen.dev token this script uses must NOT also
# cover the mizadah zone, and vice versa.
#
# Unlike DuckDNS (which infers the caller's public IP from the update
# request), Cloudflare's API takes an explicit IP, so this fetches it from
# ipify first.
#
# Reads CF_ZONE_ID / CF_API_TOKEN / WG_HOSTNAME from the environment (the
# systemd unit supplies these via EnvironmentFile=/etc/wireguard/wg-cloudflare.env).
set -euo pipefail

: "${CF_ZONE_ID:?CF_ZONE_ID not set}"
: "${CF_API_TOKEN:?CF_API_TOKEN not set}"
: "${WG_HOSTNAME:?WG_HOSTNAME not set}"

CURRENT_IP=$(curl -s --connect-timeout 3 --max-time 5 https://api.ipify.org)
if [[ -z "${CURRENT_IP}" ]]; then
  echo "ERROR: could not determine current public IP." >&2
  exit 1
fi

AUTH_HEADER="Authorization: Bearer ${CF_API_TOKEN}"

DNS_RESPONSE=$(curl -sS -X GET \
  "https://api.cloudflare.com/client/v4/zones/${CF_ZONE_ID}/dns_records?name=${WG_HOSTNAME}&type=A" \
  -H "${AUTH_HEADER}" -H "Content-Type: application/json")

if ! echo "${DNS_RESPONSE}" | grep -q '"success":true'; then
  echo "ERROR: failed fetching DNS record for ${WG_HOSTNAME}: ${DNS_RESPONSE}" >&2
  exit 1
fi

RECORD_ID=$(echo "${DNS_RESPONSE}" | jq -r '.result[0].id')
CLOUDFLARE_IP=$(echo "${DNS_RESPONSE}" | jq -r '.result[0].content')

if [[ -z "${RECORD_ID}" || "${RECORD_ID}" == "null" ]]; then
  echo "ERROR: no A record for ${WG_HOSTNAME} in zone ${CF_ZONE_ID} -- create it" \
    "once in the Cloudflare dashboard first (see ../wireguard/README.md's equivalent step)." >&2
  exit 1
fi

if [[ "${CURRENT_IP}" == "${CLOUDFLARE_IP}" ]]; then
  echo "No change needed for ${WG_HOSTNAME} (still ${CURRENT_IP})"
  exit 0
fi

UPDATE_DATA=$(jq -n --arg name "${WG_HOSTNAME}" --arg ip "${CURRENT_IP}" \
  '{type:"A",name:$name,content:$ip,ttl:60,proxied:false}')

UPDATE_RESPONSE=$(curl -sS -X PUT \
  "https://api.cloudflare.com/client/v4/zones/${CF_ZONE_ID}/dns_records/${RECORD_ID}" \
  -H "${AUTH_HEADER}" -H "Content-Type: application/json" \
  --data "${UPDATE_DATA}")

if ! echo "${UPDATE_RESPONSE}" | grep -q '"success":true'; then
  echo "ERROR: failed updating ${WG_HOSTNAME}: ${UPDATE_RESPONSE}" >&2
  exit 1
fi

echo "Updated ${WG_HOSTNAME} -> ${CURRENT_IP}"
