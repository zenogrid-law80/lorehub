#!/bin/sh
set -eu

: "${CF_ZONE_ID:?set CF_ZONE_ID}"
: "${CF_API_TOKEN:?set CF_API_TOKEN}"
: "${CERTBOT_AUTH_OUTPUT:?Certbot did not provide CERTBOT_AUTH_OUTPUT}"

curl -fsS -X DELETE "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/dns_records/$CERTBOT_AUTH_OUTPUT" -H "Authorization: Bearer $CF_API_TOKEN" | jq -e '.success == true' >/dev/null
