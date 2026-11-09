import QtQuick
import "tiles"

// One tile of the layout, by its widget id.
Loader {
    id: v

    property var ctx
    property var tile
    property int cols: 1
    property int rows: 1
    property int page: 0
    property int index: 0
    readonly property string widget: tile ? tile.widget : ""
    readonly property bool isPlugin: widget.indexOf("plugin:") === 0
    readonly property bool known: isPlugin || comps[widget] !== undefined

    signal volumeMenu(var anchor)

    readonly property var comps: ({
            wifi: wifi, bluetooth: bluetooth, tailscale: tailscale, vpn: vpn, dnd: dnd, "night-light": nightLight,
            "keep-awake": keepAwake, microphone: mic, mode: mode, capture: capture, "keyboard-light": kbd,
            battery: battery, power: power, sound: sound, media: media, upcoming: upcoming, home: home, backup: backup
        })
    sourceComponent: isPlugin ? ((ctx.client.state.plugins && (ctx.client.state.plugins.kinds || []).some(k => k.id === widget)) ? plugin : missing) : (comps[widget] || null)
    onLoaded: if (item.volumeMenu)
        item.volumeMenu.connect(a => v.volumeMenu(a))

    Component { id: wifi; WifiTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
    Component { id: bluetooth; BluetoothTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
    Component { id: tailscale; TailscaleTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
    Component { id: vpn; VpnTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
    Component {
        id: dnd
        SimpleToggle {
            ctx: v.ctx; cols: v.cols; tile: v.tile
            field: "dnd"; op: "omarchy.toggle_dnd"; swatch: "violet"; icon: "notifications-disabled-symbolic"; title: "Do not disturb"
            tips: ["Do not disturb is on: pop-ups stay quiet. Click to allow them", "Do not disturb: click to silence pop-ups (they still reach the Notification Center)"]
        }
    }
    Component {
        id: nightLight
        SimpleToggle {
            ctx: v.ctx; cols: v.cols; tile: v.tile
            field: "night_light"; op: "omarchy.toggle_night_light"; swatch: "coral"; icon: "night-light-symbolic"; title: "Night light"
            tips: ["Night light is on: warmer screen colours. Click to turn it off", "Night light: click for warmer screen colours"]
        }
    }
    Component {
        id: keepAwake
        SimpleToggle {
            ctx: v.ctx; cols: v.cols; tile: v.tile
            field: "keep_awake"; op: "omarchy.toggle_keep_awake"; swatch: "amber"; icon: "preferences-desktop-screensaver-symbolic"; title: "Keep awake"; onStatus: "Screen stays on"
            tips: ["Keep awake is on: the screen won't dim, lock or sleep. Click to turn it off", "Keep awake: click to stop the screen dimming, locking or sleeping"]
        }
    }
    Component { id: mic; MicTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
    Component { id: mode; ModeTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
    Component { id: capture; CaptureTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
    Component { id: kbd; KeyboardLightTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
    Component { id: battery; BatteryTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
    Component { id: power; PowerTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
    Component { id: sound; SoundTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
    Component { id: media; MediaTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
    Component { id: upcoming; UpcomingTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
    Component { id: home; HomeTile { ctx: v.ctx; cols: v.cols; tile: v.tile; page: v.page; index: v.index } }
    Component { id: backup; BackupTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
    Component { id: plugin; PluginTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
    Component { id: missing; MissingTile { ctx: v.ctx; cols: v.cols; tile: v.tile } }
}
