import QtQuick
import Nyx.Ui

// A toggle tile (1×1: its round icon over its name; wider: icon, name and status) or,
// with `split`, a split tile: the main part acts, the chevron opens the detail view.
TileFrame {
    id: t

    property string icon: ""
    property string title: ""
    property string status: ""
    property bool active: false
    property bool split: false
    property bool compact: width < 100
    // The word under a one-cell tile's icon in place of its name (the workspace mode's).
    property string compactLabel: ""
    property string moreTip: title + ": networks, devices and settings"
    // The icon's colour while off (Battery's green or red).
    property color iconInk: Style.text
    // A chevron at the right of a wide tile that isn't split: the whole tile opens a page.
    property bool chevron: false
    // A one-cell tile's icon and name need this much height.
    readonly property real need: compact ? 66 : 0

    signal main
    signal more

    clickable: !split || compact
    on: active && (compact || split)
    onClicked: t.main()
    // A wide split tile isn't one button, but Tab still reaches it: Enter acts, Right or
    // the Menu key opens its detail view.
    activeFocusOnTab: true
    Keys.onPressed: e => {
        if (t.split && !t.compact && (e.key === Qt.Key_Right || e.key === Qt.Key_Menu)) {
            t.more();
            e.accepted = true;
        }
    }
    Accessible.name: t.status !== "" ? t.title + ", " + t.status : t.title
    Accessible.checkable: true
    Accessible.checked: t.active

    readonly property color tintColor: t.ctx ? t.ctx.swatch(t.swatch) : Style.accent
    color: on && swatch !== "" && ctx ? Style.mix(Style.card, tintColor, hovered && clickable ? tintAmount + 0.05 : tintAmount) : (clickable && hovered ? Style.mix(Style.card, Style.text, 0.04) : Style.card)

    Chip {
        id: glyph
        visible: t.compact
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.topMargin: Math.max(6, (parent.height - height - name.implicitHeight - 5) / 2)
        icon: t.icon
        on: t.active
        tint: t.tintColor
        ink: t.iconInk
    }

    Label {
        id: name
        visible: t.compact
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: glyph.bottom
        anchors.topMargin: 5
        width: parent.width - 8
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        maximumLineCount: 2
        elide: Text.ElideRight
        text: t.compactLabel !== "" ? t.compactLabel : t.title
        size: 10.5
        lineHeight: 0.95
    }

    Chip {
        id: chip
        visible: !t.compact
        anchors.verticalCenter: parent.verticalCenter
        x: 9
        icon: t.icon
        on: t.active
        tint: t.tintColor
        ink: t.iconInk
    }

    Column {
        visible: !t.compact
        anchors.left: chip.right
        anchors.leftMargin: 10
        anchors.right: t.split ? more.left : chev.visible ? chev.left : parent.right
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

    Icon {
        id: chev
        visible: t.chevron && !t.split && !t.compact
        anchors.right: parent.right
        anchors.rightMargin: 9
        anchors.verticalCenter: parent.verticalCenter
        name: "pan-end-symbolic"
        size: 14
        recolor: true
        color: t.hovered ? Style.text : Style.dimText
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
            name: "pan-end-symbolic"
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
