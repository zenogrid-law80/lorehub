#!/bin/sh
set -eu

usage() {
    cat >&2 <<'EOF'
Usage: restart-lorehub.sh [api|all]

  api  Restart the LoreHub coordinator only (default).
  all  Restart the coordinator and currently running lorehub-worker instances.
EOF
    exit 2
}

target=${1:-api}
[ "$target" = api ] || [ "$target" = all ] || usage
[ "$#" -le 1 ] || usage

project_dir=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
binary="$project_dir/target/release/lorehub"

as_root() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    else
        command -v sudo >/dev/null 2>&1 || {
            echo "This operation requires root privileges (sudo was not found)." >&2
            exit 1
        }
        sudo "$@"
    fi
}

build_release() {
    command -v cargo >/dev/null 2>&1 || {
        echo "Rust Cargo is required to build LoreHub before restarting it." >&2
        exit 1
    }
    echo "Building LoreHub release binary"
    cargo build --locked --release --manifest-path "$project_dir/Cargo.toml"
    [ -x "$binary" ] || {
        echo "LoreHub release binary was not created: $binary" >&2
        exit 1
    }
}

restart_systemd() {
    command -v systemctl >/dev/null 2>&1 || return 1
    systemctl cat lorehub-api.service >/dev/null 2>&1 || return 1

    echo "Installing LoreHub coordinator binary"
    as_root install -m 0755 "$binary" /usr/local/bin/lorehub
    echo "Restarting lorehub-api.service"
    as_root systemctl restart lorehub-api.service

    if [ "$target" = all ]; then
        running_workers=$(systemctl list-units --type=service --state=running \
            'lorehub-worker@*.service' --no-legend --no-pager | awk '{print $1}')
        for unit in $running_workers; do
            echo "Restarting $unit"
            as_root systemctl restart "$unit"
        done
    fi
}

restart_macos() {
    [ "$(uname -s)" = Darwin ] || return 1
    command -v launchctl >/dev/null 2>&1 || return 1
    label=co.kr.zenogrid.lorehub.coordinator
    launchctl print "system/$label" >/dev/null 2>&1 || return 1

    echo "Restarting $label"
    as_root launchctl kickstart -k "system/$label"
    if [ "$target" = all ]; then
        echo "The macOS coordinator is restarted; runner LaunchDaemons are managed separately." >&2
    fi
}

restart_macos_user_agent() {
    [ "$(uname -s)" = Darwin ] || return 1
    command -v launchctl >/dev/null 2>&1 || return 1

    uid=$(id -u)
    label=co.kr.zenogrid.lorehub.coordinator
    plist="$HOME/Library/LaunchAgents/$label.plist"
    [ -f "$plist" ] || return 1

    echo "Loading and restarting $label"
    launchctl bootstrap "gui/$uid" "$plist" >/dev/null 2>&1 || true
    launchctl kickstart -k "gui/$uid/$label"
    if [ "$target" = all ]; then
        echo "The macOS coordinator is restarted; runner LaunchAgents are managed separately." >&2
    fi
}

restart_direct_macos() {
    [ "$(uname -s)" = Darwin ] || return 1
    command -v pgrep >/dev/null 2>&1 || return 1
    command -v kill >/dev/null 2>&1 || return 1

    binary=${LOREHUB_BINARY:-$binary}
    launcher=${LOREHUB_LAUNCHER:-$project_dir/deploy/macos/run-coordinator.sh}
    pattern="$binary serve"
    pids=$(pgrep -f "$pattern" 2>/dev/null || true)
    if [ -n "$pids" ]; then
        for pid in $pids; do
            echo "Stopping direct LoreHub coordinator (pid $pid)"
            kill -TERM "$pid" 2>/dev/null || as_root kill -TERM "$pid"
        done
        i=0
        still_running=true
        while [ "$i" -lt 10 ]; do
            still_running=false
            for pid in $pids; do
                if kill -0 "$pid" 2>/dev/null; then
                    still_running=true
                    break
                fi
            done
            [ "$still_running" = false ] && break
            i=$((i + 1))
            sleep 1
        done
        [ "$still_running" = false ] || {
            echo "The direct LoreHub coordinator did not stop within 10 seconds." >&2
            return 1
        }
    else
        echo "No direct LoreHub coordinator is running; starting it."
    fi

    [ -f "$launcher" ] || {
        echo "Coordinator launcher not found: $launcher" >&2
        return 1
    }
    if [ -x "$launcher" ]; then
        launcher_command="$launcher"
    else
        command -v zsh >/dev/null 2>&1 || {
            echo "Coordinator launcher is not executable and zsh was not found: $launcher" >&2
            return 1
        }
        launcher_command="$(command -v zsh) $launcher"
    fi
    data_dir=${LOREHUB_DATA_DIR:-$project_dir/.coordinator-macos}
    mkdir -p "$data_dir"
    echo "Starting direct LoreHub coordinator"
    if [ "$(id -u)" -eq 0 ] && [ -n "${SUDO_USER:-}" ]; then
        sudo -u "$SUDO_USER" -H nohup sh -c "$launcher_command" \
            >>"$data_dir/coordinator.stdout.log" 2>>"$data_dir/coordinator.stderr.log" </dev/null &
    else
        nohup sh -c "$launcher_command" \
            >>"$data_dir/coordinator.stdout.log" 2>>"$data_dir/coordinator.stderr.log" </dev/null &
    fi
    if [ "$target" = all ]; then
        echo "The direct macOS coordinator is restarted; runner processes are managed separately." >&2
    fi
}

build_release

if restart_systemd; then
    exit 0
fi

if restart_macos; then
    exit 0
fi

if restart_macos_user_agent; then
    exit 0
fi

if restart_direct_macos; then
    exit 0
fi

echo "Could not find a registered LoreHub systemd service or macOS coordinator LaunchDaemon." >&2
echo "Install deploy/lorehub-api.service, the macOS coordinator plist, or verify deploy/macos/run-coordinator.sh exists." >&2
exit 1
