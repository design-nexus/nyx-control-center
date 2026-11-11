import QtQuick
import Nyx.Ui

// One fact in a detail view. In a FactGrid, a small card: the name above the value; on
// its own, a row with the name at the left and the value at the right.
Rectangle {
    id: f
    property string name: ""
    property string value: ""
    readonly property bool inGrid: !!(parent && parent.factGrid)

    width: inGrid ? (parent.width - parent.spacing) / 2 : (parent ? parent.width : 300)
    height: inGrid ? 48 : 28
    radius: Style.radiusSmall
    color: inGrid ? Style.card : "transparent"

    Label {
        x: f.inGrid ? 10 : 8
        y: f.inGrid ? 8 : 0
        height: f.inGrid ? implicitHeight : f.height
        width: f.inGrid ? f.width - 20 : implicitWidth
        text: f.name
        size: f.inGrid ? 11 : Style.body
        color: f.inGrid ? Style.tertiary : Style.dimText
    }
    Label {
        x: f.inGrid ? 10 : f.width - width - 8
        y: f.inGrid ? 25 : 0
        height: f.inGrid ? implicitHeight : f.height
        width: f.inGrid ? f.width - 20 : implicitWidth
        text: f.value
        font.weight: f.inGrid ? Font.DemiBold : Font.Normal
        font.features: { "tnum": 1 }
    }
}
