import QtQuick
import Nyx.Ui
import Nyx.PluginView
import ".."

// A plugin's tile: its own view, or the native toggle (a split tile when the part has
// a detail view) for an item without one.
Item {
    id: t
    property var ctx
    property int cols: 2
    property var tile
    readonly property string tileId: tile ? tile.widget : ""
    readonly property var plugins: ctx.client.state.plugins || ({ kinds: [], tiles: {} })
    readonly property var kind: (plugins.kinds || []).find(k => k.id === tileId) || null
    readonly property var entry: (plugins.tiles || {})[tileId] || null
    readonly property var item: entry ? entry.item : null
    readonly property bool hasDetail: !!(entry && entry.popover)
    readonly property bool on: !!(item && (item["class"] || []).some(c => c === "on" || c === "active"))

    visible: item ? item.visible !== false : true

    function main() {
        if (item && item.on_click) {
            ctx.hide();
            ctx.call("plugins.click", { tile: tileId });
        } else if (item && item.view === undefined && hasDetail && !(kind && kind.detail)) {
            ctx.openDetail(tileId);
        } else {
            ctx.call("plugins.event", { tile: tileId, item: item ? item.id : (kind ? kind.part : ""), kind: "toggle", value: !on });
        }
    }

    TileFrame {
        anchors.fill: parent
        visible: !!(t.item && t.item.view)
        ctx: t.ctx
        swatch: t.kind && t.kind.swatch ? t.kind.swatch : ""
        tooltip: t.item && t.item.tooltip ? t.item.tooltip : ""
        clickable: !!(t.item && (t.item.on_click || t.hasDetail))
        onClicked: t.item.on_click ? t.main() : t.ctx.openDetail(t.tileId)
        PluginView {
            anchors.fill: parent
            anchors.margins: 8
            view: t.item && t.item.view ? t.item.view : null
            onViewEvent: (kind, control, value) => t.ctx.call("plugins.event", { tile: t.tileId, item: t.item.id, kind: kind, control: control, value: value })
        }
    }

    ToggleTile {
        anchors.fill: parent
        visible: !(t.item && t.item.view)
        ctx: t.ctx
        swatch: t.kind && t.kind.swatch ? t.kind.swatch : "blue"
        icon: (t.item && t.item.icon) || (t.kind ? t.kind.icon : "application-x-addon-symbolic")
        title: t.kind ? t.kind.name : ""
        status: t.item && t.item.text ? t.item.text : ""
        active: t.on
        split: t.hasDetail && !!(t.kind && t.kind.detail) && t.cols >= 2
        moreTip: title + ": more"
        tooltip: t.item && t.item.tooltip ? t.item.tooltip : (status ? title + ": " + status : title)
        onMain: t.main()
        onMore: t.ctx.openDetail(t.tileId)
    }
}
