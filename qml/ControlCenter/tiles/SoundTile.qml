import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

// Sound & brightness: the volume row (the Sound page behind its icon, mute at the end)
// and the brightness row (Screens behind its icon), one row tall when no screen's
// brightness can be set.
TileFrame {
    id: t
    property int cols: 4
    property var tile
    readonly property var au: audio.value || ({ outputs: [] })
    readonly property var br: bright.value || ({})
    readonly property var out: (au.outputs || []).find(o => o.default) || null
    BackendTopic { id: audio; client: t.ctx.client; name: "audio"; active: !!(t.ctx && t.ctx.open && t.visible) }
    BackendTopic { id: bright; client: t.ctx.client; name: "brightness"; active: !!(t.ctx && t.ctx.open && t.visible) }

    signal volumeMenu(var anchor)

    Column {
        anchors.fill: parent
        anchors.leftMargin: 6
        anchors.rightMargin: 8
        anchors.topMargin: 8
        anchors.bottomMargin: 8
        SliderRow {
            id: vol
            width: parent.width
            height: t.br.screen || t.ctx.editing ? parent.height / 2 : parent.height
            icon: t.au.muted ? "audio-volume-muted-symbolic" : t.au.volume < 0.34 ? "audio-volume-low-symbolic" : t.au.volume < 0.67 ? "audio-volume-medium-symbolic" : "audio-volume-high-symbolic"
            iconTip: "Sound: outputs, each app's volume and the microphone"
            tint: t.ctx.swatch("blue")
            to: 1.5
            value: t.au.volume || 0
            valueText: t.au.muted ? "Off" : Math.round((t.au.volume || 0) * 100) + "%"
            loud: !t.au.muted && Math.round((t.au.volume || 0) * 100) > 100
            endIcon: ({ headphones: "audio-headphones-symbolic", hdmi: "video-display-symbolic", bluetooth: "bluetooth-symbolic" })[t.out ? t.out.kind : ""] || "audio-speakers-symbolic"
            endTip: t.au.muted ? (t.out ? t.out.description : "the speakers") + " is muted. Click to unmute" : "Mute " + (t.out ? t.out.description : "the speakers")
            endOn: !!t.au.muted
            debounce: 40
            onMoved: v => t.ctx.call("audio.set_volume", { value: v })
            onIconClicked: t.ctx.openDetail("sound")
            onEndClicked: t.ctx.call("audio.set_muted", { on: !t.au.muted })
        }
        SliderRow {
            width: parent.width
            height: parent.height / 2
            visible: !!t.br.screen
            icon: "display-brightness-symbolic"
            iconTip: "Screens: brightness of each, scale, text size"
            tint: t.ctx.swatch("amber")
            from: 0.01
            to: 1
            value: t.br.screen ? Math.max(0.01, t.br.screen.value) : 0
            valueText: t.br.screen ? Math.round(t.br.screen.value * 100) + "%" : ""
            endIcon: "video-display-symbolic"
            endTip: "Screens: brightness of each, scale, text size"
            debounce: 60
            onMoved: v => t.ctx.call("brightness.set_screen", { value: v })
            onIconClicked: t.ctx.openDetail("display")
            onEndClicked: t.ctx.openDetail("display")
        }
    }
}
