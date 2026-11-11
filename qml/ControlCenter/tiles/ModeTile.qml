import QtQuick
import Nyx.Shell
import ".."

ToggleTile {
    id: t
    property int cols: 1
    property var tile
    readonly property var st: mode.value || ({ workspace: 0, mode: "tiling", label: "Tiling", modes: ["Tiling", "Scrolling"] })
    // Why the last change didn't happen, shown for a while instead of the workspace.
    property string err: ""
    Timer {
        id: clear
        interval: 4000
        onTriggered: t.err = ""
    }
    BackendTopic { id: mode; client: t.ctx.client; name: "mode"; active: !!(t.ctx && t.ctx.open && t.visible) }

    swatch: "green"
    active: true
    title: st.label
    compactLabel: st.label
    status: err !== "" ? err : "Workspace " + st.workspace
    icon: ({ tiling: "view-grid-symbolic", scrolling: "view-continuous-symbolic", floating: "window-new-symbolic" })[st.mode] || "view-grid-symbolic"
    tooltip: err !== "" ? "Couldn't change the mode: " + err : "Workspace " + st.workspace + ": " + String(st.label).toLowerCase() + ". Click for the next mode (" + (st.modes || []).join(", ").toLowerCase() + ")"
    onMain: ctx.call("mode.cycle", {}, (ok, e) => {
        t.err = e ? e.msg : "";
        if (e)
            clear.restart();
    })
}
