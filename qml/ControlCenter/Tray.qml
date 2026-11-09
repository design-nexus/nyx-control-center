import QtQuick
import Nyx.Ui

// Edit mode's tray: plugin tiles turned on but not placed, plugins to turn on, then
// the native widgets not on any page.
Column {
    id: t

    property var ctx
    property var pages: []
    property var kinds: []
    property var plugins: ({ kinds: [], off: [] })
    readonly property var used: pages.reduce((all, pg) => all.concat((pg.tiles || []).map(x => x.widget)), [])
    readonly property var freePlugins: (plugins.kinds || []).filter(k => used.indexOf(k.id) < 0)
    readonly property var freeKinds: kinds.filter(k => used.indexOf(k.id) < 0)

    signal add(string id, string size)

    spacing: 0
    topPadding: 12

    Separator {
        width: parent.width
    }
    Group {
        visible: t.freePlugins.length > 0 || (t.plugins.off || []).length > 0
        text: "From plugins"
    }
    Grid {
        visible: t.freePlugins.length > 0
        width: parent.width
        columns: 3
        spacing: 8
        Repeater {
            model: t.freePlugins
            delegate: TrayCard {
                required property var modelData
                width: (t.width - 16) / 3
                ctx: t.ctx
                icon: modelData.icon
                name: modelData.name
                sizes: modelData.sizes
                onAdd: t.add(modelData.id, modelData.sizes[0])
            }
        }
    }
    Label {
        visible: (t.plugins.off || []).length > 0
        topPadding: 8
        text: "Turn on in a terminal:"
        size: 11.5
        dim: true
    }
    Repeater {
        model: t.plugins.off || []
        delegate: Label {
            required property var modelData
            text: "nyx plugin configure " + modelData.id
            font.family: Style.mono
            font.pointSize: 8.625
            dim: true
        }
    }
    Group {
        text: t.freeKinds.length ? "Add widgets" : "Every widget is on a page"
    }
    Grid {
        width: parent.width
        columns: 3
        spacing: 8
        Repeater {
            model: t.freeKinds
            delegate: TrayCard {
                required property var modelData
                width: (t.width - 16) / 3
                ctx: t.ctx
                icon: modelData.icon
                name: modelData.name
                sizes: modelData.sizes
                onAdd: t.add(modelData.id, modelData.sizes[0])
            }
        }
    }
}
