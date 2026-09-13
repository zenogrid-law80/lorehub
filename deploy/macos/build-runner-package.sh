#!/bin/sh
set -eu

if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
  echo "Usage: $0 <version> <lorehub> <lore> [output.tar.gz]" >&2
  exit 2
fi
version=${1#v}
runner_binary=$2
lore_binary=$3
output=${4:-deploy/downloads/lorehub-runner-macos-aarch64-v$version.tar.gz}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_dir=$(CDPATH= cd -- "$script_dir/../.." && pwd)
case "$output" in /*) ;; *) output="$project_dir/$output" ;; esac

[ "$(uname -s)" = Darwin ] && [ "$(uname -m)" = arm64 ] || { echo "This package is for macOS arm64." >&2; exit 2; }
[ -f "$runner_binary" ] || { echo "Runner binary not found: $runner_binary" >&2; exit 2; }
[ -f "$lore_binary" ] || { echo "Lore binary not found: $lore_binary" >&2; exit 2; }
file "$runner_binary" "$lore_binary" | grep -q "Mach-O 64-bit executable arm64" || { echo "Both binaries must be macOS arm64 executables." >&2; exit 2; }

stage=$(mktemp -d "${TMPDIR:-/tmp}/lorehub-macos.XXXXXX")
trap 'rm -rf "$stage"' EXIT INT TERM
package="$stage/macos"
install -d "$package"
install -m 0755 "$runner_binary" "$package/lorehub"
install -m 0755 "$lore_binary" "$package/lore"
for file in run-runner.sh install-runner.sh uninstall-runner.sh; do install -m 0755 "$script_dir/package/$file" "$package/$file"; done
for file in runner.env.example co.kr.zenogrid.lorehub.runner.plist.in README.txt; do install -m 0644 "$script_dir/package/$file" "$package/$file"; done
mkdir -p "$(dirname -- "$output")"
COPYFILE_DISABLE=1 tar -C "$stage" -czf "$output" macos
echo "Created $output"
