import QtQuick
import Nyx.Ui

// The round chip holding an icon, one size for every tile (page rows set a smaller
// one): the tile's swatch when on, a faint disc off with the icon in `ink`.
Rectangle {
    property string icon: ""
    property bool on: false
    property color tint: Style.accent
    property color ink: Style.text
    property int iconSize: 17

    implicitWidth: 32
    implicitHeight: 32
    width: implicitWidth
    height: implicitHeight
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
        color: parent.on ? Style.onAccent : parent.ink
    }
}
