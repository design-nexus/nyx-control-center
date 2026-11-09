import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

Column {
    id: v
    property var ctx
    signal back
    readonly property var st: net.value || ({ networks: [] })
    readonly property var d: details.value
    readonly property var dnsState: dns.value || ({})
    readonly property var cur: (st.networks || []).find(n => n.active) || null
    property string asking: ""
    BackendTopic { id: net; client: v.ctx.client; name: "network"; active: !!(v.ctx && v.ctx.open) }
    BackendTopic { id: details; client: v.ctx.client; name: "dns-details"; active: !!(v.ctx && v.ctx.open && v.st.wifi_on === true) }
    BackendTopic { id: dns; client: v.ctx.client; name: "dns"; active: !!(v.ctx && v.ctx.open) }

    function bars(sig) {
        const b = sig >= 75 ? 4 : sig >= 50 ? 3 : sig >= 25 ? 2 : 1;
        return ["", "network-wireless-signal-weak-symbolic", "network-wireless-signal-ok-symbolic", "network-wireless-signal-good-symbolic", "network-wireless-signal-excellent-symbolic"][b];
    }
    function ms(v) {
        return v === null || v === undefined ? "—" : Math.round(v) + " ms";
    }

    spacing: 0
    DetailHead {
        title: "Wi-Fi"
        checked: !!v.st.wifi_on
        switchEnabled: !!v.st.has_wifi
        onBack: v.back()
        onToggled: on => v.ctx.call("network.set_wifi", { on: on })
    }
    Label {
        width: parent.width
        visible: !v.st.wifi_on
        horizontalAlignment: Text.AlignHCenter
        topPadding: 40
        bottomPadding: 40
        text: "Wi-Fi is off"
        dim: true
    }
    CardBox {
        visible: !!v.st.wifi_on && v.cur !== null
        CcRow {
            icon: v.cur ? v.bars(v.cur.signal) : ""
            title: v.cur ? v.cur.ssid.trim() : ""
            sub: "Connected"
            selected: true
            clickable: false
            IconButton {
                icon: "network-offline-symbolic"
                tooltip: "Disconnect"
                onClicked: v.ctx.call("network.disconnect", { uuid: v.st.wifi_uuid })
            }
        }
        CcRow {
            title: "Metered connection"
            sub: "Pause large downloads and updates on this network"
            clickable: false
            Toggle {
                checked: !!v.st.metered
                onToggled: on => v.ctx.call("network.set_metered", { uuid: v.st.wifi_uuid, on: on })
            }
        }
    }
    Repeater {
        model: v.st.wifi_on ? [["Known networks", true], ["Other networks", false]] : []
        delegate: Column {
            id: grp
            required property var modelData
            readonly property var list: (v.st.networks || []).filter(n => !n.active && n.known === modelData[1]).slice(0, 12)
            width: v.width
            visible: list.length > 0
            Group {
                text: grp.modelData[0]
            }
            CardBox {
                Repeater {
                    model: grp.list
                    delegate: Column {
                        id: netRow
                        required property var modelData
                        width: parent.width
                        CcRow {
                            icon: v.bars(netRow.modelData.signal)
                            title: netRow.modelData.ssid.trim()
                            onClicked: {
                                if (netRow.modelData.known || !netRow.modelData.secure)
                                    v.ctx.call("network.connect", { ssid: netRow.modelData.ssid });
                                else
                                    v.asking = netRow.modelData.ssid;
                            }
                            Icon {
                                visible: netRow.modelData.secure
                                anchors.verticalCenter: parent.verticalCenter
                                name: "network-wireless-encrypted-symbolic"
                                size: 14
                                recolor: true
                                color: Style.tertiary
                            }
                        }
                        Row {
                            visible: v.asking === netRow.modelData.ssid
                            leftPadding: 34
                            bottomPadding: 8
                            spacing: 6
                            SearchField {
                                id: pw
                                width: netRow.width - 34 - 48
                                icon: ""
                                password: true
                                placeholder: "Password"
                                onVisibleChanged: if (visible) focusInput()
                                onAccepted: {
                                    v.ctx.call("network.connect", { ssid: netRow.modelData.ssid, password: pw.text });
                                    v.asking = "";
                                }
                                onEscaped: v.asking = ""
                            }
                            IconButton {
                                icon: "go-next-symbolic"
                                tooltip: "Join (Enter)"
                                onClicked: pw.accepted()
                            }
                        }
                    }
                }
            }
        }
    }
    Column {
        width: parent.width
        visible: !!v.st.wifi_on && !!v.d
        Group {
            text: "Connection"
        }
        CardBox {
            Fact { name: "Address"; value: v.d ? (v.d.prefix ? v.d.ip + "/" + v.d.prefix : v.d.ip) : "" }
            Fact { name: "Router"; value: v.d ? v.d.gateway : "" }
            Fact { name: "Router ping"; value: v.d ? v.ms(v.d.router_ping_ms) : "" }
            Fact { name: "Internet ping"; value: v.d ? v.ms(v.d.internet_ping_ms) : "" }
            Fact { name: "Down"; value: v.d && v.d.down_text ? v.d.down_text : "…" }
            Fact { name: "Up"; value: v.d && v.d.up_text ? v.d.up_text : "…" }
            Fact { visible: !!(v.d && v.d.ssid); name: "Signal"; value: v.d && v.d.signal_dbm !== null && v.d.signal_dbm !== undefined ? v.d.signal_dbm + " dBm" : "—" }
        }
    }
    Column {
        width: parent.width
        visible: !!v.st.wifi_on && !!v.dnsState.portal
        topPadding: 8
        CardBox {
            CcRow {
                icon: "dialog-password-symbolic"
                title: "This network wants you to sign in"
                sub: "Opens its sign-in page"
                onClicked: v.ctx.call("dns.sign_in", {})
            }
        }
    }
    Column {
        width: parent.width
        visible: !!v.st.wifi_on && !!v.dnsState.dns
        Group {
            text: "DNS"
        }
        CardBox {
            Repeater {
                model: [["Cloudflare", "Cloudflare"], ["Google", "Google"], ["DHCP", "The network's own"], ["Custom", "Your own"]]
                delegate: CcRow {
                    required property var modelData
                    title: modelData[1]
                    selected: v.dnsState.dns === modelData[0]
                    onClicked: v.ctx.call("dns.set_dns", { provider: modelData[0] })
                }
            }
        }
    }
    Column {
        width: parent.width
        visible: !!v.st.wifi_on && !!v.dnsState.band && ((v.dnsState.band.available || []).length > 1 || !!v.dnsState.band.selected)
        Group {
            text: "Band"
        }
        CardBox {
            CcRow {
                title: "Automatic" + (v.dnsState.band ? " (now " + v.dnsState.band.current + " GHz)" : "")
                selected: !!v.dnsState.band && !v.dnsState.band.selected
                onClicked: v.ctx.call("dns.set_band", { band: "auto" })
            }
            Repeater {
                model: v.dnsState.band ? v.dnsState.band.available : []
                delegate: CcRow {
                    required property var modelData
                    title: modelData + " GHz"
                    selected: !!v.dnsState.band && v.dnsState.band.selected === modelData
                    onClicked: v.ctx.call("dns.set_band", { band: modelData })
                }
            }
        }
    }
    Foot {
        IconButton {
            icon: "emblem-system-symbolic"
            tooltip: "Wi-Fi settings"
            onClicked: {
                v.ctx.hide();
                v.ctx.call("open_settings", { page: "wifi" });
            }
        }
        IconButton {
            visible: v.ctx.env.omarchy
            icon: "qr-code-symbolic"
            tooltip: "Share this network: a QR code a phone can scan to join"
            onClicked: v.ctx.call("dns.share_qr", {})
        }
        IconButton {
            visible: v.ctx.env.omarchy
            icon: "speedometer-symbolic"
            tooltip: "Speed test: measure download and upload"
            onClicked: v.ctx.call("dns.speed_test", {})
        }
        IconButton {
            icon: "view-refresh-symbolic"
            tooltip: "Look for networks again"
            onClicked: v.ctx.call("network.rescan", {})
        }
    }
}
