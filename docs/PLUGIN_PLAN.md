# Plan: plugin tiles in the Control Center

Status: built (2026-11-03). Depends on the shared plugin system from the Nyx Bar plan
(`nyx-bar/docs/PLAN.md`). Desktop widgets are in `nyxos-project/nyx-desktop-widgets`.

### Built, and where it differs

- Steps 2 to 8, except as below; step 1 followed (see below).
- No new `control_center` messages. A tile is an item, as in the bar; the `widgets`
  list (in `hello` and its own message, shared with desktop widgets) gives each
  placed tile's size; the part's popover is its detail view, with `popover_open` and
  `popover_close`. One plugin can then serve the bar, the desktop and the Control
  Center with the same code.
- Instead of `cc.toggle` and `cc.split` view nodes, an item without a view is drawn
  as the native toggle (or split, with `detail = true` and a popover), from its
  `icon`, `text` and `on` class. Parts give `icon`, `swatch` and `detail` in the
  manifest.
- **From plugins** comes first in the tray. Tiles placed by `nyx plugin` show while
  the panel is open (it watches the layout file).
- A test copy (`--only`) no longer announces itself to the other panels, so it never
  closes the real Notification Center; `NYX_CC_TEST_PAGE=<n>` opens it on a page.

### Done since

- `nyx plugin` names each system change a plugin could make (and system services it
  doesn't know), defaults to no, and needs `--allow-system` beside `--yes`.
- Tiles' frames are capped at 30 a second and dropped while the panel is hidden.
- Spin and flood tiles, measured: the spinner stays at its 10% cap; under the flood the
  Control Center used 11% CPU until the host learned to stop reading a plugin that's
  far over its rate (now 0.6%), and the panel answered D-Bus in 4 to 6 ms throughout.

- The crate move (step 1): the plugin crates are in nyx-core (0b511a6) and
  `nyx-plugin-view` in nyx-design (ff5895f); the Control Center pins those, with
  nyx-theme from the same nyx-design revision so there's one copy of it.

## Goal

Plugins can add Control Center tiles and detail views without a Control Center rebuild.
A plugin's Control Center parts are chosen with `nyx plugin add` or `nyx plugin
configure`, placed with the existing edit mode, and drawn with the same view protocol as
the bar. Third-party plugins run sandboxed and under resource limits, exactly as in the
bar. The 18 native tiles stay native; nothing about them changes.

## What exists today

- The tiles are a hard-coded list: `tiles::kinds()` (`apps/control-center/src/tiles.rs:66-142`),
  each a `Kind { id, name, icon, sizes: &'static [Size], build: fn(&Ctx, Size, &Tile) }`.
- Detail views come from a closed `Detail` enum (`tiles.rs:28-50`), and `details::view`
  dispatches on it (`details.rs:11-20`).
- The layout is in `~/.config/nyx/control-center.toml`, version 3. A tile is
  `{ widget, size, options }`.
  - Drawing skips unknown widget IDs (`panel.rs:807-809`), but the saved layout keeps
    them, because `page_grid` filters only when drawing. A plugin tile whose plugin is
    missing therefore survives a save.
  - Edit mode doesn't show such a tile, though, so it can't be removed.
- Edit mode has drag and drop, sizes, pages and the "Add widgets" tray
  (`panel.rs:290-391`, `799-890`). Live values run only while a tile is mapped
  (`ui::bind`), so a hidden panel runs nothing.

## Steps

### 1. Move the shared crates out of nyx-bar

This is the second user of the plugin system. Following ARCHITECTURE.md §10, the
shared crates move out of nyx-bar:

- `nyx-plugin`, `nyx-plugin-protocol`, `nyx-plugin-host` and `nyx-plugin-sdk` go to
  `nyx-core`;
- `nyx-plugin-view` (GTK) goes to `nyx-design`, beside `nyx-ui`.

Re-pin nyx-bar to the new revisions. Coordinate with any session working in nyx-core
first, and if nyx-core is busy, pin the Control Center to nyx-bar's crates for now.

### 2. Manifest and protocol for the `control-center` kind

```toml
[control-center]
mode = "stream"                    # interval also allowed, as in the bar
[[control-center.items]]
id = "tile"
name = "Weather"
sizes = ["1x1", "2x1", "2x2"]      # first is the default; checked against Size::parse
swatch = "teal"                    # @cc_<swatch>; the accent if the theme lacks it
detail = true                      # the tile can open a detail view
default = true
```

New messages go in a `control_center` module of `nyx-plugin-protocol`:

- Host to plugin:
  - `tile_open`: item, size, page visible;
  - `resize`;
  - `visible` (shared with the bar);
  - `detail_open` and `detail_close`;
  - `event`, as in the bar.
- Plugin to host:
  - `tile`: item, `view`;
  - `detail`: item, `view` (header, groups, cards, rows, toggles, sliders, scroll);
  - `patch`, `popover` and `menu`, as in the bar.

Two view templates make plugin tiles look native without hand-built trees:

- `cc.toggle { icon, title, status, on, swatch }` is the same layout as `tiles::toggle`:
  just the icon chip at 1×1, and chip, title and status at 2×1.
- `cc.split { …toggle, detail: true }` is the same as `tiles::split`. The main area sends
  `event: toggle`, and the arrow opens the detail view.

Plugins can still send any view tree, scoped CSS, vector nodes or frames. These
Control Center limits apply in addition to the shared ones:

- a tile can be at most 4×3 cells;
- a detail view is the panel width (400 px) and scrolls;
- frames run at no more than 30 fps, and only while the panel is open.

### 3. Dynamic tile registry

- In `tiles.rs`:
  - Replace `kind(id) -> Option<Kind>` with
    `kind(id) -> Option<KindRef>`, where `KindRef` is either `Native(Kind)` or
    `Plugin(PluginKind)`.
  - Plugin tile IDs are `plugin:<plugin-id>/<item>`; native IDs stay as they are, so
    existing layouts don't change.
  - `PluginKind { id, item, name, icon, sizes: Vec<Size>, swatch }` comes from the manifest.
  - Sizes become `Cow<'static, [Size]>`, so `edit_cell` and `next_size` work for both.
- `PluginKind::build` returns a `nyx-plugin-view` widget, with the `cc-tile`, `cc-{c}x{r}`
  and `tint-<swatch>` classes added, so tiles keep the same frame, spacing and colours.
- `Detail` gains `Plugin { id, item }`. `details::view` renders the plugin's `detail`
  view under the standard `head` with a back button.
- The registry merges native kinds with the enabled `control-center` parts from
  `plugins.toml`. It rebuilds when `plugins.toml` or any plugin folder changes, using the
  host's GIO monitors.

### 4. Edit mode

- The "Add widgets" tray gains a **From plugins** group: every enabled plugin tile that
  isn't already placed, with its icon and name.
  - Installed plugins with Control Center parts that aren't enabled appear below it,
    under "Turn on in a terminal: `nyx plugin configure <id>`".
  - The Control Center never approves third-party permissions itself.
- A placed tile whose plugin is missing, disabled or stopped shows in edit mode as a
  placeholder saying "<name> isn't available". It can still be removed and moved.
  Outside edit mode it's still skipped, as today.
- Removing a plugin tile from the grid doesn't disable the plugin, because the part can
  be used elsewhere. Adding it from the tray enables that part in `plugins.toml` if it
  isn't already.
- Placement on install: `nyx plugin add` and `configure` put a newly enabled tile at the
  end of the last page, by editing `control-center.toml` with `toml_edit`. The layout
  version stays 3; `plugin:` IDs need no migration.

### 5. Process lifecycle

The panel is hidden most of the time, so plugin tiles shouldn't run while it is.

- A tile plugin is started the first time its tile is shown. While the panel is open,
  it gets `visible = true`; when the panel is hidden, it gets `visible = false` and
  should pause.
- After 2 minutes hidden, the host stops the process (`stop_after` in the host config).
  The last view is cached, so the next open shows it instantly while the plugin restarts
  and refreshes. Trusted plugins can set `keep_alive = true` to skip stopping.
- Units are named `nyx-plugin-control-center-<id>.service`. Each host starts its own
  instance of a plugin that has several kinds, so a crash in the Control Center
  instance doesn't affect the bar.

### 6. Security

The same model as the bar: bubblewrap for third-party plugins, approved permissions,
the shared D-Bus proxy, and `host_commands`. Two additions for the Control Center:

- The permission review warns more strongly about `system_talk` names that change
  system state, such as NetworkManager, BlueZ, power-profiles-daemon or systemd-logind:
  "can change network settings", and so on.
- Tile actions that open other Nyx panels (`--toggle`) go through `host_commands`, as
  in the bar.

### 7. Config, D-Bus and settings

- `control-center.toml` gains `[plugins."<id>"]` (settings) and
  `[plugins."<id>".limits]`, as in `bar.toml`.
- The app serves `io.github.design_nexus.nyx.ControlCenterPlugins1`: `ListPlugins`
  (state, grants, limits, cgroup memory and CPU), `RestartPlugin` and `Reload`, so
  `nyx plugin list` can show Control Center instances too.
- Nyx Settings → Desktop → Control Center lists plugin tiles next to native ones. That
  belongs to nyx-settings and isn't part of this plan.

### 8. First plugin tiles

- `nyx.weather`: add the `control-center` part to the first-party plugin in nyx-bar, a
  2×1 tile with the current conditions that opens `nyx-weather --toggle`. This proves a
  single plugin can provide both bar and Control Center parts.
- `examples/cc-ping`: a third-party example in Python. It's a 2×1 tile showing latency
  to a host, with a detail view listing recent pings. It needs `network`, and it's used
  for the sandbox and limit tests.

## Testing

Use a headless output only (the screenshot rule) and
`NYX_CC_NO_KEYBOARD=1 XDG_CONFIG_HOME=<scratch> nyx-control-center --only HEADLESS-<n> --show`,
with `NYX_CC_TEST_EDIT` and a new `NYX_CC_TEST_DETAIL=plugin:<id>/<item>`.

- `cargo test`:
  - registry merging;
  - `plugin:` ID parsing;
  - a missing plugin tile survives save, migrate and tidy;
  - plugin sizes go through `next_size` and `pack`;
  - `toml_edit` placement keeps comments.
- Screenshots:
  - the `nyx.weather` tile at each size next to native tiles;
  - the `cc.toggle` and `cc.split` templates against `wifi` and `bluetooth`, checking
    that they look the same;
  - the plugin detail view;
  - the edit-mode tray with **From plugins**;
  - a placeholder for a missing plugin.
- Lifecycle: the plugin gets `visible = false` when the panel hides, the process stops
  after the grace period, and the cached view shows on reopen.
- Sandbox and limits: `cc-ping` without `network` can't reach the network. A test tile
  that floods messages or spins the CPU is throttled, and the panel stays responsive.
- Install with `./install.sh </dev/null`, confirm `~/.local/bin/nyx-control-center` has
  a fresh timestamp, and tell Ken to reopen the Control Center (its service is resident).

## Not in this plan

- Converting native tiles to plugins.
- Plugin pages, meaning a whole page owned by a plugin.
- Approving permissions from inside the Control Center.
- The Nyx Settings page.
- NyxOS packaging changes beyond rebuilding with the new pins. The Control Center isn't
  packaged for NyxOS yet (class 3 in the NyxOS audit).
