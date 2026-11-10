import QtQuick
import Nyx.Shell
import ".."

// Phones and other devices paired through Flux (Nyx Connect).
ToggleTile {
    id: t
    property int cols: 2
    property var tile
    readonly property var st: dv.value || ({ devices: [], pairing: [] })
    readonly property var connected: (st.devices || []).filter(d => d.connected)
    readonly property var pairing: st.pairing || []
    BackendTopic { id: dv; client: t.ctx.client; name: "devices"; active: !!(t.ctx && t.ctx.open && t.visible) }

    function battery(d) {
        return d.battery !== null && d.battery !== undefined ? " · " + d.battery + "%" : "";
    }

    swatch: "green"
    split: cols >= 2
    title: "Devices"
    icon: "phone-symbolic"
    moreTip: "Devices: connected phones, ring and send the clipboard"
    active: connected.length > 0
    enabled: st.installed !== false
    opacity: enabled ? 1 : 0.5
    status: !st.installed ? "Not installed" : !st.available ? "Flux isn't running" : pairing.length > 0 ? pairing[0] + " wants to pair" : connected.length === 0 ? ((st.devices || []).length === 0 ? "None paired" : "None connected") : connected.length === 1 ? connected[0].name + battery(connected[0]) : connected.length + " connected"
    tooltip: !st.installed ? "Flux isn't installed" : !st.available ? "Flux isn't running. Click to start it" : connected.length === 0 ? "No device is connected. Click for your devices" : "Connected: " + connected.map(d => d.name).join(", ") + ". Click for your devices"
    onMain: st.installed && !st.available ? ctx.call("devices.start", {}) : ctx.openDetail("devices")
    onMore: ctx.openDetail("devices")
}
