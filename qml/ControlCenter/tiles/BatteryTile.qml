import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

TileFrame {
    id: t
    property int cols: 1
    property var tile
    readonly property var b: (pw.value && pw.value.battery) || null
    BackendTopic { id: pw; client: t.ctx.client; name: "power"; active: !!(t.ctx && t.ctx.open && t.visible) }

    clickable: true
    onClicked: ctx.openDetail("battery")
    readonly property color tint: !b ? Style.text : (b.percent < 15 && !b.charging) ? ctx.swatch("red") : (b.charging || b.full) ? ctx.swatch("green") : Style.text
    tooltip: !b ? "No battery" : "Battery " + Math.round(b.percent) + "%, " + (b.full ? "full" : b.charging ? (b.duration ? "charging, full in " + b.duration : "charging") : (b.duration ? b.duration + " left" : "on battery")) + ". Click for details"

    Row {
        anchors.centerIn: parent
        spacing: 6
        Icon {
            anchors.verticalCenter: parent.verticalCenter
            size: 18
            recolor: true
            color: t.tint
            name: {
                if (!t.b)
                    return "battery-missing-symbolic";
                const p = t.b.percent;
                const level = p <= 10 ? "empty" : p <= 30 ? "caution" : p <= 60 ? "low" : p <= 90 ? "good" : "full";
                return "battery-" + level + (t.b.charging ? "-charging" : "") + "-symbolic";
            }
        }
        Label {
            anchors.verticalCenter: parent.verticalCenter
            text: t.b ? Math.round(t.b.percent) + "%" : ""
            font.weight: Font.Medium
            color: t.tint
        }
    }
}
