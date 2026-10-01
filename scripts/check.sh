#!/bin/sh
# The same verification entry point for developers, GitHub, and Lore runners.
set -eu

check_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$check_root"

for check_tool in cargo rustfmt node protoc python3; do
    if ! command -v "$check_tool" >/dev/null 2>&1; then
        echo "Missing prerequisite: $check_tool. See README.md: verification." >&2
        exit 1
    fi
done

echo 'Checking Rust formatting and lints'
cargo fmt --check
cargo clippy --locked --no-default-features --all-targets -- -D warnings

echo 'Running JavaScript regression tests'
node --test tests/*.test.mjs

echo 'Running folder archive regression tests'
python3 -B tests/folder_archive_test.py
python3 -B tests/server_snapshot_test.py

echo 'Validating the Lore CI configuration'
cargo run --locked --no-default-features -- validate .lore-ci.toml

echo 'Running all Rust tests, including PostgreSQL integration tests'
sh scripts/with-test-postgres.sh cargo test --locked --no-default-features -- --include-ignored

echo 'All checks passed'
