import QtQuick
import Nyx.Ui

// A row in a detail view: icon, title and sub-line, trailing items; selected rows in
// an accent tint with the icon in the accent.
Rectangle {
    id: r

    property string icon: ""
    property string title: ""
    property string sub: ""
    property bool selected: false
    property bool clickable: true
    property bool hovered: area.containsMouse
    default property alias trailing: tail.data

    signal clicked

    width: parent ? parent.width : 300
    implicitHeight: Math.max(36, texts.implicitHeight + 10)
    radius: Style.radiusSmall
    color: selected ? Style.alpha(Style.accent, 0.16) : (clickable && hovered ? Style.alpha(Style.text, 0.04) : "transparent")

    MouseArea {
        id: area
        anchors.fill: parent
        enabled: r.clickable
        hoverEnabled: true
        cursorShape: r.clickable ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: r.clicked()
    }
    Icon {
        id: ic
        visible: r.icon !== ""
        x: 8
        anchors.verticalCenter: parent.verticalCenter
        name: r.icon
        size: 16
        recolor: true
        color: r.selected ? Style.accent : Style.dimText
    }
    Column {
        id: texts
        anchors.left: ic.visible ? ic.right : parent.left
        anchors.leftMargin: ic.visible ? 10 : 8
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
        anchors.right: parent.right
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        spacing: 4
    }
}
