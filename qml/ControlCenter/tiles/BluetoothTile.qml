import QtQuick
import Nyx.Shell
import ".."

ToggleTile {
    id: t
    property int cols: 2
    property var tile
    readonly property var st: bt.value || ({ devices: [] })
    readonly property var connected: (st.devices || []).filter(d => d.connected)
    BackendTopic { id: bt; client: t.ctx.client; name: "bluetooth"; active: t.ctx.open && t.visible }

    swatch: "teal"
    split: cols >= 2
    title: "Bluetooth"
    icon: "bluetooth-symbolic"
    active: !!st.powered
    enabled: st.available !== false
    opacity: enabled ? 1 : 0.5
    status: !st.available ? "No adapter" : !st.powered ? "Off" : connected.length === 0 ? "On" : connected.length === 1 ? connected[0].name : connected.length + " devices"
    tooltip: !st.available ? "Bluetooth: no adapter found" : !st.powered ? "Bluetooth is off. Click to turn it on" : connected.length === 0 ? "Bluetooth is on, nothing connected. Click to turn it off" : "Connected: " + connected.map(d => d.name).join(", ") + ". Click to turn Bluetooth off"
    onMain: ctx.call("bluetooth.set_powered", { on: !st.powered })
    onMore: ctx.openDetail("bluetooth")
}
