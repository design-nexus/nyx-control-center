import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

// Up next: the coming events from Nyx Calendar, in their calendars' colours.
TileFrame {
    id: t
    property int cols: 4
    property var tile
    readonly property var st: cal.value || ({ available: false, events: [] })
    BackendTopic { id: cal; client: t.ctx.client; name: "calendar"; active: t.ctx.open && t.visible }

    clickable: true
    tooltip: "Open Calendar"
    onClicked: ctx.call("open_app", { id: "io.github.design_nexus.nyx.Calendar" })

    Row {
        id: head
        x: 10
        y: 8
        spacing: 6
        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: "x-office-calendar-symbolic"
            size: 14
            recolor: true
            color: t.ctx.swatch("red")
        }
        Label {
            text: "Up next"
            size: 12.5
            font.weight: Font.Medium
        }
    }
    Label {
        anchors.centerIn: parent
        visible: !t.st.available || t.st.events.length === 0
        text: t.st.available ? "Nothing coming up" : "Open Calendar once to see events here"
        size: 11.5
        dim: true
    }
    Column {
        anchors.top: head.bottom
        anchors.topMargin: 8
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: 10
        spacing: 4
        Repeater {
            model: (t.st.events || []).slice(0, t.cols >= 4 ? 4 : 3)
            delegate: Rectangle {
                id: ev
                required property var modelData
                readonly property color c: /^#[0-9a-fA-F]{6}/.test(modelData.color || "") ? modelData.color.substr(0, 7) : "#8a8f98"
                width: parent.width
                height: 30
                radius: Style.radiusSmall
                color: Style.mix(Style.card, ev.c, 0.13)
                border.width: 1
                border.color: Style.alpha(ev.c, 0.16)
                Label {
                    anchors.left: parent.left
                    anchors.leftMargin: 8
                    anchors.right: when.left
                    anchors.rightMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    text: ev.modelData.title
                    color: Style.mix(Style.text, ev.c, 0.22)
                    font.weight: Font.Medium
                }
                Label {
                    id: when
                    anchors.right: parent.right
                    anchors.rightMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    text: ev.modelData.when || ""
                    role: "caption"
                    color: Style.alpha(Style.mix(Style.text, ev.c, 0.22), 0.7)
                }
                HoverHandler {
                    id: evh
                }
                Tip {
                    target: ev
                    text: ev.modelData.location ? ev.modelData.title + " at " + ev.modelData.location + " (" + ev.modelData.calendar + ")" : ev.modelData.title + " (" + ev.modelData.calendar + ")"
                    shown: evh.hovered
                }
            }
        }
    }
}
