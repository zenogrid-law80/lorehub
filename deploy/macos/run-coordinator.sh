#!/bin/zsh
set -euo pipefail

project_dir="/Users/law80/GitHub/lorehub"
data_dir="$project_dir/.coordinator-macos"

export HOME="/Users/law80"
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

set -a
source "$project_dir/.env"
set +a

mkdir -p "$data_dir"
exec "$project_dir/target/release/lorehub" serve
