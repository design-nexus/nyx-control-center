import QtQuick
import Nyx.Shell
import ".."

ToggleTile {
    id: t
    property int cols: 1
    property var tile
    readonly property var st: mode.value || ({ workspace: 0, mode: "tiling", label: "Tiling" })
    BackendTopic { id: mode; client: t.ctx.client; name: "mode"; active: t.ctx.open && t.visible }

    swatch: "green"
    active: true
    title: st.label
    status: "Workspace " + st.workspace
    icon: ({ tiling: "view-grid-symbolic", scrolling: "view-continuous-symbolic", floating: "window-new-symbolic" })[st.mode] || "view-grid-symbolic"
    tooltip: "Workspace " + st.workspace + ": " + String(st.label).toLowerCase() + ". Click for the next mode (tiling, scrolling, floating)"
    onMain: ctx.call("mode.cycle", {})
}
