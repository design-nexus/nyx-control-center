import QtQuick
import Quickshell
import Nyx.Ui

// Brightness and colour for a light, under its cell.
PopupWindow {
    id: pop

    property Item anchorItem
    property var ctx
    property var device: ({})
    readonly property bool keyboardOff: Quickshell.env("NYX_SHELL_NO_KEYBOARD") === "1"

    function openAt(x, y) {
        anchor.rect.x = x;
        anchor.rect.y = y;
        visible = true;
    }

    anchor.item: anchorItem
    anchor.adjustment: PopupAdjustment.All
    grabFocus: !keyboardOff
    color: "transparent"
    visible: false
    implicitWidth: 280
    implicitHeight: col.implicitHeight + 24

    Surface {
        anchors.fill: parent
        shadow: false
        Column {
            id: col
            x: 12
            y: 12
            width: parent.width - 24
            spacing: 8
            Label {
                text: pop.device.name || ""
                font.weight: Font.DemiBold
            }
            SliderRow {
                visible: !!pop.device.dimmable
                width: parent.width
                height: 32
                icon: "display-brightness-symbolic"
                iconTip: "Brightness"
                from: 1
                to: 100
                value: pop.device.brightness !== null && pop.device.brightness !== undefined ? Math.max(1, pop.device.brightness * 100) : 100
                valueText: Math.round(value) + "%"
                debounce: 150
                onMoved: v => pop.ctx.call("home.call", { entity: pop.device.id, service: "turn_on", data: { brightness_pct: Math.round(v) } })
            }
            Row {
                visible: !!pop.device.colorable
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 6
                Repeater {
                    model: [["Warm", [255, 180, 107]], ["White", [255, 244, 229]], ["Blue", [91, 156, 255]], ["Violet", [157, 123, 255]], ["Pink", [255, 111, 174]], ["Green", [70, 208, 138]]]
                    delegate: Rectangle {
                        id: sw
                        required property var modelData
                        width: 26
                        height: 26
                        radius: 13
                        color: Qt.rgba(modelData[1][0] / 255, modelData[1][1] / 255, modelData[1][2] / 255, 1)
                        border.width: swArea.containsMouse ? 2 : 0
                        border.color: Style.text
                        MouseArea {
                            id: swArea
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: pop.ctx.call("home.call", { entity: pop.device.id, service: "turn_on", data: { rgb_color: sw.modelData[1] } })
                        }
                        Tip {
                            target: sw
                            text: sw.modelData[0]
                            shown: swArea.containsMouse
                        }
                    }
                }
            }
        }
    }
}
