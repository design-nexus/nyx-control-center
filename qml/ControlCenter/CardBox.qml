import QtQuick
import Nyx.Ui

// A card of rows in a detail view: `@theme_card`, radius 11, 6 px inside.
Rectangle {
    default property alias rows: col.data
    property alias spacing: col.spacing

    width: parent ? parent.width : 300
    implicitHeight: col.implicitHeight + 12
    radius: Style.radiusCard
    color: Style.card
    Column {
        id: col
        x: 6
        y: 6
        width: parent.width - 12
    }
}
