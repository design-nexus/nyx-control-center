import QtQuick
import Nyx.Ui
import ".."

// A tile whose plugin isn't available, shown in edit mode so it can be moved or removed.
TileFrame {
    property int cols: 2
    property var tile
    readonly property string plugin: tile ? String(tile.widget).replace(/^plugin:/, "").split("/")[0] : ""
    Label {
        anchors.centerIn: parent
        width: parent.width - 16
        horizontalAlignment: Text.AlignHCenter
        text: parent.plugin + " isn't available"
        size: 11.5
        dim: true
    }
}
