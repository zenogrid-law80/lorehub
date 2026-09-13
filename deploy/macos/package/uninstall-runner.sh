#!/bin/sh
set -eu

[ "$(id -u)" -eq 0 ] || { echo "Run this uninstaller with sudo." >&2; exit 1; }
label="co.kr.zenogrid.lorehub.runner"
plist="/Library/LaunchDaemons/$label.plist"
launchctl bootout "system/$label" >/dev/null 2>&1 || true
[ ! -f "$plist" ] || rm -f "$plist"
rm -rf "/usr/local/libexec/lorehub-runner"
echo "LoreHub Runner removed. Configuration remains in /Library/Application Support/LoreHub Runner."
