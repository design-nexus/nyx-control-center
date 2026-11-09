import QtQuick
import Nyx.Ui

// The 30 px rounded square holding a tile's icon; the tile's swatch when on.
Rectangle {
    property string icon: ""
    property bool on: false
    property color tint: Style.accent
    property int iconSize: 17

    implicitWidth: 30
    implicitHeight: 30
    radius: Style.radiusSmall
    color: on ? tint : Style.alpha(Style.text, 0.1)
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
        color: parent.on ? Style.bg : Style.text
    }
}
