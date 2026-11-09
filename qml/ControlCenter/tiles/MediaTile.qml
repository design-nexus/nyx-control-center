import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

// Now playing: the cover, what it is with a bar to drag, and big controls.
TileFrame {
    id: t
    property int cols: 4
    property var tile
    readonly property var st: media.value || ({ player: "" })
    readonly property bool nothing: !st.player
    BackendTopic { id: media; client: t.ctx.client; name: "media"; active: t.ctx.open && t.visible }

    function clock(us) {
        const s = Math.max(0, Math.floor(us / 1000000));
        const two = n => (n < 10 ? "0" : "") + n;
        return s >= 3600 ? Math.floor(s / 3600) + ":" + two(Math.floor(s / 60) % 60) + ":" + two(s % 60) : Math.floor(s / 60) + ":" + two(s % 60);
    }

    Rectangle {
        id: art
        x: 14
        anchors.verticalCenter: parent.verticalCenter
        width: 80
        height: 80
        radius: Style.radiusSmall
        color: Style.alpha(Style.text, 0.06)
        clip: true
        Image {
            anchors.fill: parent
            visible: !!t.st.art
            source: t.st.art || ""
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
        }
        Icon {
            anchors.centerIn: parent
            visible: !t.st.art
            name: "audio-x-generic-symbolic"
            size: 28
            recolor: true
            color: Style.dimText
        }
    }
    Column {
        anchors.left: art.right
        anchors.leftMargin: 12
        anchors.right: buttons.left
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        spacing: 1
        Label {
            width: parent.width
            text: t.nothing ? "Nothing playing" : (t.st.title || "Unknown")
            font.weight: Font.DemiBold
        }
        Label {
            width: parent.width
            readonly property var who: [t.st.artist || "", t.st.player_name || ""].filter(x => x !== "")
            visible: who.length > 0
            text: who.join(" · ")
            role: "secondary"
            dim: true
        }
        Row {
            visible: t.st.length > 0
            width: parent.width
            spacing: 6
            topPadding: 4
            Label {
                text: t.clock(seek.pressed ? seek.live * t.st.length : t.st.position_now || 0)
                role: "caption"
                dim: true
            }
            Item {
                width: parent.width - 80
                height: 14
                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width
                    height: 4
                    radius: 2
                    color: Style.alpha(Style.text, 0.1)
                    Rectangle {
                        height: parent.height
                        radius: 2
                        width: parent.width * Math.max(0, Math.min(1, seek.pressed ? seek.live : (t.st.length > 0 ? (t.st.position_now || 0) / t.st.length : 0)))
                        color: t.ctx.swatch("pink")
                    }
                }
                MouseArea {
                    id: seek
                    anchors.fill: parent
                    enabled: !!t.st.can_seek
                    property real live: 0
                    onPressed: m => live = Math.max(0, Math.min(1, m.x / width))
                    onPositionChanged: m => live = Math.max(0, Math.min(1, m.x / width))
                    onReleased: t.ctx.call("media.seek", { position: Math.round(live * t.st.length) })
                }
            }
            Label {
                text: t.clock(t.st.length || 0)
                role: "caption"
                dim: true
            }
        }
    }
    Row {
        id: buttons
        anchors.right: parent.right
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        spacing: 4
        enabled: !t.nothing
        opacity: enabled ? 1 : 0.45
        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            icon: "media-skip-backward-symbolic"
            tooltip: "Previous track"
            enabled: !!t.st.can_previous
            onClicked: t.ctx.call("media.command", { method: "Previous" })
        }
        Rectangle {
            width: 40
            height: 40
            radius: 20
            anchors.verticalCenter: parent.verticalCenter
            color: playArea.containsMouse ? Style.alpha(Style.text, 0.16) : Style.alpha(Style.text, 0.1)
            Icon {
                anchors.centerIn: parent
                name: t.st.playing ? "media-playback-pause-symbolic" : "media-playback-start-symbolic"
                size: 18
                recolor: true
                color: Style.text
            }
            MouseArea {
                id: playArea
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: t.ctx.call("media.command", { method: "PlayPause" })
            }
        }
        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            icon: "media-skip-forward-symbolic"
            tooltip: "Next track"
            enabled: !!t.st.can_next
            onClicked: t.ctx.call("media.command", { method: "Next" })
        }
    }
}
