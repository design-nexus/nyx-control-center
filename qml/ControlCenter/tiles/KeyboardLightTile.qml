import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

ToggleTile {
    id: t
    property int cols: 1
    property var tile
    readonly property var st: br.value || ({})
    readonly property var k: st.keyboard || null
    readonly property int max: k ? k.max : 0
    readonly property int step: k ? Math.round(k.value * k.max) : 0
    BackendTopic { id: br; client: t.ctx.client; name: "brightness"; active: !!(t.ctx && t.ctx.open) }

    visible: k !== null || !st.screen || t.ctx.editing
    swatch: "teal"
    icon: "keyboard-brightness-symbolic"
    title: "Key light"
    active: k !== null && k.value > 0
    status: active ? "Level " + step + " of " + max : "Off"
    tooltip: active ? "Keyboard light at level " + step + " of " + max + ". Click for the next level" : "Keyboard light is off. Click to turn it on"
    onMain: {
        if (!k)
            return;
        const next = step >= max ? 0 : step + 1;
        ctx.call("brightness.set", { device: k.device, value: next / max });
    }

    // A bar per level, stacked upward, lit to the current one: in the text colour, which
    // reads on the plain card and the tinted one alike.
    Column {
        visible: t.compact && t.max > 0
        anchors.right: parent.right
        anchors.rightMargin: 9
        anchors.verticalCenter: parent.verticalCenter
        spacing: 4
        Repeater {
            model: Math.min(5, t.max)
            Rectangle {
                required property int index
                width: 12
                height: 4
                radius: 2
                color: (Math.min(5, t.max) - index) <= t.step ? Style.text : Style.alpha(Style.text, 0.2)
            }
        }
    }
}
