#!/bin/sh
set -eu

if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
  echo "Usage: $0 <version> <lorehub.exe> <lore.exe> [output.msi]" >&2
  exit 2
fi

version=${1#v}
runner_binary=$2
lore_binary=$3
output=${4:-deploy/downloads/lorehub-runner-windows-x86_64-v$version.msi}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_dir=$(CDPATH= cd -- "$script_dir/../.." && pwd)

case "$version" in
  *[!0-9.]*|.*|*.) echo "Version must contain three numeric components." >&2; exit 2 ;;
esac
[ "$(printf '%s' "$version" | awk -F. '{print NF}')" -eq 3 ] || { echo "Version must contain three numeric components." >&2; exit 2; }
command -v wixl >/dev/null 2>&1 || { echo "wixl is required (Homebrew: brew install msitools)." >&2; exit 2; }
command -v msiinfo >/dev/null 2>&1 || { echo "msiinfo is required (Homebrew: brew install msitools)." >&2; exit 2; }
[ -f "$runner_binary" ] || { echo "Runner binary not found: $runner_binary" >&2; exit 2; }
[ -f "$lore_binary" ] || { echo "Lore binary not found: $lore_binary" >&2; exit 2; }

case "$output" in /*) ;; *) output="$project_dir/$output" ;; esac
stage=$(mktemp -d "${TMPDIR:-/tmp}/lorehub-msi.XXXXXX")
trap 'rm -rf "$stage"' EXIT INT TERM

install -m 0755 "$runner_binary" "$stage/lorehub.exe"
install -m 0755 "$lore_binary" "$stage/lore.exe"
for file in run-runner.ps1 configure-runner.ps1 uninstall-runner.ps1 README.txt; do
  install -m 0644 "$script_dir/$file" "$stage/$file"
done
install -m 0644 "$script_dir/runner.env.msi.example" "$stage/runner.env.example"
sed "s/@VERSION@/$version/g" "$script_dir/lorehub-runner.wxs.in" > "$stage/lorehub-runner.wxs"
grep -F -- '--install-directory &quot;[INSTALLFOLDER].&quot;' "$stage/lorehub-runner.wxs" >/dev/null || {
  echo "INSTALLFOLDER must append a dot before the closing quote." >&2
  exit 2
}
mkdir -p "$(dirname -- "$output")"
(cd "$stage" && wixl -a x64 -o "$output" lorehub-runner.wxs)
execute_sequence=$(msiinfo export "$output" InstallExecuteSequence | tr -d '\r')
install_initialize=$(printf '%s\n' "$execute_sequence" | awk -F '\t' '$1 == "InstallInitialize" { print $3 }')
remove_existing=$(printf '%s\n' "$execute_sequence" | awk -F '\t' '$1 == "RemoveExistingProducts" { print $3 }')
case "$install_initialize:$remove_existing" in
  *[!0-9:]*|:*|*:) echo "Unable to validate MSI upgrade sequence." >&2; exit 2 ;;
esac
[ "$remove_existing" -gt "$install_initialize" ] || {
  echo "RemoveExistingProducts must run after InstallInitialize to support deferred upgrade actions." >&2
  exit 2
}
printf '%s\n' "$execute_sequence" | awk -F '\t' '$1 == "UnregisterRunner" { print $2 }' |
  grep -F 'NOT UPGRADINGPRODUCTCODE' >/dev/null || {
    echo "UnregisterRunner must not run during a major upgrade." >&2
    exit 2
  }
echo "Created $output"
