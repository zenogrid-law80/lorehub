#!/bin/zsh
set -euo pipefail

project_dir="/Users/law80/GitHub/lorehub"
data_dir="$project_dir/.runner-macos"
runner_bin="$data_dir/lorehub"

export HOME="/Users/law80"
export PATH="/Users/law80/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

set -a
source "$project_dir/.env"
set +a

export LOREHUB_RUNNER_NAME="${LOREHUB_MACOS_RUNNER_NAME:-macos-runner-01}"
export LOREHUB_COORDINATOR_URL="${LOREHUB_COORDINATOR_URL:-http://127.0.0.1:8080}"
mkdir -p "$data_dir/work"
if [[ ! -x "$runner_bin" ]]; then
  install -m 0755 "$project_dir/target/release/lorehub" "$runner_bin"
fi

exec "$runner_bin" worker \
  --work-dir "$data_dir/work"
