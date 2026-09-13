#!/bin/sh
set -eu

if [ "$#" -lt 4 ] || [ "$#" -gt 5 ]; then
  echo "Usage: $0 <linux|macos|windows> <x86_64|aarch64> <version> <binary> [release-dir]" >&2
  exit 2
fi

runner_os=$1
runner_arch=$2
version=${3#v}
source_binary=$4
release_dir=${5:-deploy/runner-releases}

case "$runner_os" in linux|macos|windows) ;; *) echo "Unsupported OS: $runner_os" >&2; exit 2 ;; esac
case "$runner_arch" in x86_64|aarch64) ;; *) echo "Unsupported architecture: $runner_arch" >&2; exit 2 ;; esac
test -f "$source_binary" || { echo "Binary not found: $source_binary" >&2; exit 2; }

if [ "$runner_os" = windows ]; then
  command -v file >/dev/null 2>&1 || { echo "file is required to validate Windows Runner binaries." >&2; exit 2; }
  binary_type=$(file -b "$source_binary")
  case "$binary_type" in
    *"PE32+ executable"*"x86-64"*"MS Windows"*) ;;
    *) echo "Windows Runner release must be a raw x86_64 PE executable, not an MSI or archive: $binary_type" >&2; exit 2 ;;
  esac
fi

suffix=
if [ "$runner_os" = windows ]; then
  suffix=.exe
fi
mkdir -p "$release_dir"
destination="$release_dir/lorehub-runner-$runner_os-$runner_arch-v$version$suffix"
install -m 0755 "$source_binary" "$destination"
echo "Published $destination"
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$destination"
else
  shasum -a 256 "$destination"
fi
echo "Restart the LoreHub coordinator to load this release."
