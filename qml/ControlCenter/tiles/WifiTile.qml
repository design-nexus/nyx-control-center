import QtQuick
import Nyx.Shell
import ".."

ToggleTile {
    id: t
    property int cols: 2
    property var tile
    readonly property var st: net.value || ({})
    BackendTopic { id: net; client: t.ctx.client; name: "network"; active: t.ctx.open && t.visible }

    swatch: "blue"
    split: cols >= 2
    title: "Wi-Fi"
    active: !!st.wifi_on
    on: !!(st.wifi_on && st.has_wifi) && split && !compact
    icon: !st.wifi_on ? "network-wireless-offline-symbolic" : !st.ssid ? "network-wireless-no-route-symbolic" : ["network-wireless-signal-weak-symbolic", "network-wireless-signal-weak-symbolic", "network-wireless-signal-ok-symbolic", "network-wireless-signal-good-symbolic", "network-wireless-signal-excellent-symbolic"][Math.min(4, st.bars || 0)]
    status: !st.has_wifi ? "No Wi-Fi" : !st.wifi_on ? "Off" : (st.ssid ? st.ssid.trim() : "Not connected")
    tooltip: !st.has_wifi ? "Wi-Fi: this computer has no Wi-Fi" : !st.wifi_on ? "Wi-Fi is off. Click to turn it on" : st.ssid ? "Connected to " + st.ssid.trim() + " (" + st.signal + "% signal). Click to turn Wi-Fi off" : "Wi-Fi is on, not connected. Click to turn it off"
    onMain: ctx.call("network.set_wifi", { on: !st.wifi_on })
    onMore: ctx.openDetail("wifi")
}
