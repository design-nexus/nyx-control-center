import QtQuick
import Nyx.Ui

Rectangle {
    id: c
    property var ctx
    property string icon: ""
    property string name: ""
    property var sizes: []

    signal add

    implicitHeight: col.implicitHeight + 20
    radius: Style.radiusCard
    color: Style.card
    Column {
        id: col
        x: 10
        y: 10
        width: parent.width - 20
        spacing: 6
        Item {
            width: parent.width
            height: 30
            Chip {
                icon: c.icon
            }
            IconButton {
                anchors.right: parent.right
                width: 28
                height: 28
                icon: "list-add-symbolic"
                tooltip: "Add " + c.name
                onClicked: c.add()
            }
        }
        Label {
            width: parent.width
            text: c.name
            size: 12.5
            font.weight: Font.Medium
        }
        Label {
            width: parent.width
            text: c.sizes.map(s => String(s).replace("x", "×")).join(" · ")
            size: 11.5
            dim: true
        }
    }
}
