#!/bin/zsh
set -euo pipefail

install_dir="/usr/local/libexec/lorehub-runner"
data_dir="/Library/Application Support/LoreHub Runner"
environment_file="$data_dir/runner.env"

[ -f "$environment_file" ] || { echo "Missing Runner environment file: $environment_file" >&2; exit 1; }
export HOME="$data_dir"
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
set -a
source "$environment_file"
set +a
mkdir -p "$data_dir/work"
exec "$install_dir/lorehub" worker --work-dir "$data_dir/work"
