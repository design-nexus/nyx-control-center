import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

// Sound: the volume and where it plays, each app's volume, and the microphone (the
// volume popover's rows as a page). The head's switch mutes the output.
Column {
    id: v
    property var ctx
    signal back
    readonly property var st: audio.value || ({ outputs: [], streams: [], inputs: [], recording: [] })
    readonly property var out: (st.outputs || []).find(o => o.default) || null
    BackendTopic { id: audio; client: v.ctx.client; name: "audio"; active: !!(v.ctx && v.ctx.open) }

    function kindIcon(kind) {
        return ({ headphones: "audio-headphones-symbolic", hdmi: "video-display-symbolic", bluetooth: "bluetooth-symbolic" })[kind] || "audio-speakers-symbolic";
    }

    DetailHead {
        title: "Sound"
        checked: !v.st.muted
        onToggled: on => v.ctx.call("audio.set_muted", { on: !on })
        onBack: v.back()
    }
    Label {
        width: parent.width
        visible: v.st.available === false
        horizontalAlignment: Text.AlignHCenter
        topPadding: 40
        bottomPadding: 40
        text: "No sound devices"
        dim: true
    }
    CardBox {
        visible: v.st.available !== false
        SliderRow {
            width: parent.width
            height: 40
            icon: v.st.muted ? "audio-volume-muted-symbolic" : v.st.volume < 0.34 ? "audio-volume-low-symbolic" : v.st.volume < 0.67 ? "audio-volume-medium-symbolic" : "audio-volume-high-symbolic"
            iconTip: v.st.muted ? (v.out ? v.out.description : "The speakers") + " is muted. Click to unmute" : "Mute " + (v.out ? v.out.description : "the speakers")
            tint: v.ctx.swatch("blue")
            to: 1.5
            value: v.st.volume || 0
            valueText: v.st.muted ? "Off" : Math.round((v.st.volume || 0) * 100) + "%"
            loud: !v.st.muted && Math.round((v.st.volume || 0) * 100) > 100
            debounce: 40
            onMoved: x => v.ctx.call("audio.set_volume", { value: x })
            onIconClicked: v.ctx.call("audio.set_muted", { on: !v.st.muted })
        }
    }
    Column {
        width: parent.width
        visible: (v.st.outputs || []).length > 0
        Group {
            text: "Output"
        }
        CardBox {
            Repeater {
                model: v.st.outputs || []
                delegate: CcRow {
                    required property var modelData
                    icon: v.kindIcon(modelData.kind)
                    title: modelData.description
                    selected: modelData.default
                    onClicked: v.ctx.call("audio.set_output", { name: modelData.name })
                }
            }
        }
    }
    Column {
        width: parent.width
        visible: (v.st.streams || []).length > 0
        Group {
            text: "Apps"
        }
        CardBox {
            Repeater {
                model: v.st.streams || []
                delegate: Column {
                    required property var modelData
                    width: parent.width
                    Label {
                        leftPadding: 8
                        topPadding: 4
                        text: modelData.app || modelData.binary
                        role: "secondary"
                        dim: true
                    }
                    SliderRow {
                        width: parent.width
                        height: 34
                        icon: modelData.icon || String(modelData.binary || "").toLowerCase() || "application-x-executable"
                        to: 1.5
                        value: modelData.volume
                        valueText: Math.round(modelData.volume * 100) + "%"
                        debounce: 40
                        onMoved: x => v.ctx.call("audio.set_stream_volume", { index: modelData.index, value: x })
                    }
                }
            }
        }
    }
    Column {
        width: parent.width
        visible: v.st.available !== false
        Group {
            text: "Microphone"
        }
        CardBox {
            SliderRow {
                width: parent.width
                height: 38
                icon: v.st.mic_muted ? "microphone-sensitivity-muted-symbolic" : "audio-input-microphone-symbolic"
                iconTip: v.st.mic_muted ? "The microphone is muted. Click to unmute" : "Mute the microphone"
                tint: v.st.mic_muted ? Style.muted : Style.accent
                to: 1.5
                value: v.st.mic_volume || 0
                valueText: v.st.mic_muted ? "Off" : Math.round((v.st.mic_volume || 0) * 100) + "%"
                debounce: 40
                onMoved: x => v.ctx.call("audio.set_mic_volume", { value: x })
                onIconClicked: v.ctx.call("audio.set_mic_muted", { on: !v.st.mic_muted })
            }
            Repeater {
                model: (v.st.inputs || []).length > 1 ? v.st.inputs : []
                delegate: CcRow {
                    required property var modelData
                    icon: "audio-input-microphone-symbolic"
                    title: modelData.description
                    selected: modelData.default
                    onClicked: v.ctx.call("audio.set_input", { name: modelData.name })
                }
            }
        }
        Row {
            visible: (v.st.recording || []).length > 0
            leftPadding: 8
            topPadding: 8
            spacing: 6
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: 8
                height: 8
                radius: 4
                color: Style.danger
            }
            Label {
                text: "Recording now: " + (v.st.recording || []).join(", ")
                role: "secondary"
                color: Style.danger
            }
        }
    }
    Foot {
        IconButton {
            icon: "nyx-settings-symbolic"
            tooltip: "Sound settings"
            onClicked: {
                v.ctx.hide();
                v.ctx.call("open_settings", { page: "audio" });
            }
        }
    }
}
