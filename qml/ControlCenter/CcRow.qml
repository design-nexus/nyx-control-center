import QtQuick
import Nyx.Ui

// A row in a detail view: its icon in a disc, title and sub-line, trailing items. A
// selected row is tinted in the accent, its disc filled with it, and ends in a tick.
Rectangle {
    id: r

    property string icon: ""
    property string title: ""
    property string sub: ""
    property bool selected: false
    property bool clickable: true
    property bool hovered: area.containsMouse
    // The tick on a selected row: a choice among rows, not a connected device with its
    // own buttons.
    property bool tick: clickable
    default property alias trailing: tail.data

    signal clicked

    width: parent ? parent.width : 300
    implicitHeight: Math.max(40, texts.implicitHeight + 12)
    radius: Style.radiusSmall
    color: selected ? Style.alpha(Style.accent, hovered && clickable ? 0.2 : 0.14) : (clickable && hovered ? Style.alpha(Style.text, 0.05) : "transparent")
    Behavior on color {
        ColorAnimation {
            duration: Style.fast
        }
    }

    MouseArea {
        id: area
        anchors.fill: parent
        enabled: r.clickable
        hoverEnabled: true
        cursorShape: r.clickable ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: r.clicked()
    }
    Chip {
        id: disc
        visible: r.icon !== ""
        x: 6
        anchors.verticalCenter: parent.verticalCenter
        implicitWidth: 28
        implicitHeight: 28
        iconSize: 15
        icon: r.icon
        on: r.selected
        ink: Style.dimText
    }
    Column {
        id: texts
        anchors.left: disc.visible ? disc.right : parent.left
        anchors.leftMargin: disc.visible ? 10 : 8
        anchors.right: tail.left
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        Label {
            width: parent.width
            text: r.title
            font.weight: r.selected ? Font.Medium : Font.Normal
        }
        Label {
            width: parent.width
            visible: r.sub !== ""
            text: r.sub
            role: "secondary"
            dim: true
        }
    }
    Row {
        id: tail
        anchors.right: check.visible ? check.left : parent.right
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        spacing: 4
    }
    Icon {
        id: check
        visible: r.selected && r.tick
        anchors.right: parent.right
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        name: "object-select-symbolic"
        size: 14
        recolor: true
        color: Style.accent
    }
}
