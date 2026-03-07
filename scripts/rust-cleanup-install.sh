#!/usr/bin/env bash
# Install helper for the Rust cleanup workflow.
#
# Can:
# - install a user-level systemd timer that runs daily
# - print a cron alternative
#
# Usage:
#   ./scripts/rust-cleanup-install.sh --install-systemd-user
#   ./scripts/rust-cleanup-install.sh --print-cron

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CLEANUP_SCRIPT="$ROOT_DIR/scripts/rust-cleanup.sh"
SYSTEMD_USER_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"

usage() {
  cat <<'USAGE'
Usage: rust-cleanup-install.sh [options]

Options:
  --install-systemd-user  Install and enable a daily user-level systemd timer
  --print-cron            Print a daily cron entry
  -h, --help              Show this help

Examples:
  ./scripts/rust-cleanup-install.sh --install-systemd-user
  ./scripts/rust-cleanup-install.sh --print-cron
USAGE
}

install_systemd_user() {
  mkdir -p "$SYSTEMD_USER_DIR"

  cat > "$SYSTEMD_USER_DIR/rust-cleanup.service" <<EOF2
[Unit]
Description=Rust target cleanup under %h/projects

[Service]
Type=oneshot
ExecStart=$CLEANUP_SCRIPT
EOF2

  cat > "$SYSTEMD_USER_DIR/rust-cleanup.timer" <<'EOF2'
[Unit]
Description=Daily Rust target cleanup

[Timer]
OnCalendar=*-*-* 08:30:00
Persistent=true

[Install]
WantedBy=timers.target
EOF2

  systemctl --user daemon-reload
  systemctl --user enable --now rust-cleanup.timer
  echo "Installed systemd user timer:"
  echo "  $SYSTEMD_USER_DIR/rust-cleanup.service"
  echo "  $SYSTEMD_USER_DIR/rust-cleanup.timer"
  echo
  systemctl --user status rust-cleanup.timer --no-pager || true
}

print_cron() {
  cat <<EOF2
# Daily Rust target cleanup (08:30)
30 8 * * * $CLEANUP_SCRIPT >> "${XDG_STATE_HOME:-$HOME/.local/state}/rust-cleanup/cron.log" 2>&1
EOF2
}

if [[ $# -eq 0 ]]; then
  usage
  exit 0
fi

while [[ $# -gt 0 ]]; do
  case "$1" in
    --install-systemd-user)
      install_systemd_user
      shift
      ;;
    --print-cron)
      print_cron
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done
