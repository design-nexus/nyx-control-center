import QtQuick
import Nyx.Ui
import Nyx.PluginView
import ".."

// A plugin tile's detail view: its popover.
Column {
    id: v
    property var ctx
    property string tileId: ""
    signal back
    readonly property var plugins: ctx.client.state.plugins || ({ kinds: [], tiles: {} })
    readonly property var kind: (plugins.kinds || []).find(k => k.id === tileId) || null
    readonly property var entry: (plugins.tiles || {})[tileId] || null
    readonly property var item: entry ? entry.item : null

    Component.onCompleted: if (item) ctx.call("plugins.event", { tile: tileId, item: item.id, kind: "popover_open" })
    Component.onDestruction: if (item) ctx.call("plugins.event", { tile: tileId, item: item.id, kind: "popover_close" })

    DetailHead {
        title: v.kind ? v.kind.name : ""
        hasSwitch: false
        onBack: v.back()
    }
    Flickable {
        width: parent.width
        height: Math.min(contentHeight, 480)
        clip: true
        contentHeight: view.implicitHeight
        boundsBehavior: Flickable.StopAtBounds
        PluginView {
            id: view
            width: parent.width
            height: implicitHeight
            view: v.entry && v.entry.popover ? v.entry.popover : null
            onViewEvent: (kind, control, value) => v.ctx.call("plugins.event", { tile: v.tileId, item: v.item ? v.item.id : "", kind: kind, control: control, value: value })
        }
    }
    Label {
        width: parent.width
        visible: !(v.entry && v.entry.popover)
        horizontalAlignment: Text.AlignHCenter
        topPadding: 24
        text: "Nothing to show yet"
        dim: true
    }
}
