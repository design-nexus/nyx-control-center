import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

// Paired devices (Flux): who's connected, battery, ring and send the clipboard.
// Pairing and what each device may do are in Settings › Connected Devices.
Column {
    id: v
    property var ctx
    signal back
    readonly property var st: dv.value || ({ devices: [], pairing: [] })
    property var busy: ({})
    BackendTopic { id: dv; client: v.ctx.client; name: "devices"; active: !!(v.ctx && v.ctx.open) }

    function work(id, op) {
        const b = Object.assign({}, busy);
        b[id + op] = true;
        busy = b;
        ctx.call(op, { id: id }, () => {
            const c = Object.assign({}, busy);
            delete c[id + op];
            busy = c;
        });
    }
    function settings() {
        ctx.hide();
        ctx.call("open_settings", { page: "devices" });
    }

    DetailHead {
        title: "Devices"
        hasSwitch: false
        onBack: v.back()
    }
    Label {
        width: parent.width
        visible: !v.st.available
        horizontalAlignment: Text.AlignHCenter
        topPadding: 32
        bottomPadding: 8
        text: v.st.installed ? "Flux isn't running" : "Flux isn't installed"
        dim: true
    }
    Button {
        visible: !!v.st.installed && !v.st.available
        anchors.horizontalCenter: parent.horizontalCenter
        text: "Start Flux"
        onClicked: v.ctx.call("devices.start", {})
    }
    Column {
        width: parent.width
        visible: !!v.st.available && (v.st.pairing || []).length > 0
        Group {
            text: "Asking to pair"
        }
        CardBox {
            Repeater {
                model: v.st.pairing || []
                delegate: CcRow {
                    required property var modelData
                    icon: "dialog-password-symbolic"
                    title: modelData
                    sub: "Compare the key in Settings"
                    onClicked: v.settings()
                }
            }
        }
    }
    Label {
        width: parent.width
        visible: !!v.st.available && (v.st.devices || []).length === 0
        horizontalAlignment: Text.AlignHCenter
        topPadding: 24
        text: "No paired devices yet"
        dim: true
    }
    Column {
        width: parent.width
        visible: !!v.st.available && (v.st.devices || []).length > 0
        Group {
            text: "Paired"
        }
        CardBox {
            Repeater {
                model: v.st.devices || []
                delegate: CcRow {
                    id: row
                    required property var modelData
                    icon: modelData.icon || "phone-symbolic"
                    title: modelData.name
                    selected: modelData.connected
                    clickable: false
                    sub: !modelData.connected ? "Not connected" : (modelData.battery !== null && modelData.battery !== undefined ? "Connected · battery " + modelData.battery + "%" + (modelData.charging ? ", charging" : "") : "Connected")
                    IconButton {
                        visible: row.modelData.can_clipboard
                        enabled: !v.busy[row.modelData.id + "devices.send_clipboard"]
                        icon: "edit-paste-symbolic"
                        tooltip: "Send the clipboard to " + row.modelData.name
                        onClicked: v.work(row.modelData.id, "devices.send_clipboard")
                    }
                    IconButton {
                        visible: row.modelData.can_ring
                        enabled: !v.busy[row.modelData.id + "devices.ring"]
                        icon: "audio-volume-high-symbolic"
                        tooltip: "Ring " + row.modelData.name + ", to find it"
                        onClicked: v.work(row.modelData.id, "devices.ring")
                    }
                }
            }
        }
    }
    Foot {
        IconButton {
            icon: "emblem-system-symbolic"
            tooltip: "Connected Devices settings: pair, and what each device may do"
            onClicked: v.settings()
        }
    }
}
