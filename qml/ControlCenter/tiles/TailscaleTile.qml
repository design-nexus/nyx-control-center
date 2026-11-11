import QtQuick
import Nyx.Shell
import ".."

ToggleTile {
    id: t
    property int cols: 2
    property var tile
    readonly property var st: ts.value || ({ peers: [] })
    readonly property int online: (st.peers || []).filter(p => p.online).length
    BackendTopic { id: ts; client: t.ctx.client; name: "tailscale"; active: !!(t.ctx && t.ctx.open && t.visible) }

    swatch: "violet"
    split: cols >= 2
    title: "Tailscale"
    icon: "network-workgroup-symbolic"
    active: !!st.running
    enabled: st.installed !== false
    opacity: enabled ? 1 : 0.5
    status: !st.installed ? "Not installed" : st.backend === "NeedsLogin" ? "Needs sign-in" : !st.running ? "Off" : st.exit_node ? "Exit: " + (st.exit_node.location || st.exit_node.name) : online + " online"
    tooltip: !st.installed ? "Tailscale isn't installed" : st.running ? "Tailscale is on in " + st.tailnet + " (" + online + " machines online). Click to turn it off" : "Tailscale is off. Click to connect to your tailnet"
    onMain: ctx.call("tailscale.set_up", { on: !st.running })
    onMore: ctx.openDetail("tailscale")
}
