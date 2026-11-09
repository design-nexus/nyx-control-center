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
    color: on && swatch !== "" && ctx ? Style.mix(Style.card, ctx.swatch(swatch), tintAmount) : (clickable && hovered ? Style.mix(Style.card, Style.text, 0.04) : Style.card)
    Behavior on color {
        ColorAnimation {
            duration: Style.fast
        }
    }

    HoverHandler {
        id: hover
    }
    TapHandler {
        enabled: f.clickable
        onTapped: f.clicked()
    }
    Tip {
        target: f
        text: f.tooltip
        shown: f.hovered && f.tooltip !== ""
    }
}
