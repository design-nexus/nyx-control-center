# Control Center

Application source: apps/control-center (GTK, layer-shell). The widget framework is
crates/nyx-widgets (live values, tile sizes, page layout; no GTK) and the system data
is crates/nyx-providers (NetworkManager, BlueZ, PipeWire via pactl, power-profiles,
UPower, brightnessctl, MPRIS, Omarchy toggles). Both move to nyx-design / nyx-core once
Desktop Widgets uses them too (see nyx/docs/desktop/ARCHITECTURE.md).

Rules: a provider runs only while a view subscribes (tiles subscribe on map, drop on
unmap), so the hidden panel runs nothing. Omarchy state is changed only through
Omarchy's own commands. Passwords go to nmcli on stdin, never argv.

Commands: `cargo test --locked --workspace`; `cargo clippy --workspace --all-targets`;
`./install.sh`; `cargo run -p nyx-providers --example dump` (read-only state dump).
Try it on a headless output without focus: `NYX_CC_NO_KEYBOARD=1 XDG_CONFIG_HOME=<scratch>
nyx-control-center --only HEADLESS-<n> --show` (`NYX_CC_TEST_DETAIL=wifi|bluetooth|vpn`
opens a detail view). Never inject input or steal focus.

Use the public, pinned shared crates; never import another application's internals.
Use canonical STYLE.md from the pinned nyx-design dependency.
Common guidance: https://github.com/design-nexus/nyx/blob/main/docs/AGENTS_COMMON.md
