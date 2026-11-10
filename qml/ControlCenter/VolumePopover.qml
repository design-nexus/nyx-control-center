import QtQuick
import Quickshell
import Nyx.Shell
import Nyx.Ui

// Sound: the outputs, each app's volume, and the microphone, under the volume icon.
PopupWindow {
    id: pop

    property var ctx
    property Item anchorItem: null
    readonly property bool keyboardOff: Quickshell.env("NYX_SHELL_NO_KEYBOARD") === "1"
    readonly property var st: audio.value || ({ outputs: [], streams: [], inputs: [], recording: [] })
    BackendTopic { id: audio; client: pop.ctx.client; name: "audio"; active: pop.visible }

    function openAt(x, y) {
        anchor.rect.x = x;
        anchor.rect.y = y;
        visible = true;
    }

    anchor.item: anchorItem
    anchor.adjustment: PopupAdjustment.All
    grabFocus: !keyboardOff
    color: "transparent"
    visible: false
    implicitWidth: 320
    implicitHeight: col.implicitHeight + 12

    Surface {
        anchors.fill: parent
        shadow: false
        Column {
            id: col
            x: 6
            y: 6
            width: parent.width - 12
            Group {
                text: "Output"
            }
            Repeater {
                model: pop.st.outputs || []
                delegate: CcRow {
                    required property var modelData
                    icon: ({ headphones: "audio-headphones-symbolic", hdmi: "video-display-symbolic", bluetooth: "bluetooth-symbolic" })[modelData.kind] || "audio-speakers-symbolic"
                    title: modelData.description
                    selected: modelData.default
                    onClicked: pop.ctx.call("audio.set_output", { name: modelData.name })
                }
            }
            Group {
                visible: (pop.st.streams || []).length > 0
                text: "Apps"
            }
            Repeater {
                model: pop.st.streams || []
                delegate: Column {
                    required property var modelData
                    width: col.width
                    Label {
                        leftPadding: 8
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
                        onMoved: x => pop.ctx.call("audio.set_stream_volume", { index: modelData.index, value: x })
                    }
                }
            }
            Group {
                text: "Microphone"
            }
            SliderRow {
                width: parent.width
                height: 34
                icon: pop.st.mic_muted ? "microphone-sensitivity-muted-symbolic" : "audio-input-microphone-symbolic"
                iconTip: pop.st.mic_muted ? "The microphone is muted. Click to unmute" : "Mute the microphone"
                to: 1.5
                value: pop.st.mic_volume || 0
                valueText: Math.round((pop.st.mic_volume || 0) * 100) + "%"
                debounce: 40
                onMoved: x => pop.ctx.call("audio.set_mic_volume", { value: x })
                onIconClicked: pop.ctx.call("audio.set_mic_muted", { on: !pop.st.mic_muted })
            }
            Repeater {
                model: (pop.st.inputs || []).length > 1 ? pop.st.inputs : []
                delegate: CcRow {
                    required property var modelData
                    icon: "audio-input-microphone-symbolic"
                    title: modelData.description
                    selected: modelData.default
                    onClicked: pop.ctx.call("audio.set_input", { name: modelData.name })
                }
            }
            Label {
                visible: (pop.st.recording || []).length > 0
                leftPadding: 8
                topPadding: 4
                text: "Recording now: " + (pop.st.recording || []).join(", ")
                role: "secondary"
                color: Style.danger
            }
            Row {
                width: parent.width
                layoutDirection: Qt.RightToLeft
                IconButton {
                    icon: "nyx-settings-symbolic"
                    tooltip: "Sound settings"
                    onClicked: {
                        pop.visible = false;
                        pop.ctx.hide();
                        pop.ctx.call("open_settings", { page: "audio" });
                    }
                }
            }
        }
    }
}
