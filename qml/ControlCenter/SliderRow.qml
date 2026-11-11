import QtQuick
import Nyx.Ui

// A slider row: an icon button in the row's colour, the bar, the value and an end
// button. A value coming back from the system doesn't fight a drag; moves are sent
// after `debounce` ms of rest.
Item {
    id: r

    property string icon: ""
    property string iconTip: ""
    property color tint: Style.accent
    property real from: 0
    property real to: 1
    property real value: 0
    property string valueText: ""
    property bool loud: false
    property string endIcon: ""
    property string endTip: ""
    property bool endOn: false
    property int debounce: 40
    readonly property Item iconItem: lead

    signal moved(real v)
    signal iconClicked
    signal endClicked

    IconButton {
        id: lead
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        icon: r.icon
        tooltip: r.iconTip
        onClicked: r.iconClicked()
        Component.onCompleted: color = Qt.binding(() => hovered ? Style.rowHover : "transparent")
    }
    Item {
        id: track
        anchors.left: lead.right
        anchors.leftMargin: 4
        anchors.right: val.left
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        height: 20
        readonly property real shown: area.pressed ? area.live : r.value
        readonly property real fraction: r.to > r.from ? Math.max(0, Math.min(1, (shown - r.from) / (r.to - r.from))) : 0
        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width
            height: 6
            radius: 3
            color: Style.alpha(Style.text, 0.1)
            Rectangle {
                height: parent.height
                radius: 3
                width: Math.max(6, parent.width * track.fraction)
                color: r.tint
            }
        }
        Rectangle {
            width: 16
            height: 16
            radius: 8
            color: "white"
            border.width: 1
            border.color: Style.alpha(Style.shadow, 0.25)
            anchors.verticalCenter: parent.verticalCenter
            x: Math.max(0, Math.min(parent.width - width, parent.width * track.fraction - width / 2))
        }
        MouseArea {
            id: area
            anchors.fill: parent
            anchors.margins: -6
            property real live: r.value
            cursorShape: Qt.PointingHandCursor
            function at(mx) {
                return r.from + (r.to - r.from) * Math.max(0, Math.min(1, (mx - 6) / track.width));
            }
            onPressed: m => {
                live = at(m.x);
                send.restart();
            }
            onPositionChanged: m => {
                if (pressed) {
                    live = at(m.x);
                    send.restart();
                }
            }
            onWheel: w => {
                live = Math.max(r.from, Math.min(r.to, r.value + (w.angleDelta.y > 0 ? 1 : -1) * (r.to - r.from) / 20));
                send.restart();
            }
        }
        Timer {
            id: send
            interval: r.debounce
            onTriggered: r.moved(area.live)
        }
    }
    Label {
        id: val
        anchors.right: end.left
        anchors.rightMargin: 2
        anchors.verticalCenter: parent.verticalCenter
        width: 38
        horizontalAlignment: Text.AlignRight
        text: r.valueText
        size: 12
        font.features: { "tnum": 1 }
        color: r.loud ? Style.warning : Style.dimText
    }
    IconButton {
        id: end
        visible: r.endIcon !== ""
        width: visible ? 32 : 0
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        icon: r.endIcon
        tooltip: r.endTip
        toggled: r.endOn
        onClicked: r.endClicked()
    }
}
