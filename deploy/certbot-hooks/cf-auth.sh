#!/bin/sh
set -eu

: "${CF_ZONE_ID:?set CF_ZONE_ID}"
: "${CF_API_TOKEN:?set CF_API_TOKEN}"
: "${CERTBOT_VALIDATION:?Certbot did not provide CERTBOT_VALIDATION}"

payload=$(jq -n --arg value "$CERTBOT_VALIDATION" '{type:"TXT",name:"_acme-challenge.ytlaw80.com",content:$value,ttl:120}')
record_id=$(curl -fsS -X POST "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/dns_records" -H "Authorization: Bearer $CF_API_TOKEN" -H 'Content-Type: application/json' --data "$payload" | jq -er 'select(.success == true) | .result.id')
sleep "${CERTBOT_DNS_PROPAGATION_SECONDS:-60}"
printf '%s\n' "$record_id"
