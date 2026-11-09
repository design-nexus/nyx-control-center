import QtQuick
import Nyx.Ui

// One fact in a detail view: name and value.
Item {
    property string name: ""
    property string value: ""
    width: parent ? parent.width : 300
    height: 28
    Label {
        x: 8
        anchors.verticalCenter: parent.verticalCenter
        text: parent.name
        dim: true
    }
    Label {
        anchors.right: parent.right
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        text: parent.value
        font.features: { "tnum": 1 }
    }
}
