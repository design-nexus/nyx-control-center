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
    status: active ? Math.round(k.value * 100) + "%" : "Off"
    tooltip: active ? "Keyboard light at level " + step + " of " + max + ". Click for the next level" : "Keyboard light is off. Click to turn it on"
    onMain: {
        if (!k)
            return;
        const next = step >= max ? 0 : step + 1;
        ctx.call("brightness.set", { device: k.device, value: next / max });
    }

    // A bar per level, stacked upward, lit to the current one.
    Column {
        visible: t.compact
        anchors.right: parent.right
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        spacing: 5
        Repeater {
            model: Math.min(5, t.max)
            Rectangle {
                required property int index
                width: 6
                height: 2
                radius: 1
                // On the lit tile, in its ink; on the plain one, in the swatch.
                color: (Math.min(5, t.max) - index) <= t.step ? (t.active ? Style.alpha(Style.bg, 0.85) : t.ctx.swatch("teal")) : (t.active ? Style.alpha(Style.bg, 0.3) : Style.alpha(Style.text, 0.18))
            }
        }
    }
}
