#!/bin/sh
set -eu

install_dir=${LOREHUB_INSTALL_DIR:-/usr/local/bin}
config_dir=${LOREHUB_CONFIG_DIR:-/etc/lorehub}
unit_dir=${LOREHUB_SYSTEMD_DIR:-/etc/systemd/system}
instance=${LOREHUB_RUNNER_INSTANCE:-1}
runner_dir="/var/lib/lorehub-$instance"

if [ "$(id -u)" -ne 0 ]; then
  echo "Run this installer as root (for example: sudo ./install-runner.sh)." >&2
  exit 1
fi

package_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
runner_binary=${LOREHUB_BINARY:-$package_dir/lorehub}
lore_binary=${LORE_BINARY:-$package_dir/lore}
test -f "$runner_binary" || { echo "LoreHub binary not found: $runner_binary" >&2; exit 2; }
if [ ! -f "$lore_binary" ]; then
  if [ -x "$install_dir/lore" ] && [ -z "${LORE_BINARY:-}" ]; then
    lore_binary=
  else
    echo "Lore binary not found: $lore_binary" >&2
    exit 2
  fi
fi
getent group lorehub >/dev/null 2>&1 || groupadd --system lorehub
id lorehub >/dev/null 2>&1 || useradd --system --gid lorehub --home-dir /var/lib/lorehub --create-home lorehub
if [ -n "$lore_binary" ]; then
  install -m 0755 "$lore_binary" "$install_dir/lore"
fi
install -d -o lorehub -g lorehub -m 0700 "$runner_dir" "$runner_dir/bin" "$runner_dir/work"
install -o lorehub -g lorehub -m 0755 "$runner_binary" "$runner_dir/bin/lorehub"
install -d -m 0700 "$config_dir"
if [ ! -f "$config_dir/environment" ]; then
  install -m 0600 "$package_dir/runner.env.example" "$config_dir/environment"
fi
install -m 0644 "$package_dir/lorehub-worker@.service" "$unit_dir/lorehub-worker@.service"
systemctl daemon-reload
systemctl enable "lorehub-worker@${instance}.service"

echo "Edit $config_dir/environment, then start with:"
echo "  sudo systemctl start lorehub-worker@${instance}.service"
