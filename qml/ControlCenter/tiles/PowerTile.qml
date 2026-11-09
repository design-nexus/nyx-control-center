import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

// Power mode: a segmented control of its three icons.
TileFrame {
    id: t
    property int cols: 2
    property var tile
    readonly property var st: pw.value || ({ profiles: [], profile: null })
    BackendTopic { id: pw; client: t.ctx.client; name: "power"; active: t.ctx.open && t.visible }

    Rectangle {
        anchors.fill: parent
        anchors.margins: 5
        radius: Style.radiusSmall
        color: Style.alpha(Style.text, 0.035)
        Row {
            anchors.fill: parent
            anchors.margins: 2
            spacing: 2
            Repeater {
                model: [["power-saver", "power-profile-power-saver-symbolic", "green", "Power saver: longer battery life, slower"], ["balanced", "power-profile-balanced-symbolic", "blue", "Balanced: the everyday setting"], ["performance", "power-profile-performance-symbolic", "coral", "Performance: fastest, uses more power and runs warmer"]].filter(p => t.st.profiles.length === 0 || t.st.profiles.indexOf(p[0]) >= 0)
                delegate: Rectangle {
                    id: seg
                    required property var modelData
                    required property int index
                    readonly property bool sel: t.st.profile === modelData[0]
                    width: (parent.width - 2 * (t.st.profiles.length > 0 ? t.st.profiles.length - 1 : 2)) / Math.max(1, t.st.profiles.length || 3)
                    height: parent.height
                    radius: 5
                    color: sel ? Style.raised : area.containsMouse ? Style.alpha(Style.text, 0.05) : "transparent"
                    Icon {
                        anchors.centerIn: parent
                        name: seg.modelData[1]
                        size: 18
                        recolor: true
                        color: seg.sel ? t.ctx.swatch(seg.modelData[2]) : Style.dimText
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
