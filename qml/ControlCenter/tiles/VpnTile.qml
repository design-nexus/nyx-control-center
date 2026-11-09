import QtQuick
import Nyx.Shell
import ".."

ToggleTile {
    id: t
    property int cols: 2
    property var tile
    readonly property var st: routes.value || ({ nord: {}, tor: {}, route: "direct" })
    readonly property var nm: ((net.value || {}).vpns || []).find(v => v.active && v.id !== "tailscale") || null
    readonly property string route: st.route || "direct"
    BackendTopic { id: routes; client: t.ctx.client; name: "routes"; active: t.ctx.open && t.visible }
    BackendTopic { id: net; client: t.ctx.client; name: "network"; active: t.ctx.open && t.visible }

    swatch: route === "both" ? "red" : "green"
    split: cols >= 2
    title: "VPN"
    active: route !== "direct" || nm !== null
    icon: ({ direct: "security-high-symbolic", nord: "security-high-symbolic", tor: "security-medium-symbolic", both: "dialog-warning-symbolic" })[route] || "security-high-symbolic"
    readonly property var text: {
        const n = st.nord || {}, tor = st.tor || {};
        if (route === "both")
            return [n.name + " and Tor", n.name + " and Tor are both on. Click to go direct"];
        if (route === "nord") {
            const place = n.city ? n.country + ", " + n.city : n.country;
            return [n.country ? n.name + " · " + n.country : n.name, "Traffic goes through " + n.name + " (" + place + "). Click to go direct"];
        }
        if (route === "tor")
            return [tor.exit_country ? "Tor · " + tor.exit_country.toUpperCase() : "Tor", "Traffic goes through Tor. Click to go direct"];
        if (nm)
            return [nm.name, nm.name + " is connected. Click to disconnect"];
        return ["Direct", n.installed ? "Traffic goes out directly. Click to connect " + n.name : "Traffic goes out directly"];
    }
    status: text[0]
    tooltip: text[1]
    onMain: {
        if (nm)
            ctx.call("network.set_vpn", { id: nm.id, on: false });
        else if (route === "direct") {
            if (st.nord.installed)
                ctx.call("routes.nord", {});
            else if (st.tor.installed)
                ctx.call("routes.tor", { mode: "lan" });
        } else
            ctx.call("routes.direct", {});
    }
    onMore: ctx.openDetail("vpn")
}
