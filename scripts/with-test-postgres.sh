#!/bin/sh
# Always use a new local cluster. Never inherit a developer/runner DATABASE_URL.
set -eu

if [ "$#" -eq 0 ]; then
    echo "Usage: sh scripts/with-test-postgres.sh command [arguments...]" >&2
    exit 2
fi

# Debian/Ubuntu install server tools outside PATH; Homebrew exposes pg_config too.
if command -v pg_config >/dev/null 2>&1; then
    test_pg_bin=$(pg_config --bindir)
    PATH="$test_pg_bin:$PATH"
    export PATH
fi
for test_pg_tool in initdb pg_ctl; do
    if ! command -v "$test_pg_tool" >/dev/null 2>&1; then
        echo "Missing prerequisite: $test_pg_tool. Install PostgreSQL server tools." >&2
        exit 1
    fi
done
if [ "$(id -u)" -eq 0 ]; then
    echo 'Run verification as a non-root user; PostgreSQL cannot run as root.' >&2
    exit 1
fi

# Keep the Unix socket path short, private, and unique even during parallel runs.
test_pg_dir=$(mktemp -d /tmp/lorehub-ci.XXXXXX)
test_pg_child=
cleanup() {
    test_pg_result=$?
    trap - EXIT HUP INT TERM
    if [ -n "$test_pg_child" ]; then
        kill -TERM "$test_pg_child" 2>/dev/null || true
        wait "$test_pg_child" 2>/dev/null || true
    fi
    if [ -s "$test_pg_dir/data/postmaster.pid" ]; then
        if ! pg_ctl -D "$test_pg_dir/data" -m fast -w stop > /dev/null 2>&1; then
            echo "Could not stop test PostgreSQL; retained $test_pg_dir for recovery." >&2
            exit 1
        fi
    fi
    if [ "$test_pg_result" -ne 0 ]; then
        for test_pg_log in "$test_pg_dir/init.log" "$test_pg_dir/server.log"; do
            if [ -f "$test_pg_log" ]; then tail -n 30 "$test_pg_log" >&2; fi
        done
    fi
    rm -rf "$test_pg_dir"
    exit "$test_pg_result"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

initdb -D "$test_pg_dir/data" -U lorehub_test -A trust --no-locale -E UTF8 > "$test_pg_dir/init.log" 2>&1
cat >> "$test_pg_dir/data/postgresql.conf" <<EOF
listen_addresses = ''
unix_socket_directories = '$test_pg_dir'
port = 5432
EOF
pg_ctl -D "$test_pg_dir/data" -l "$test_pg_dir/server.log" -w start > /dev/null

# The query-string host selects our private Unix socket, not a TCP server.
DATABASE_URL="postgresql://lorehub_test@localhost/postgres?host=$test_pg_dir&port=5432"
export DATABASE_URL
unset PGHOST PGHOSTADDR PGPORT PGDATABASE PGUSER PGPASSWORD PGSERVICE PGSERVICEFILE PGOPTIONS

test_pg_result=0
"$@" &
test_pg_child=$!
wait "$test_pg_child" || test_pg_result=$?
test_pg_child=
exit "$test_pg_result"
