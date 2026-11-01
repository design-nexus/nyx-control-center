#!/usr/bin/env bash
# Install Nyx Control Center.
#
#   curl -fsSL https://raw.githubusercontent.com/design-nexus/nyx-control-center/main/install.sh | bash
#
# It runs as a user service and opens with `nyx-control-center --toggle` (bind a key to
# it, or use the bar button).
set -euo pipefail

REPO="design-nexus/nyx-control-center"
say() { printf '\033[1m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[33m==>\033[0m %s\n' "$*" >&2; }
die() { printf '\033[31m==>\033[0m %s\n' "$*" >&2; exit 1; }

command -v hyprctl >/dev/null || warn "Hyprland wasn't found. Nyx Control Center is made for Omarchy (Hyprland)."

if command -v pacman >/dev/null; then
  need=()
  for p in gtk4 gtk4-layer-shell pkgconf gcc git brightnessctl; do pacman -Qq "$p" >/dev/null 2>&1 || need+=("$p"); done
  if ((${#need[@]})); then
    say "Installing ${need[*]}"
    sudo pacman -S --needed --noconfirm "${need[@]}"
  fi
fi

# This checkout, or a fresh one when piped from curl.
here=""
[[ -n ${BASH_SOURCE[0]:-} && -f ${BASH_SOURCE[0]} ]] && here=$(cd "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
if [[ -n $here && -f $here/Cargo.toml && -d $here/apps/control-center ]]; then
  src=$here
else
  say "Fetching the source"
  git clone -q --depth 1 "https://github.com/$REPO.git" "$work/src"
  src="$work/src"
fi

if ! command -v cargo >/dev/null; then
  command -v pacman >/dev/null || die "Rust (cargo) is needed to build Nyx Control Center. Install it and run this again."
  say "Installing rust"
  sudo pacman -S --needed --noconfirm rust
fi
say "Building Nyx Control Center (a few minutes the first time)"
(cd "$src" && cargo build --release --locked -p nyx-control-center)

bin="$HOME/.local/bin"
units="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
mkdir -p "$bin" "$units"
target="${CARGO_TARGET_DIR:-$src/target}/release"
# Replace the file rather than writing into it, so a running panel keeps its old binary
# until it restarts.
install -m 755 "$target/nyx-control-center" "$bin/nyx-control-center.new" && mv -f "$bin/nyx-control-center.new" "$bin/nyx-control-center"
install -m 644 "$src/apps/control-center/data/nyx-control-center.service" "$units/nyx-control-center.service"
systemctl --user daemon-reload 2>/dev/null || true

# The bar button, for omarchy-shell. It's put in place, not added to the bar: turn it on
# from Settings → Plugins (or `omarchy-plugin-enable design-nexus.control-center right`).
plugins="${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/plugins"
if [[ -d $plugins ]]; then
  mkdir -p "$plugins/design-nexus.control-center"
  install -m 644 "$src/apps/control-center/plugin/manifest.json" "$src/apps/control-center/plugin/BarWidget.qml" \
    "$plugins/design-nexus.control-center/"
  omarchy-shell shell rescanPlugins >/dev/null 2>&1 || true
fi

if systemctl --user is-active --quiet nyx-control-center.service 2>/dev/null; then
  say "Installed. The running panel uses the new build after: systemctl --user restart nyx-control-center"
else
  systemctl --user enable --now nyx-control-center.service 2>/dev/null || true
  say "Installed. Open it with: nyx-control-center --toggle"
fi
case ":$PATH:" in
  *":$bin:"*) ;;
  *) warn "$bin isn't on your PATH." ;;
esac
