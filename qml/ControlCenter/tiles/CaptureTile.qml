import QtQuick
import ".."

ToggleTile {
    property int cols: 1
    property var tile
    swatch: "pink"
    icon: "camera-photo-symbolic"
    title: "Capture"
    status: "Screenshot"
    tooltip: "Screenshot: closes this panel and opens the capture bar"
    onMain: {
        ctx.hide();
        ctx.call("capture", {});
    }
}
