import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

Column {
    id: v
    property var ctx
    signal back
    readonly property var st: pw.value || ({ profiles: [] })
    readonly property var b: st.battery || null
    readonly property var d: det.value
    BackendTopic { id: pw; client: v.ctx.client; name: "power"; active: v.ctx.open }
    BackendTopic { id: det; client: v.ctx.client; name: "power-details"; active: v.ctx.open }

    DetailHead {
        title: "Battery"
        hasSwitch: false
        onBack: v.back()
    }
    Label {
        width: parent.width
        visible: !v.b
        horizontalAlignment: Text.AlignHCenter
        topPadding: 40
        bottomPadding: 40
        text: "No battery"
        dim: true
    }
    CardBox {
        visible: !!v.b
        Item {
            width: parent.width
            height: 64
            Label {
                x: 8
                y: 6
                text: v.b ? Math.round(v.b.percent) + "%" : ""
                role: "readout"
            }
            Label {
                x: 8
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 6
                text: !v.b ? "" : v.b.full ? "Fully charged" : v.b.charging ? (v.b.duration ? "Charging, full in " + v.b.duration : "Charging") : (v.b.duration ? "On battery, " + v.b.duration + " left" : "On battery")
                dim: true
            }
        }
    }
    Column {
        width: parent.width
        visible: !!v.d
        Group {
            text: "Details"
        }
        CardBox {
            Fact { visible: !!(v.d && v.d.rate); name: v.b && v.b.charging ? "Charging at" : "Using"; value: v.d ? v.d.rate : "" }
            Fact { visible: !!(v.d && v.d.size); name: "Capacity"; value: v.d ? v.d.size : "" }
            Fact { visible: !!(v.d && v.d.cycles); name: "Charge cycles"; value: v.d && v.d.cycles ? String(v.d.cycles) : "" }
            Fact { visible: !!(v.d && v.d.threshold); name: "Charge limit"; value: v.d ? v.d.threshold : "" }
            Label {
                visible: !!(v.d && v.d.threshold)
                leftPadding: 8
                bottomPadding: 6
                width: parent.width
                wrapMode: Text.Wrap
                text: "Charging stops inside the limit to spare the battery."
                role: "secondary"
                dim: true
            }
        }
    }
    Group {
        text: "Power mode"
    }
    CardBox {
        Repeater {
            model: [["power-saver", "power-profile-power-saver-symbolic", "Power saver", "Longer battery life, slower"], ["balanced", "power-profile-balanced-symbolic", "Balanced", "The everyday setting"], ["performance", "power-profile-performance-symbolic", "Performance", "Fastest; more power, more heat"]].filter(p => v.st.profiles.length === 0 || v.st.profiles.indexOf(p[0]) >= 0)
            delegate: CcRow {
                required property var modelData
                icon: modelData[1]
                title: modelData[2]
                sub: modelData[3]
                selected: v.st.profile === modelData[0]
                onClicked: v.ctx.call("power.set_profile", { profile: modelData[0] })
            }
        }
    }
    Foot {
        IconButton {
            icon: "emblem-system-symbolic"
            tooltip: "Power settings: suspend, lid, power button"
            onClicked: {
                v.ctx.hide();
                v.ctx.call("open_settings", { page: "power" });
            }
        }
    }
}
