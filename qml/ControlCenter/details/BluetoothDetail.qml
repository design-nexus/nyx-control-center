import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

Column {
    id: v
    property var ctx
    signal back
    readonly property var st: bt.value || ({ devices: [], nearby: [] })
    property var busy: ({})
    BackendTopic { id: bt; client: v.ctx.client; name: "bluetooth"; active: v.ctx.open }

    // Looking for devices while this view shows.
    Component.onCompleted: if (st.powered) ctx.call("bluetooth.discover", { on: true })
    Component.onDestruction: ctx.call("bluetooth.discover", { on: false })
    function work(addr, op, body) {
        const b = Object.assign({}, busy);
        b[addr] = true;
        busy = b;
        ctx.call(op, body, () => {
            const c = Object.assign({}, busy);
            delete c[addr];
            busy = c;
        });
    }

    DetailHead {
        title: "Bluetooth"
        checked: !!v.st.powered
        switchEnabled: !!v.st.available
        onBack: v.back()
        onToggled: on => v.ctx.call("bluetooth.set_powered", { on: on })
    }
    Label {
        width: parent.width
        visible: !v.st.powered
        horizontalAlignment: Text.AlignHCenter
        topPadding: 40
        bottomPadding: 40
        text: v.st.available ? "Bluetooth is off" : "No Bluetooth adapter"
        dim: true
    }
    Label {
        width: parent.width
        visible: !!v.st.powered && (v.st.devices || []).length === 0
        horizontalAlignment: Text.AlignHCenter
        topPadding: 24
        text: "No paired devices yet"
        dim: true
    }
    Column {
        width: parent.width
        visible: !!v.st.powered && (v.st.devices || []).length > 0
        Group {
            text: "Devices"
        }
        CardBox {
            Repeater {
                model: v.st.devices || []
                delegate: CcRow {
                    required property var modelData
                    icon: modelData.icon || "bluetooth-symbolic"
                    title: modelData.name
                    selected: modelData.connected
                    sub: v.busy[modelData.path] ? "Working…" : modelData.connected ? (modelData.battery !== null && modelData.battery !== undefined ? "Connected · battery " + modelData.battery + "%" : "Connected") : (modelData.battery !== null && modelData.battery !== undefined ? "Battery " + modelData.battery + "%" : "")
                    onClicked: v.work(modelData.path, "bluetooth.set_connected", { path: modelData.path, on: !modelData.connected })
                    IconButton {
                        icon: "user-trash-symbolic"
                        danger: true
                        tooltip: "Forget " + parent.parent.title + ": it has to be paired again to connect"
                        onClicked: v.ctx.call("bluetooth.forget", { address: parent.parent.modelData.address })
                    }
                }
            }
        }
    }
    Column {
        width: parent.width
        visible: !!v.st.powered
        Group {
            text: v.st.discovering ? "Available (looking…)" : "Available"
        }
        CardBox {
            CcRow {
                visible: (v.st.nearby || []).length === 0
                icon: "system-search-symbolic"
                title: "Looking for devices…"
                sub: "Put the device in pairing mode"
                clickable: false
            }
            Repeater {
                model: (v.st.nearby || []).slice(0, 12)
                delegate: CcRow {
                    required property var modelData
                    icon: modelData.icon || "bluetooth-symbolic"
                    title: modelData.name
                    sub: v.busy[modelData.address] ? "Pairing…" : ""
                    onClicked: v.work(modelData.address, "bluetooth.pair", { address: modelData.address })
                }
            }
        }
    }
    Foot {
        IconButton {
            icon: "emblem-system-symbolic"
            tooltip: "Bluetooth settings"
            onClicked: {
                v.ctx.hide();
                v.ctx.call("open_settings", { page: "bluetooth" });
            }
        }
        IconButton {
            icon: "view-refresh-symbolic"
            tooltip: "Look for devices again"
            onClicked: v.ctx.call("bluetooth.discover", { on: false }, () => restart.start())
            Timer {
                id: restart
                interval: 300
                onTriggered: v.ctx.call("bluetooth.discover", { on: true })
            }
        }
    }
}
