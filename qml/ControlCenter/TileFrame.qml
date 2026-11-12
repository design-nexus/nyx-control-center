import QtQuick
import Nyx.Ui

// A dashboard tile: `@theme_card`, radius 11; hover lightens it when clickable. `on`
// with a swatch tints it as the split tiles do when on.
Rectangle {
    id: f

    property bool clickable: false
    property bool on: false
    property string swatch: ""
    property var ctx
    property string tooltip: ""
    property bool hovered: hover.hovered
    property real tintAmount: 0.13

    signal clicked

    radius: Style.radiusCard
    // Tab reaches a tile that acts; Enter or Space acts, as a click does.
    activeFocusOnTab: clickable
    Keys.onReturnPressed: f.clicked()
    Keys.onEnterPressed: f.clicked()
    Keys.onSpacePressed: f.clicked()
    Accessible.role: Accessible.Button
    Accessible.name: f.tooltip
    Accessible.onPressAction: f.clicked()
    border.width: activeFocus ? 2 : 0
    border.color: Style.accent
    color: on && swatch !== "" && ctx ? Style.mix(Style.card, ctx.swatch(swatch), tintAmount) : (clickable && hovered ? Style.mix(Style.card, Style.text, 0.04) : Style.card)
    Behavior on color {
        ColorAnimation {
            duration: Style.fast
        }
    }

    HoverHandler {
        id: hover
    }
    // A MouseArea, not a TapHandler: the first click on the panel also gives it the
    // keyboard, and the window turning active cancels a TapHandler's tap.
    MouseArea {
        anchors.fill: parent
        enabled: f.clickable
        onClicked: f.clicked()
    }
    Tip {
        target: f
        text: f.tooltip
        shown: f.hovered && f.tooltip !== ""
    }
}
