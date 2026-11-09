import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

Column {
    id: v
    property var ctx
    signal back
    readonly property var st: disp.value || ({ screens: [] })
    readonly property var lap: (st.screens || []).find(s => s.internal) || null
    readonly property bool external: (st.screens || []).some(s => !s.internal && s.enabled)
    BackendTopic { id: disp; client: v.ctx.client; name: "display"; active: v.ctx.open }

    DetailHead {
        title: "Screens"
        hasSwitch: false
        onBack: v.back()
    }
    Group {
        visible: (v.st.screens || []).length > 0
        text: "Brightness"
    }
    CardBox {
        visible: (v.st.screens || []).length > 0
        Repeater {
            model: (v.st.screens || []).filter(s => s.enabled && s.brightness !== null && s.brightness !== undefined)
            delegate: Column {
                required property var modelData
                width: parent.width
                Label {
                    leftPadding: 8
                    topPadding: 4
                    text: modelData.internal ? "Laptop screen" : modelData.name + " (" + modelData.width + "×" + modelData.height + ")"
                    role: "secondary"
                    dim: true
                }
                SliderRow {
                    width: parent.width
                    height: 34
                    icon: "display-brightness-symbolic"
                    from: 0.01
                    to: 1
                    value: Math.max(0.01, modelData.brightness)
                    valueText: Math.round(modelData.brightness * 100) + "%"
                    tint: v.ctx.swatch("amber")
                    debounce: 80
                    onMoved: x => v.ctx.call("display.set_brightness", { screen: modelData.name, value: x })
                }
            }
        }
    }
    Column {
        width: parent.width
        visible: v.ctx.env.omarchy
        Group {
            text: "Scale"
        }
        Segmented {
            width: parent.width
            options: [1, 1.25, 1.6, 2, 3, 4].map(s => ({ id: String(s), label: String(s) + "×" }))
            current: String(v.st.scale)
            onChosen: id => v.ctx.call("display.set_scale", { value: parseFloat(id) })
        }
        Group {
            text: "Text size"
        }
        CardBox {
            SliderRow {
                width: parent.width
                height: 34
                icon: "font-x-generic-symbolic"
                iconTip: "Back to the standard size (12 px)"
                from: 9
                to: 20
                value: v.st.text_px || 12
                valueText: Math.round(v.st.text_px || 12) + " px"
                debounce: 700
                onMoved: x => v.ctx.call("display.set_text_size", { value: Math.round(x) })
                onIconClicked: v.ctx.call("display.set_text_size", { value: 12 })
            }
        }
        Column {
            width: parent.width
            visible: v.lap !== null && (v.external || !v.lap.enabled)
            topPadding: 8
            CardBox {
                CcRow {
                    icon: "computer-symbolic"
                    title: "Laptop screen"
                    sub: v.lap && v.lap.enabled ? "On" : "Off: the other screen shows everything"
                    clickable: false
                    Toggle {
                        checked: !!(v.lap && v.lap.enabled)
                        onToggled: on => v.ctx.call("display.set_internal", { on: on })
                    }
                }
            }
        }
    }
    Foot {
        IconButton {
            icon: "emblem-system-symbolic"
            tooltip: "Display settings: resolution, arrangement, refresh rate"
            onClicked: {
                v.ctx.hide();
                v.ctx.call("open_settings", { page: "displays" });
            }
        }
    }
}
