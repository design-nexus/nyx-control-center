import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

// Sound & brightness: each a small caps label with a chip at the right (the output's
// name, opening the Sound page; Screens, opening Display) over its slider. The volume
// icon mutes. One section when no screen's brightness can be set.
TileFrame {
    id: t
    property int cols: 4
    property var tile
    readonly property var au: audio.value || ({ outputs: [] })
    readonly property var br: bright.value || ({})
    readonly property var out: (au.outputs || []).find(o => o.default) || null
    readonly property bool both: !!br.screen || t.ctx.editing
    BackendTopic { id: audio; client: t.ctx.client; name: "audio"; active: !!(t.ctx && t.ctx.open && t.visible) }
    BackendTopic { id: bright; client: t.ctx.client; name: "brightness"; active: !!(t.ctx && t.ctx.open && t.visible) }

    signal volumeMenu(var anchor)

    function kindIcon(kind) {
        return ({ headphones: "audio-headphones-symbolic", hdmi: "video-display-symbolic", bluetooth: "bluetooth-symbolic" })[kind] || "audio-speakers-symbolic";
    }

    // A section's head: its name, and a chip that opens its page.
    component Head: Item {
        id: h
        property string text: ""
        property string chipIcon: ""
        property string chipText: ""
        property string chipTip: ""
        signal opened
        width: parent ? parent.width : 300
        height: 24
        Label {
            x: 6
            anchors.verticalCenter: parent.verticalCenter
            text: h.text
            role: "group"
            color: Style.tertiary
        }
        Rectangle {
            id: chip
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            height: 22
            // The icon, the name and the chevron with the gaps between them and 8 px at
            // each end; a long name ends in "…" past 60 % of the tile.
            readonly property real fixed: chipGlyph.width + chev.width + 2 * row.spacing + 16
            width: Math.min(h.width * 0.6, fixed + chipLabel.implicitWidth)
            radius: 11
            color: chipArea.containsMouse ? Style.alpha(Style.text, 0.12) : Style.alpha(Style.text, 0.07)
            Row {
                id: row
                anchors.verticalCenter: parent.verticalCenter
                x: 8
                spacing: 5
                Icon {
                    id: chipGlyph
                    anchors.verticalCenter: parent.verticalCenter
                    name: h.chipIcon
                    size: 13
                    recolor: true
                    color: Style.dimText
                }
                Label {
                    id: chipLabel
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.min(implicitWidth, chip.width - chip.fixed)
                    text: h.chipText
                    size: 11.5
                    dim: true
                }
                Icon {
                    id: chev
                    anchors.verticalCenter: parent.verticalCenter
                    name: "pan-end-symbolic"
                    size: 11
                    recolor: true
                    color: Style.tertiary
                }
            }
            MouseArea {
                id: chipArea
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: h.opened()
            }
            Tip {
                target: chip
                text: h.chipTip
                shown: chipArea.containsMouse
            }
        }
    }

    Column {
        anchors.fill: parent
        anchors.leftMargin: 6
        anchors.rightMargin: 8
        anchors.topMargin: 8
        anchors.bottomMargin: 6
        Head {
            text: "Sound"
            chipIcon: t.kindIcon(t.out ? t.out.kind : "")
            chipText: t.out ? t.out.description : "Outputs"
            chipTip: "Sound: outputs, each app's volume and the microphone"
            onOpened: t.ctx.openDetail("sound")
        }
        SliderRow {
            id: vol
            width: parent.width
            height: (parent.height - (t.both ? 48 : 24)) / (t.both ? 2 : 1)
            icon: t.au.muted ? "audio-volume-muted-symbolic" : t.au.volume < 0.34 ? "audio-volume-low-symbolic" : t.au.volume < 0.67 ? "audio-volume-medium-symbolic" : "audio-volume-high-symbolic"
            iconTip: t.au.muted ? (t.out ? t.out.description : "The speakers") + " is muted. Click to unmute" : "Mute " + (t.out ? t.out.description : "the speakers")
            tint: t.ctx.swatch("blue")
            to: 1.5
            value: t.au.volume || 0
            valueText: t.au.muted ? "Off" : Math.round((t.au.volume || 0) * 100) + "%"
            loud: !t.au.muted && Math.round((t.au.volume || 0) * 100) > 100
            debounce: 40
            onMoved: v => t.ctx.call("audio.set_volume", { value: v })
            onIconClicked: t.ctx.call("audio.set_muted", { on: !t.au.muted })
        }
        Head {
            visible: t.both
            text: "Brightness"
            chipIcon: "video-display-symbolic"
            chipText: "Screens"
            chipTip: "Screens: brightness of each, scale, text size"
            onOpened: t.ctx.openDetail("display")
        }
        SliderRow {
            width: parent.width
            height: vol.height
            visible: t.both
            icon: "display-brightness-symbolic"
            iconTip: "Screens: brightness of each, scale, text size"
            tint: t.ctx.swatch("amber")
            from: 0.01
            to: 1
            value: t.br.screen ? Math.max(0.01, t.br.screen.value) : 0
            valueText: t.br.screen ? Math.round(t.br.screen.value * 100) + "%" : ""
            debounce: 60
            onMoved: v => t.ctx.call("brightness.set_screen", { value: v })
            onIconClicked: t.ctx.openDetail("display")
        }
    }
}
