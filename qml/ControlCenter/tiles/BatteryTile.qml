import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

// Battery: its icon in the round chip (green charging or full, red when low), the
// percentage and, wider, the time left and a chevron. A click opens the Battery page.
ToggleTile {
    id: t
    property int cols: 1
    property var tile
    readonly property var b: (pw.value && pw.value.battery) || null
    BackendTopic { id: pw; client: t.ctx.client; name: "power"; active: !!(t.ctx && t.ctx.open && t.visible) }

    active: false
    chevron: true
    iconInk: !b ? Style.dimText : (b.percent < 15 && !b.charging) ? ctx.swatch("red") : (b.charging || b.full) ? ctx.swatch("green") : Style.text
    icon: {
        if (!t.b)
            return "battery-missing-symbolic";
        const p = t.b.percent;
        const level = p <= 10 ? "empty" : p <= 30 ? "caution" : p <= 60 ? "low" : p <= 90 ? "good" : "full";
        return "battery-" + level + (t.b.charging ? "-charging" : "") + "-symbolic";
    }
    title: b ? Math.round(b.percent) + "%" : "Battery"
    status: !b ? "No battery" : b.full ? "Fully charged" : b.charging ? (b.duration ? "Charging, full in " + b.duration : "Charging") : (b.duration ? b.duration + " left" : "On battery")
    tooltip: !b ? "No battery" : "Battery " + Math.round(b.percent) + "%, " + status.toLowerCase() + ". Click for details"
    onMain: ctx.openDetail("battery")
}
