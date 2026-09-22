#!/bin/sh
# Restore only into the disposable local cluster created by our test wrapper.
set -eu
if [ "$#" -ne 1 ] || [ ! -f "$1" ]; then
    echo 'Usage: sh scripts/verify-postgres-backup.sh trusted-lorehub.dump' >&2
    exit 2
fi
verify_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
sh "$verify_root/scripts/with-test-postgres.sh" sh -eu -c '
    pg_restore --exit-on-error --single-transaction --no-owner --no-privileges --dbname="$DATABASE_URL" "$1"
    psql -X --dbname="$DATABASE_URL" --set=ON_ERROR_STOP=1 --file="$2/scripts/verify-restored-postgres.sql"
' sh "$1" "$verify_root"
