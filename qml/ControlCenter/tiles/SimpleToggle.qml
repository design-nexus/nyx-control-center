import QtQuick
import Nyx.Shell
import ".."

// Do not disturb, Night light, Keep awake: Omarchy's switches.
ToggleTile {
    id: t
    property int cols: 1
    property var tile
    property string field: "dnd"
    property string op: "omarchy.toggle_dnd"
    property string onStatus: "On"
    property var tips: ["", ""]
    readonly property bool isOn: !!(om.value && om.value[field])
    BackendTopic { id: om; client: t.ctx.client; name: "omarchy"; active: t.ctx.open && t.visible }

    active: isOn
    status: isOn ? onStatus : "Off"
    tooltip: isOn ? tips[0] : tips[1]
    onMain: ctx.call(op, {})
}
