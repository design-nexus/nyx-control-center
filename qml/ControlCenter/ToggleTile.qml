import QtQuick
import Nyx.Ui

// A toggle tile (1×1: its icon, centred; wider: icon, name and status) or, with
// `split`, a split tile: the main part acts, the arrow opens the detail view.
TileFrame {
    id: t

    property string icon: ""
    property string title: ""
    property string status: ""
    property bool active: false
    property bool split: false
    property bool compact: width < 100
    property string moreTip: title + ": networks, devices and settings"

    signal main
    signal more

    clickable: !split || compact
    on: split && active && !compact
    onClicked: t.main()

    Chip {
        id: chip
        anchors.verticalCenter: parent.verticalCenter
        x: t.compact ? (t.width - width) / 2 : 9
        icon: t.icon
        on: t.active
        tint: t.ctx ? t.ctx.swatch(t.swatch) : "transparent"
    }

    Column {
        visible: !t.compact
        anchors.left: chip.right
        anchors.leftMargin: t.split ? 8 : 10
        anchors.right: t.split ? more.left : parent.right
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        Label {
            width: parent.width
            text: t.title
            size: 12.5
            font.weight: Font.Medium
        }
        Label {
            width: parent.width
            visible: text !== ""
            text: t.status
            size: 11.5
            dim: true
        }
    }

    // The main part of a split tile.
    MouseArea {
        visible: t.split && !t.compact
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.right: more.left
        cursorShape: Qt.PointingHandCursor
        onClicked: t.main()
    }

    Rectangle {
        id: more
        visible: t.split && !t.compact
        anchors.right: parent.right
        width: 28
        height: parent.height
        radius: Style.radiusCard
        color: moreArea.containsMouse ? Style.alpha(Style.text, 0.05) : "transparent"
        Rectangle {
            width: 1
            height: parent.height
            color: t.on ? Style.alpha(t.ctx.swatch(t.swatch), 0.25) : Style.divider
        }
        Icon {
            anchors.centerIn: parent
            name: "go-next-symbolic"
            size: 14
            recolor: true
            color: moreArea.containsMouse ? Style.text : Style.dimText
        }
        MouseArea {
            id: moreArea
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: t.more()
        }
        Tip {
            target: more
            text: t.moreTip
            shown: moreArea.containsMouse
        }
    }
}
