import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

// Power mode: its name over a segmented row of the three modes (leaf, gauge, bolt, the
// suite's own drawings so they never look alike), the chosen one filled in its swatch.
TileFrame {
    id: t
    property int cols: 2
    property var tile
    readonly property var st: pw.value || ({ profiles: [], profile: null })
    readonly property var modes: [["power-saver", "icons/leaf.svg", "green", "Power saver: longer battery life, slower"], ["balanced", "icons/gauge.svg", "blue", "Balanced: the everyday setting"], ["performance", "icons/bolt.svg", "coral", "Performance: fastest, uses more power and runs warmer"]].filter(p => t.st.profiles.length === 0 || t.st.profiles.indexOf(p[0]) >= 0)
    // The name and the row, as tall as a one-cell tile.
    readonly property real need: 64
    BackendTopic { id: pw; client: t.ctx.client; name: "power"; active: !!(t.ctx && t.ctx.open && t.visible) }

    Label {
        id: name
        x: 12
        anchors.bottom: track.top
        anchors.bottomMargin: 6
        text: "Power mode"
        size: 12.5
        font.weight: Font.Medium
    }

    Rectangle {
        id: track
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: 9
        height: 28
        radius: 8
        color: Style.alpha(Style.text, 0.06)
        Row {
            anchors.fill: parent
            anchors.margins: 2
            spacing: 2
            Repeater {
                model: t.modes
                delegate: Rectangle {
                    id: seg
                    required property var modelData
                    readonly property bool sel: t.st.profile === modelData[0]
                    width: (parent.width - (t.modes.length - 1) * 2) / Math.max(1, t.modes.length)
                    height: parent.height
                    radius: 6
                    color: sel ? t.ctx.swatch(modelData[2]) : area.containsMouse ? Style.alpha(Style.text, 0.06) : "transparent"
                    Behavior on color {
                        ColorAnimation {
                            duration: Style.fast
                        }
                    }
                    Icon {
                        anchors.centerIn: parent
                        name: Qt.resolvedUrl("../" + seg.modelData[1])
                        size: 15
                        recolor: true
                        color: seg.sel ? Style.onAccent : area.containsMouse ? Style.text : Style.dimText
                    }
                    MouseArea {
                        id: area
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: if (!seg.sel) t.ctx.call("power.set_profile", { profile: seg.modelData[0] })
                    }
                    Tip {
                        target: seg
                        text: seg.modelData[3]
                        shown: area.containsMouse
                    }
                }
            }
        }
    }
}
