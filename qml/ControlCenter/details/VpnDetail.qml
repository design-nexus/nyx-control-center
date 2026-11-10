import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

// Where traffic goes: directly, through NordVPN or through Tor (only one at a time),
// NordVPN's place and settings, Tor's mode and exit, and NetworkManager's VPNs.
Column {
    id: v
    property var ctx
    signal back
    readonly property var st: routes.value || ({ nord: {}, tor: {}, route: "direct" })
    readonly property var nord: st.nord || {}
    readonly property var tor: st.tor || {}
    readonly property string route: st.route || "direct"
    readonly property var nmVpns: ((net.value || {}).vpns || []).filter(x => x.id !== "tailscale")
    property var settings: []
    property string technology: ""
    property string protocol: ""
    property var places: []
    property bool showPlaces: false
    property string placeQuery: ""
    property var cities: ({})
    property string openCountry: ""
    property var exits: []
    property bool showExits: false
    property var bridges: null
    property var conf: ({})
    readonly property string svcId: nord.service ? nord.service.id : ""
    readonly property var svcConf: (conf.services || {})[svcId] || ({ favorites: [] })
    function starred(arg) {
        return (svcConf.favorites || []).some(f => f.arg === arg);
    }
    function star(arg, name) {
        ctx.call("vpn.set_favorite", { service: svcId, arg: arg, name: name, on: !starred(arg) }, ok => {
            if (ok)
                v.conf = ok;
        });
    }
    property var latency: null
    BackendTopic { id: routes; client: v.ctx.client; name: "routes"; active: !!(v.ctx && v.ctx.open) }
    BackendTopic { id: net; client: v.ctx.client; name: "network"; active: !!(v.ctx && v.ctx.open) }

    function load() {
        if (nord.installed) {
            ctx.call("routes.nord_settings", {}, ok => v.settings = ok || []);
            ctx.call("routes.nord_technology", {}, ok => v.technology = ok || "");
            ctx.call("routes.nord_protocol", {}, ok => v.protocol = ok || "");
        }
        if (tor.installed)
            ctx.call("routes.tor_bridges", {}, ok => v.bridges = ok);
        ctx.call("vpn.config", {}, ok => v.conf = ok || {});
    }
    Component.onCompleted: Qt.callLater(load)
    function setting(name) {
        const s = (settings || []).find(x => String(x[0]).toLowerCase() === name.toLowerCase());
        return s ? s[1] : false;
    }

    DetailHead {
        title: "VPN"
        checked: v.route !== "direct"
        onBack: v.back()
        onToggled: on => v.ctx.call(on ? "routes.nord" : "routes.direct", {})
    }
    CardBox {
        visible: v.route === "both"
        CcRow {
            icon: "dialog-warning-symbolic"
            title: (v.nord.name || "NordVPN") + " and Tor are both on"
            sub: "Keep one of them"
            clickable: false
        }
        Row {
            leftPadding: 34
            bottomPadding: 6
            spacing: 6
            Button {
                text: "Keep " + (v.nord.name || "NordVPN")
                onClicked: v.ctx.call("routes.tor_off", {})
            }
            Button {
                text: "Keep Tor"
                onClicked: v.ctx.call("routes.nord_off", {})
            }
        }
    }
    Item {
        width: 1
        height: v.route === "both" ? 8 : 0
    }
    CardBox {
        CcRow {
            icon: "network-wired-symbolic"
            title: "Direct"
            sub: "No VPN: traffic goes straight out"
            selected: v.route === "direct"
            onClicked: v.ctx.call("routes.direct", {})
            Icon {
                visible: v.route === "direct"
                anchors.verticalCenter: parent.verticalCenter
                name: "object-select-symbolic"
                size: 14
                recolor: true
                color: Style.text
            }
        }
        CcRow {
            visible: !!v.nord.installed
            icon: "security-high-symbolic"
            title: v.nord.name || "NordVPN"
            sub: v.nord.connected ? (v.nord.city ? v.nord.country + ", " + v.nord.city : (v.nord.country || "Connected")) : (v.nord.status || "Disconnected")
            selected: v.route === "nord" || v.route === "both"
            onClicked: v.ctx.call("routes.nord", {})
        }
        CcRow {
            visible: !!v.tor.installed
            icon: "security-medium-symbolic"
            title: "Tor"
            sub: v.tor.running ? (v.tor.connected ? "Connected" + (v.tor.exit_country ? " · exit " + v.tor.exit_country.toUpperCase() : "") : "Starting… " + (v.tor.bootstrap || 0) + "%") : "Off"
            selected: v.route === "tor" || v.route === "both"
            onClicked: v.ctx.call("routes.tor", { mode: "lan" })
        }
    }
    Column {
        width: parent.width
        visible: !!v.nord.installed
        Group {
            text: v.nord.name || "NordVPN"
        }
        CardBox {
            CcRow {
                icon: "mark-location-symbolic"
                title: "Location"
                sub: v.nord.connected && v.nord.country ? (v.nord.city ? v.nord.country + ", " + v.nord.city : v.nord.country) : (v.svcConf.server ? ((v.svcConf.favorites || []).find(f => f.arg === v.svcConf.server) || { name: String(v.svcConf.server).replace(/_/g, " ") }).name : "Fastest")
                onClicked: {
                    v.showPlaces = !v.showPlaces;
                    if (v.showPlaces && v.places.length === 0)
                        v.ctx.call("vpn.places", { service: v.svcId }, ok => v.places = (ok || []).map(p => p.arg));
                }
                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: v.showPlaces ? "pan-down-symbolic" : "pan-end-symbolic"
                    size: 12
                    recolor: true
                    color: Style.dimText
                }
            }
            Column {
                visible: v.showPlaces
                width: parent.width
                SearchField {
                    width: parent.width
                    placeholder: "Search countries"
                    onTextChanged: v.placeQuery = text
                }
                Flickable {
                    width: parent.width
                    height: 200
                    clip: true
                    contentHeight: placeCol.implicitHeight
                    boundsBehavior: Flickable.StopAtBounds
                    Column {
                        id: placeCol
                        width: parent.width
                        Repeater {
                            model: v.places.filter(c => v.placeQuery === "" || String(c).toLowerCase().indexOf(v.placeQuery.toLowerCase()) >= 0)
                            delegate: Column {
                                id: pc
                                required property var modelData
                                width: placeCol.width
                                CcRow {
                                    title: String(pc.modelData).replace(/_/g, " ")
                                    onClicked: v.ctx.call("routes.nord", { place: pc.modelData })
                                    IconButton {
                                        icon: v.starred(pc.modelData) ? "starred-symbolic" : "non-starred-symbolic"
                                        toggled: v.starred(pc.modelData)
                                        tooltip: v.starred(pc.modelData) ? "Remove from favourites" : "Add to favourites"
                                        onClicked: v.star(pc.modelData, String(pc.modelData).replace(/_/g, " "))
                                    }
                                    IconButton {
                                        icon: v.openCountry === pc.modelData ? "pan-down-symbolic" : "pan-end-symbolic"
                                        tooltip: "Cities"
                                        onClicked: {
                                            v.openCountry = v.openCountry === pc.modelData ? "" : pc.modelData;
                                            if (v.openCountry && !v.cities[pc.modelData])
                                                v.ctx.call("routes.nord_cities", { country: pc.modelData }, ok => {
                                                    const c = Object.assign({}, v.cities);
                                                    c[pc.modelData] = ok || [];
                                                    v.cities = c;
                                                });
                                        }
                                    }
                                }
                                Repeater {
                                    model: v.openCountry === pc.modelData ? (v.cities[pc.modelData] || []) : []
                                    delegate: CcRow {
                                        id: city
                                        required property var modelData
                                        title: "    " + String(modelData).replace(/_/g, " ")
                                        onClicked: v.ctx.call("routes.nord", { place: modelData })
                                        IconButton {
                                            icon: v.starred(city.modelData) ? "starred-symbolic" : "non-starred-symbolic"
                                            toggled: v.starred(city.modelData)
                                            tooltip: v.starred(city.modelData) ? "Remove from favourites" : "Add to favourites"
                                            onClicked: v.star(city.modelData, String(city.modelData).replace(/_/g, " "))
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Favourite servers: connect with a click; the marked one (or Fastest) is
            // where the tile and the switch connect.
            Repeater {
                model: (v.svcConf.favorites || []).length ? [{ arg: null, name: "Fastest" }].concat(v.svcConf.favorites) : []
                delegate: CcRow {
                    id: fav
                    required property var modelData
                    readonly property bool isDefault: (v.svcConf.server || null) === modelData.arg
                    icon: modelData.arg === null ? "find-location-symbolic" : "starred-symbolic"
                    title: modelData.name
                    sub: isDefault ? "Default" : ""
                    onClicked: v.ctx.call("routes.nord", { place: modelData.arg || "" })
                    IconButton {
                        icon: fav.isDefault ? "radio-checked-symbolic" : "radio-symbolic"
                        toggled: fav.isDefault
                        tooltip: fav.isDefault ? "Where the VPN connects by default" : "Connect here by default"
                        onClicked: v.ctx.call("vpn.set_server", { service: v.svcId, arg: fav.modelData.arg || "" }, ok => {
                            if (ok)
                                v.conf = ok;
                        })
                    }
                    IconButton {
                        visible: fav.modelData.arg !== null
                        icon: "starred-symbolic"
                        toggled: true
                        tooltip: "Remove " + fav.modelData.name + " from favourites"
                        onClicked: v.star(fav.modelData.arg, fav.modelData.name)
                    }
                }
            }
            Repeater {
                model: [["Kill Switch", "killswitch", "Block the internet whenever the VPN drops"], ["Auto-connect", "autoconnect", "Connect when the computer starts"], ["Threat Protection Lite", "threatprotectionlite", "Block ads and malicious sites"]]
                delegate: CcRow {
                    required property var modelData
                    icon: "nyx-settings-symbolic"
                    title: modelData[0]
                    sub: modelData[2]
                    clickable: false
                    Toggle {
                        checked: v.setting(modelData[0])
                        onToggled: on => v.ctx.call("routes.nord_set", { name: modelData[1], on: on }, () => v.load())
                    }
                }
            }
            CcRow {
                visible: !!v.nord.connected
                icon: "media-playback-pause-symbolic"
                title: "Pause"
                clickable: false
                Repeater {
                    model: [["5m", "5 min"], ["15m", "15 min"], ["1h", "1 h"], ["24h", "24 h"]]
                    delegate: Button {
                        required property var modelData
                        text: modelData[1]
                        flat: true
                        onClicked: v.ctx.call("routes.nord_pause", { duration: modelData[0] })
                    }
                }
            }
            Segmented {
                visible: v.technology !== ""
                width: parent.width
                options: [{ id: "NORDLYNX", label: "NordLynx" }, { id: "OPENVPN", label: "OpenVPN" }, { id: "NORDWHISPER", label: "NordWhisper" }]
                current: v.technology.toUpperCase()
                onChosen: id => v.ctx.call("routes.nord_set_value", { name: "technology", value: id }, () => v.load())
            }
            Item {
                width: 1
                height: v.technology.toUpperCase() === "OPENVPN" ? 6 : 0
            }
            Segmented {
                visible: v.technology.toUpperCase() === "OPENVPN"
                width: parent.width
                options: [{ id: "udp", label: "UDP" }, { id: "tcp", label: "TCP" }]
                current: v.protocol.toLowerCase()
                onChosen: id => v.ctx.call("routes.nord_set_value", { name: "protocol", value: id }, () => v.load())
            }
        }
    }
    Column {
        width: parent.width
        visible: !!v.tor.installed
        Group {
            text: "Tor"
        }
        CardBox {
            Repeater {
                model: [["strict", "Maximum", "Everything, the local network cut off"], ["lan", "Standard", "Internet traffic; the local network still works"], ["socks", "Browser only", "Only apps set to use it; works beside NordVPN"]]
                delegate: CcRow {
                    required property var modelData
                    icon: "security-medium-symbolic"
                    title: modelData[1]
                    sub: modelData[2]
                    selected: !!v.tor.running && String(v.tor.mode).toLowerCase() === modelData[0]
                    onClicked: v.ctx.call("routes.tor", { mode: modelData[0] })
                }
            }
            Column {
                width: parent.width
                visible: !!v.tor.connected
                CcRow {
                    icon: "view-refresh-symbolic"
                    title: "Exit" + (v.tor.exit_country ? ": " + v.tor.exit_country.toUpperCase() : "")
                    sub: v.tor.exit_ip || ""
                    onClicked: {
                        v.showExits = !v.showExits;
                        if (v.showExits && v.exits.length === 0)
                            v.ctx.call("routes.tor_exit_countries", {}, ok => v.exits = ok || []);
                    }
                    IconButton {
                        icon: "view-refresh-symbolic"
                        tooltip: "A new circuit"
                        onClicked: v.ctx.call("routes.tor_new_circuit", {})
                    }
                }
                Flickable {
                    visible: v.showExits
                    width: parent.width
                    height: 180
                    clip: true
                    contentHeight: exitCol.implicitHeight
                    Column {
                        id: exitCol
                        width: parent.width
                        CcRow {
                            title: "Let Tor choose"
                            onClicked: v.ctx.call("routes.tor_exit", { country: "auto" })
                        }
                        Repeater {
                            model: v.exits
                            delegate: CcRow {
                                required property var modelData
                                title: String(modelData[0]).toUpperCase() + " (" + modelData[1] + ")"
                                onClicked: v.ctx.call("routes.tor_exit", { country: modelData[0] })
                            }
                        }
                    }
                }
                CcRow {
                    icon: "network-transmit-receive-symbolic"
                    title: "Latency"
                    sub: v.latency === null ? "Measure it" : Math.round(v.latency) + " ms"
                    onClicked: v.ctx.call("routes.tor_latency", {}, ok => v.latency = ok)
                }
                CcRow {
                    icon: "web-browser-symbolic"
                    title: "Tor browser"
                    sub: "A browser that only goes through Tor"
                    onClicked: v.ctx.call("routes.tor_browser", {})
                }
            }
            CcRow {
                icon: "network-vpn-symbolic"
                title: "Bridges"
                sub: v.bridges === null ? "" : (v.bridges[0] ? "Set up" : "None set up (tormarchy bridge set)")
                clickable: false
                Toggle {
                    enabled: v.bridges !== null && v.bridges[0]
                    checked: v.bridges !== null && v.bridges[1]
                    onToggled: on => v.ctx.call("routes.tor_set_bridges", { on: on }, () => v.load())
                }
            }
            CcRow {
                visible: !!v.tor.running
                icon: "process-stop-symbolic"
                title: "Panic"
                sub: "Stop Tor and cut its connections now"
                onClicked: v.ctx.call("routes.tor_panic", {})
            }
        }
    }
    Column {
        width: parent.width
        visible: v.nmVpns.length > 0
        Group {
            text: "Connections"
        }
        CardBox {
            Repeater {
                model: v.nmVpns
                delegate: CcRow {
                    required property var modelData
                    icon: "network-vpn-symbolic"
                    title: modelData.name
                    clickable: false
                    Toggle {
                        checked: modelData.active
                        onToggled: on => v.ctx.call("network.set_vpn", { id: modelData.id, on: on })
                    }
                }
            }
        }
    }
    Foot {
        IconButton {
            icon: "list-add-symbolic"
            tooltip: "VPN accounts and connections"
            onClicked: {
                v.ctx.hide();
                v.ctx.call("open_settings", { page: "online-accounts" });
            }
        }
    }
}
