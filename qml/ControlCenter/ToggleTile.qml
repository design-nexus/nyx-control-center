import QtQuick
import Nyx.Ui

// A toggle tile (1×1: its icon, centred; wider: icon, name and status) or, with
// `split`, a split tile: the main part acts, the arrow opens the detail view.
TileFrame {
    id: t

    property string icon: ""
    property string title: ""
    property string status: ""
    property bool active: false
    property bool split: false
    property bool compact: width < 100
    // A word under a one-cell tile's icon (the workspace mode's name).
    property string compactLabel: ""
    property string moreTip: title + ": networks, devices and settings"

    signal main
    signal more

    clickable: !split || compact
    on: split && active && !compact
    onClicked: t.main()

    // A one-cell tile is its icon: the whole tile takes the swatch when on, with a
    // larger glyph, rather than a small chip inside a card.
    readonly property color tintColor: t.ctx ? t.ctx.swatch(t.swatch) : Style.accent
    color: compact ? (active ? (hovered ? Style.mix(tintColor, Style.text, 0.12) : tintColor) : (hovered ? Style.mix(Style.card, Style.text, 0.04) : Style.card)) : on && swatch !== "" && ctx ? Style.mix(Style.card, tintColor, tintAmount) : (clickable && hovered ? Style.mix(Style.card, Style.text, 0.04) : Style.card)
    Icon {
        id: glyph
        visible: t.compact
        anchors.centerIn: parent
        anchors.verticalCenterOffset: t.compactLabel !== "" ? -7 : 0
        name: t.icon
        size: 22
        recolor: true
        color: t.active ? Style.bg : Style.text
    }

    Label {
        visible: t.compact && t.compactLabel !== ""
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: glyph.bottom
        anchors.topMargin: 3
        width: parent.width - 8
        horizontalAlignment: Text.AlignHCenter
        elide: Text.ElideRight
        text: t.compactLabel
        size: 10
        color: t.active ? Style.bg : Style.text
    }

    Chip {
        id: chip
        visible: !t.compact
        anchors.verticalCenter: parent.verticalCenter
        x: 9
        icon: t.icon
        on: t.active
        tint: t.tintColor
    }

    Column {
        visible: !t.compact
        anchors.left: chip.right
        anchors.leftMargin: t.split ? 8 : 10
        anchors.right: t.split ? more.left : parent.right
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        Label {
            width: parent.width
            text: t.title
            size: 12.5
            font.weight: Font.Medium
        }
        Label {
            width: parent.width
            visible: text !== ""
            text: t.status
            size: 11.5
            dim: true
        }
    }

    // The main part of a split tile.
    MouseArea {
        visible: t.split && !t.compact
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.right: more.left
        cursorShape: Qt.PointingHandCursor
        onClicked: t.main()
    }

    Rectangle {
        id: more
        visible: t.split && !t.compact
        anchors.right: parent.right
        width: 28
        height: parent.height
        radius: Style.radiusCard
        color: moreArea.containsMouse ? Style.alpha(Style.text, 0.05) : "transparent"
        Rectangle {
            width: 1
            height: parent.height
            color: t.on ? Style.alpha(t.ctx.swatch(t.swatch), 0.25) : Style.divider
        }
        Icon {
            anchors.centerIn: parent
            name: "go-next-symbolic"
            size: 14
            recolor: true
            color: moreArea.containsMouse ? Style.text : Style.dimText
        }
        MouseArea {
            id: moreArea
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: t.more()
        }
        Tip {
            target: more
            text: t.moreTip
            shown: moreArea.containsMouse
        }
    }
}
