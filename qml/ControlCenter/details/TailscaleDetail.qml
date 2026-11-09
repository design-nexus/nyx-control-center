import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

Column {
    id: v
    property var ctx
    signal back
    readonly property var st: ts.value || ({ peers: [], accounts: [] })
    property string query: ""
    property string openCountry: ""
    readonly property var peers: (st.peers || []).filter(p => {
        const q = query.toLowerCase().trim();
        return q === "" || (p.name + " " + p.dns + " " + (p.ips || []).join(" ")).toLowerCase().indexOf(q) >= 0;
    })
    readonly property var exits: (st.peers || []).filter(p => p.exit_option)
    readonly property var own: exits.filter(p => !p.location)
    readonly property var located: exits.filter(p => !!p.location)
    readonly property var countries: Array.from(new Set(located.map(p => p.country || p.location))).sort()
    BackendTopic { id: ts; client: v.ctx.client; name: "tailscale"; active: !!(v.ctx && v.ctx.open) }

    DetailHead {
        title: "Tailscale"
        checked: !!v.st.running
        switchEnabled: !!v.st.installed
        onBack: v.back()
        onToggled: on => v.ctx.call("tailscale.set_up", { on: on })
    }
    Label {
        width: parent.width
        visible: !v.st.installed
        horizontalAlignment: Text.AlignHCenter
        topPadding: 40
        bottomPadding: 40
        text: "Tailscale isn't installed"
        dim: true
    }
    Column {
        width: parent.width
        visible: !!v.st.installed && v.st.backend === "NeedsLogin"
        topPadding: 12
        spacing: 8
        Label {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: "Sign in to join your tailnet"
            dim: true
        }
        Button {
            anchors.horizontalCenter: parent.horizontalCenter
            text: "Sign in"
            primary: true
            onClicked: v.ctx.call("tailscale.login", {})
        }
    }
    Column {
        width: parent.width
        visible: !!v.st.running
        SearchField {
            width: parent.width
            placeholder: "Search machines"
            onTextChanged: v.query = text
        }
        Group {
            text: "This machine"
        }
        CardBox {
            CcRow {
                icon: "computer-symbolic"
                title: v.st.me ? v.st.me.name : ""
                sub: v.st.me ? (v.st.me.ips || [])[0] || "" : ""
                clickable: false
                IconButton {
                    icon: "edit-copy-symbolic"
                    tooltip: "Copy the address"
                    onClicked: v.ctx.call("copy", { text: (v.st.me.ips || [])[0] || "" })
                }
                IconButton {
                    icon: "insert-link-symbolic"
                    tooltip: "Copy the name"
                    onClicked: v.ctx.call("copy", { text: v.st.me.dns || v.st.me.name })
                }
            }
            CcRow {
                icon: "folder-download-symbolic"
                title: "Receive files"
                sub: "Saved to Downloads"
                onClicked: v.ctx.call("open_folder", {})
                Toggle {
                    checked: !!v.st.receiving
                    onToggled: on => v.ctx.call("tailscale.set_receiving", { on: on })
                }
            }
        }
        Group {
            visible: v.exits.length > 0
            text: "Exit node"
        }
        CardBox {
            visible: v.exits.length > 0
            CcRow {
                title: "None"
                sub: "Traffic goes out from this machine"
                selected: !v.st.exit_node
                onClicked: v.ctx.call("tailscale.set_exit", {})
            }
            Repeater {
                model: v.own
                delegate: CcRow {
                    required property var modelData
                    icon: "network-server-symbolic"
                    title: modelData.name
                    selected: !!modelData.exit_active
                    onClicked: v.ctx.call("tailscale.set_exit", { ip: (modelData.ips || [])[0] })
                }
            }
            Repeater {
                model: v.countries
                delegate: Column {
                    id: country
                    required property var modelData
                    width: parent.width
                    CcRow {
                        icon: "mark-location-symbolic"
                        title: country.modelData
                        selected: v.located.some(p => p.exit_active && (p.country || p.location) === country.modelData)
                        onClicked: v.openCountry = v.openCountry === country.modelData ? "" : country.modelData
                        Icon {
                            anchors.verticalCenter: parent.verticalCenter
                            name: v.openCountry === country.modelData ? "pan-down-symbolic" : "pan-end-symbolic"
                            size: 12
                            recolor: true
                            color: Style.dimText
                        }
                    }
                    Repeater {
                        model: v.openCountry === country.modelData ? v.located.filter(p => (p.country || p.location) === country.modelData) : []
                        delegate: CcRow {
                            required property var modelData
                            title: "    " + (modelData.location || modelData.name)
                            selected: !!modelData.exit_active
                            onClicked: v.ctx.call("tailscale.set_exit", { ip: (modelData.ips || [])[0] })
                        }
                    }
                }
            }
        }
        Group {
            text: "Machines"
        }
        CardBox {
            Repeater {
                model: v.peers.filter(p => !p.exit_option || !p.location)
                delegate: CcRow {
                    id: m
                    required property var modelData
                    icon: modelData.online ? "computer-symbolic" : "network-offline-symbolic"
                    title: modelData.name
                    sub: (modelData.online ? "" : "Offline · ") + ((modelData.ips || [])[0] || "")
                    clickable: false
                    IconButton {
                        icon: "edit-copy-symbolic"
                        tooltip: "Copy the address"
                        onClicked: v.ctx.call("copy", { text: (m.modelData.ips || [])[0] || "" })
                    }
                    IconButton {
                        icon: "insert-link-symbolic"
                        tooltip: "Copy the name"
                        onClicked: v.ctx.call("copy", { text: m.modelData.dns || m.modelData.name })
                    }
                    IconButton {
                        visible: v.ctx.env.omarchy && m.modelData.online && v.ctx.have("omarchy-tailscale-send")
                        icon: "document-send-symbolic"
                        tooltip: "Send files"
                        onClicked: v.ctx.call("tailscale.send", { dns: m.modelData.dns })
                    }
                }
            }
        }
        Column {
            width: parent.width
            visible: (v.st.accounts || []).length > 1
            Group {
                text: "Accounts"
            }
            CardBox {
                Repeater {
                    model: v.st.accounts || []
                    delegate: CcRow {
                        required property var modelData
                        icon: "avatar-default-symbolic"
                        title: modelData.account
                        sub: modelData.tailnet
                        selected: modelData.active
                        onClicked: v.ctx.call("tailscale.switch", { id: modelData.id })
                    }
                }
            }
        }
    }
}
