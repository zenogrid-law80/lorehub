#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$project_dir"

command -v docker >/dev/null 2>&1 || {
    echo "Docker is required to restart the LoreHub stack." >&2
    exit 1
}
command -v curl >/dev/null 2>&1 || {
    echo "curl is required to verify Lore Server health." >&2
    exit 1
}

echo "Building and starting LoreHub Docker services (excluding Compose Runner)"
compose_services="postgres dynamodb-local dynamodb-init lore-server lore-server-local reverse-proxy certbot-renew"
if ! docker compose \
    --profile repository \
    --profile tls \
    up -d --build \
    $compose_services; then
    echo "Docker image build failed; retrying with available local images" >&2
    docker compose \
        --profile repository \
        --profile tls \
        up -d \
        $compose_services
fi

echo "Building and restarting the LoreHub coordinator"
LOREHUB_DATABASE_URL_OVERRIDE="${LOREHUB_DATABASE_URL_OVERRIDE:-postgres://lorehub:lorehub@127.0.0.1:5432/lorehub}" \
    sh "$project_dir/deploy/restart-lorehub.sh" api

echo "Checking LoreHub coordinator and Lore Server health"
curl --fail --silent --show-error --retry 12 --retry-connrefused --retry-delay 2 \
    http://127.0.0.1:8080/ >/dev/null
curl --fail --silent --show-error --retry 12 --retry-connrefused --retry-delay 2 \
    http://127.0.0.1:41339/health_check >/dev/null
curl --fail --silent --show-error --retry 12 --retry-connrefused --retry-delay 2 \
    http://127.0.0.1:41340/health_check >/dev/null

echo "LoreHub stack is running"
docker compose \
    --profile repository \
    --profile tls \
    ps
