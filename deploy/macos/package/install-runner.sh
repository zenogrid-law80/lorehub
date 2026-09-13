#!/bin/sh
set -eu

[ "$(id -u)" -eq 0 ] || { echo "Run this installer with sudo." >&2; exit 1; }
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
install_dir="/usr/local/libexec/lorehub-runner"
data_dir="/Library/Application Support/LoreHub Runner"
plist="/Library/LaunchDaemons/co.kr.zenogrid.lorehub.runner.plist"
label="co.kr.zenogrid.lorehub.runner"

launchctl bootout "system/$label" >/dev/null 2>&1 || true
install -d -m 0755 "$install_dir"
install -d -m 0700 "$data_dir" "$data_dir/work"
install -m 0755 "$script_dir/lorehub" "$install_dir/lorehub"
install -m 0755 "$script_dir/lore" "$install_dir/lore"
install -m 0755 "$script_dir/run-runner.sh" "$install_dir/run-runner.sh"
if [ ! -f "$data_dir/runner.env" ]; then
  install -m 0600 "$script_dir/runner.env.example" "$data_dir/runner.env"
fi

sed "s|@RUNNER_SCRIPT@|$install_dir/run-runner.sh|g; s|@STDOUT_LOG@|$data_dir/runner.stdout.log|g; s|@STDERR_LOG@|$data_dir/runner.stderr.log|g" \
  "$script_dir/co.kr.zenogrid.lorehub.runner.plist.in" > "$plist"
chown root:wheel "$plist"
chmod 0644 "$plist"
launchctl bootstrap system "$plist"
echo "LoreHub Runner installed. Edit '$data_dir/runner.env', add the JWT files, then run:"
echo "  sudo launchctl kickstart -k system/$label"
