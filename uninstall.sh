#!/usr/bin/env bash
# Remove Nyx Control Center.
#
#   ./uninstall.sh            remove the program and its service
#   ./uninstall.sh --purge    also remove its layout (~/.config/nyx/control-center.toml)
set -euo pipefail
say() { printf '\033[1m==>\033[0m %s\n' "$*"; }

bin="$HOME/.local/bin"
config="${XDG_CONFIG_HOME:-$HOME/.config}"
systemctl --user disable --now nyx-control-center.service 2>/dev/null || true
rm -f "$bin/nyx-control-center" "$config/systemd/user/nyx-control-center.service"
omarchy-plugin-disable design-nexus.control-center >/dev/null 2>&1 || true
rm -rf "$config/omarchy/plugins/design-nexus.control-center"
systemctl --user daemon-reload 2>/dev/null || true
if [[ ${1:-} == --purge ]]; then
  rm -f "$config/nyx/control-center.toml"
fi
say "Nyx Control Center removed."
