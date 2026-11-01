# Control Center

Quick settings for [Omarchy](https://omarchy.org) and Hyprland: a panel on the right
edge of the screen with your connections, sound, brightness, power mode, what's playing
and Omarchy's own switches. It takes its colors from your Omarchy theme.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/design-nexus/nyx-control-center/main/install.sh | bash
```

It runs as a user service and stays hidden until you open it:

```sh
nyx-control-center --toggle     # open or close (bind a key to this)
```

Click outside it or press Esc to close it.

## What it does

- **Wi-Fi, Bluetooth, VPN**: the tile switches it on or off; the arrow opens the full
  list in the panel (join networks, with the password typed in place; metered
  connection; connect or disconnect paired devices; turn VPNs and Tailscale on or off).
- **Sound and brightness** sliders. The speaker button opens outputs and each app's
  volume; the output button mutes.
- **Now playing** from any player (Nyx Music, browsers, …) with its controls.
- **Power mode** (power saver, balanced, performance) and the battery.
- **Omarchy's switches**: Do not disturb, Night light, Keep awake, the workspace's mode
  (tiling, scrolling, floating; a click goes to the next), keyboard light, and a
  screenshot.
- Pages of tiles: swipe or scroll sideways, or click the dots. It opens on the page you
  left (unless you turn that off).
- Opening it closes the Nyx Notification Center, and the other way round.

Everything is changed the way Omarchy changes it (its own commands for Do not disturb,
Night light, Keep awake and the workspace mode; NetworkManager, BlueZ, PipeWire and
power-profiles-daemon for the rest), so the bar and keybindings stay in step. Nothing
runs while the panel is closed: each tile listens only while it's on screen.

## Layout

`~/.config/nyx/control-center.toml`, written on first start:

```toml
remember_page = true

[[pages]]
tiles = [
  { widget = "wifi", size = "2x1" },
  { widget = "bluetooth", size = "2x1" },
  { widget = "sound", size = "4x1" },
  { widget = "dnd", size = "1x1" },
]

[[pages]]
tiles = [{ widget = "battery", size = "1x1" }]
```

Tiles fill a 4-column grid in order, each in the first place it fits. Widgets and the
sizes they come in:

| Widget | Sizes |
| --- | --- |
| `wifi`, `bluetooth`, `vpn` | 2x1 (with details), 1x1 |
| `sound` (volume and brightness) | 4x1, 2x1 |
| `media`, `power` | 2x1, 4x1 |
| `dnd`, `mode`, `night-light`, `keep-awake`, `keyboard-light`, `capture`, `battery` | 1x1, 2x1 |

## Build

```sh
cargo build --release --locked -p nyx-control-center
cargo test --locked --workspace
```

`cargo run -p nyx-providers --example dump` prints what every source reports.
`nyx-control-center --only HEADLESS-1 --show` opens it on one output only.

## Remove

```sh
./uninstall.sh            # or --purge to remove the layout file too
```

## License

MIT
