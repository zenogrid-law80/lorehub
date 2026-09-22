#!/bin/sh
# DATABASE_URL is the explicit source. Never overwrite an existing backup.
set -eu
umask 077
if [ "$#" -ne 1 ] || [ -z "${DATABASE_URL:-}" ]; then
    echo 'Usage: DATABASE_URL=... sh scripts/backup-postgres.sh existing-backup-directory' >&2
    exit 2
fi
if command -v pg_config >/dev/null 2>&1; then
    PATH="$(pg_config --bindir):$PATH"
    export PATH
fi
backup_dir=$(mktemp -d "$1/lorehub-$(date -u +%Y%m%dT%H%M%SZ).XXXXXX")
backup_complete=false
cleanup() {
    if [ "$backup_complete" = false ]; then rm -rf "$backup_dir"; fi
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
# Use the explicit source URL; ignore other connection overrides.
unset PGHOST PGHOSTADDR PGPORT PGUSER PGPASSWORD PGSERVICE PGSERVICEFILE PGOPTIONS
unset PGDATABASE
pg_dump --dbname="$DATABASE_URL" --format=custom --file="$backup_dir/lorehub.dump"
pg_restore --list "$backup_dir/lorehub.dump" > /dev/null
backup_complete=true
printf '%s\n' "$backup_dir/lorehub.dump"
