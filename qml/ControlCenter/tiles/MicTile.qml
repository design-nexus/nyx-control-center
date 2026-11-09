import QtQuick
import Nyx.Shell
import ".."

ToggleTile {
    id: t
    property int cols: 1
    property var tile
    readonly property var st: au.value || ({ inputs: [], recording: [] })
    readonly property bool live: !st.mic_muted && (st.recording || []).length > 0
    BackendTopic { id: au; client: t.ctx.client; name: "audio"; active: !!(t.ctx && t.ctx.open && t.visible) }

    title: "Microphone"
    swatch: live ? "red" : "coral"
    active: !st.mic_muted
    icon: st.mic_muted ? "microphone-sensitivity-muted-symbolic" : "audio-input-microphone-symbolic"
    status: st.mic_muted ? "Muted" : "On"
    enabled: (st.inputs || []).length > 0 || !st.available
    opacity: enabled ? 1 : 0.5
    tooltip: st.mic_muted ? "The microphone is muted. Click to unmute" : (st.recording || []).length === 0 ? "Microphone on (" + Math.round(st.mic_volume * 100) + "%). Click to mute" : "Recording now: " + st.recording.join(", ") + ". Click to mute the microphone"
    onMain: ctx.call("audio.set_mic_muted", { on: !st.mic_muted })
}
