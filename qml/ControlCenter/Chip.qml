import QtQuick
import Nyx.Ui

// The round chip holding a tile's icon: the tile's swatch when on, a faint disc off.
Rectangle {
    property string icon: ""
    property bool on: false
    property color tint: Style.accent
    property int iconSize: 16

    implicitWidth: 30
    implicitHeight: 30
    radius: width / 2
    color: on ? tint : Style.alpha(Style.text, 0.09)
    Behavior on color {
        ColorAnimation {
            duration: Style.fast
        }
    }
    Icon {
        anchors.centerIn: parent
        name: parent.icon
        size: parent.iconSize
        recolor: true
        color: parent.on ? Style.onAccent : Style.text
    }
}
