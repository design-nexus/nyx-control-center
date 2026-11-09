import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

// Home: a room's lights and devices from Nyx Home, with its scenes. Click switches a
// device; right-click a light for brightness and colour.
TileFrame {
    id: t
    property int cols: 4
    property var tile
    property int page: 0
    property int index: 0
    readonly property var st: home.value || ({ available: false, rooms: [], devices: [], scenes: [] })
    readonly property int columns: cols >= 4 ? 4 : 2
    readonly property string room: {
        const saved = t.tile && t.tile.options ? t.tile.options.room : undefined;
        return (saved && st.rooms.indexOf(saved) >= 0) ? saved : (st.rooms.length ? st.rooms[0] : "");
    }
    readonly property var devices: {
        const order = d => d.domain === "light" ? 0 : (d.domain === "switch" || d.domain === "fan") ? 1 : 2;
        return st.devices.filter(d => room === "" || d.room === room).sort((a, b) => order(a) - order(b)).slice(0, columns * 2);
    }
    readonly property var scenes: {
        const here = st.scenes.filter(s => room !== "" && s.room === room);
        return (here.length ? here : st.scenes.filter(s => !s.room)).slice(0, columns === 4 ? 5 : 2);
    }
    BackendTopic { id: home; client: t.ctx.client; name: "home"; active: !!(t.ctx && t.ctx.open && t.visible) }

    function status(d) {
        if (!d.available)
            return "Unavailable";
        if (d.domain === "climate") {
            if (d.target !== null && d.target !== undefined)
                return d.target.toFixed(1) + "°" + (d.current !== null && d.current !== undefined ? " · now " + d.current.toFixed(1) + "°" : "");
            return d.on ? "On" : "Off";
        }
        if (!d.on)
            return "Off";
        return d.brightness !== null && d.brightness !== undefined ? Math.round(d.brightness * 100) + "%" : "On";
    }
    function tint(d) {
        return d.color ? Qt.rgba(d.color[0] / 255, d.color[1] / 255, d.color[2] / 255, 1) : Style.warning;
    }

    Row {
        id: head
        x: 10
        y: 8
        width: parent.width - 20
        spacing: 6
        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: "go-home-symbolic"
            size: 16
            recolor: true
            color: Style.success
        }
        Rectangle {
            id: roomButton
            anchors.verticalCenter: parent.verticalCenter
            height: 26
            width: roomLabel.implicitWidth + 26
            radius: Style.radiusSmall
            color: roomArea.containsMouse ? Style.alpha(Style.text, 0.06) : "transparent"
            Label {
                id: roomLabel
                x: 6
                anchors.verticalCenter: parent.verticalCenter
                text: t.st.available ? (t.room || "Home") : "Home"
                font.weight: Font.Medium
            }
            Icon {
                anchors.right: parent.right
                anchors.rightMargin: 4
                anchors.verticalCenter: parent.verticalCenter
                name: "pan-down-symbolic"
                size: 12
                recolor: true
                color: Style.dimText
            }
            MouseArea {
                id: roomArea
                anchors.fill: parent
                hoverEnabled: true
                enabled: t.st.available
                cursorShape: Qt.PointingHandCursor
                onClicked: rooms.openAt(0, roomButton.height + 4)
            }
            Menu {
                id: rooms
                anchorItem: roomButton
                entries: t.st.rooms.map(r => ({ id: r, label: r, checked: r === t.room }))
                onTriggered: id => t.ctx.call("layout.set_option", { page: t.page, index: t.index, key: "room", value: id })
            }
        }
        Item {
            width: head.width - 16 - roomButton.width - 32 - 18
            height: 1
        }
        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            icon: "go-next-symbolic"
            tooltip: "Open Home"
            onClicked: {
                t.ctx.hide();
                t.ctx.call("open_app", { id: "io.github.design_nexus.nyx.Home" });
            }
        }
    }

    Label {
        anchors.centerIn: parent
        width: parent.width - 24
        visible: !t.st.available
        wrapMode: Text.Wrap
        horizontalAlignment: Text.AlignHCenter
        text: t.st.problem || "Connect Home Assistant in Home"
        size: 11.5
        dim: true
    }

    Grid {
        id: grid
        visible: t.st.available
        anchors.top: head.bottom
        anchors.topMargin: 6
        x: 10
        width: parent.width - 20
        columns: t.columns
        spacing: 6
        Repeater {
            model: t.devices
            delegate: Rectangle {
                id: cell
                required property var modelData
                width: (grid.width - (t.columns - 1) * 6) / t.columns
                height: 54
                radius: Style.radiusSmall
                opacity: modelData.available ? 1 : 0.5
                color: modelData.on ? Style.mix(Style.card, t.tint(modelData), 0.2) : Style.alpha(Style.text, 0.05)
                Column {
                    x: 8
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - 16
                    Icon {
                        name: ({ light: "display-brightness-symbolic", fan: "weather-windy-symbolic", climate: "weather-few-clouds-symbolic" })[cell.modelData.domain] || "system-shutdown-symbolic"
                        size: 15
                        recolor: true
                        color: cell.modelData.on ? t.tint(cell.modelData) : Style.dimText
                    }
                    Label {
                        width: parent.width
                        text: cell.modelData.name
                        size: 11.5
                        font.weight: Font.Medium
                    }
                    Label {
                        width: parent.width
                        text: t.status(cell.modelData)
                        role: "caption"
                        dim: true
                    }
                }
                MouseArea {
                    id: cellArea
                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    cursorShape: Qt.PointingHandCursor
                    onClicked: m => {
                        if (m.button === Qt.RightButton && (cell.modelData.dimmable || cell.modelData.colorable))
                            light.openAt(0, cell.height + 4);
                        else if (m.button === Qt.LeftButton && cell.modelData.domain !== "climate")
                            t.ctx.call("home.toggle", { id: cell.modelData.id });
                    }
                }
                Tip {
                    target: cell
                    text: cell.modelData.dimmable || cell.modelData.colorable ? cell.modelData.name + " (right-click for brightness)" : cell.modelData.name
                    shown: cellArea.containsMouse
                }
                LightPopover {
                    id: light
                    anchorItem: cell
                    ctx: t.ctx
                    device: cell.modelData
                }
            }
        }
    }

    Row {
        visible: t.st.available
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 10
        x: 10
        width: parent.width - 20
        spacing: 6
        Repeater {
            model: t.scenes
            delegate: Button {
                required property var modelData
                width: (parent.width - (t.scenes.length - 1) * 6) / Math.max(1, t.scenes.length)
                text: modelData.name
                onClicked: t.ctx.call("home.call", { entity: modelData.id, service: "turn_on", data: null })
            }
        }
    }
}
