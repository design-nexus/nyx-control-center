import QtQuick
import Quickshell
import Nyx.Shell
import Nyx.Ui

// The panel's content: header, the dashboard (or a detail view in its place), page
// dots, and the tray of widgets to add in edit mode.
FocusScope {
    id: p

    property var ctx
    readonly property var lay: ctx.client.state.layout || ({ pages: [], kinds: [], remember_page: true })
    readonly property var plugins: ctx.client.state.plugins || ({ kinds: [], off: [] })
    property string detail: ""
    // Height changes animate once the panel has settled (not while it opens).
    property bool animate: true
    // The most the panel may be (the screen's room); a detail view taller than what's
    // left below the header scrolls, as GTK's did.
    property real maxHeight: 100000
    readonly property real roomForContent: maxHeight - 52
    property var draft: null
    readonly property var pages: draft || lay.pages || []
    readonly property var pw: power.value
    BackendTopic { id: power; client: p.ctx.client; name: "power"; active: !!(p.ctx && p.ctx.open) }

    signal volumeMenu(var anchor)
    signal powerMenu(var anchor)

    function back() {
        detail = "";
    }
    function setEditing(on) {
        if (!on && draft) {
            // Leaving edit mode drops empty pages and saves.
            const tidy = draft.filter(pg => (pg.tiles || []).length > 0);
            ctx.call("layout.save", { pages: tidy.length ? tidy : [{ tiles: [] }] });
            draft = null;
        }
        ctx.editing = on;
        if (on)
            draft = JSON.parse(JSON.stringify(lay.pages));
        if (dash.page >= pages.length)
            dash.page = Math.max(0, pages.length - 1);
    }
    function save(newPages) {
        if (ctx.editing)
            draft = newPages;
        ctx.call("layout.save", { pages: newPages });
    }

    implicitHeight: col.implicitHeight

    Keys.onEscapePressed: event => {
        if (detail !== "")
            back();
        else if (ctx.editing)
            setEditing(false);
        else
            ctx.hide();
        event.accepted = true;
    }

    Column {
        id: col
        width: parent.width
        spacing: 0

        // ---------- Header ----------
        Item {
            width: parent.width
            height: 40
            Rectangle {
                id: avatar
                anchors.verticalCenter: parent.verticalCenter
                width: 32
                height: 32
                radius: 16
                color: Style.accent
                Label {
                    anchors.centerIn: parent
                    text: (p.ctx.env.name || "?").charAt(0).toUpperCase()
                    color: Style.onAccent
                    font.weight: Font.DemiBold
                }
            }
            Column {
                anchors.left: avatar.right
                anchors.leftMargin: 10
                anchors.right: buttons.left
                anchors.verticalCenter: parent.verticalCenter
                Label {
                    width: parent.width
                    text: p.ctx.env.name || ""
                    font.weight: Font.Medium
                }
                Label {
                    width: parent.width
                    readonly property var b: p.pw ? p.pw.battery : null
                    visible: text !== ""
                    text: !b ? "" : b.full ? "Battery full" : b.duration ? (b.charging ? "Battery " + Math.round(b.percent) + "% · " + b.duration + " to full" : "Battery " + Math.round(b.percent) + "% · " + b.duration + " left") : "Battery " + Math.round(b.percent) + "%"
                    size: 12
                    font.features: { "tnum": 1 }
                    dim: true
                }
            }
            Row {
                id: buttons
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2
                IconButton {
                    visible: !p.ctx.editing
                    icon: "document-edit-symbolic"
                    tooltip: "Edit the dashboard"
                    onClicked: p.setEditing(true)
                }
                IconButton {
                    visible: !p.ctx.editing
                    icon: "nyx-settings-symbolic"
                    tooltip: "Settings"
                    onClicked: {
                        p.ctx.hide();
                        p.ctx.call("open_settings", { page: "" });
                    }
                }
                IconButton {
                    id: powerButton
                    visible: !p.ctx.editing
                    icon: "system-shutdown-symbolic"
                    tooltip: "Lock, suspend, restart, shut down"
                    onClicked: {
                        if (p.ctx.env.omarchy) {
                            p.ctx.hide();
                            p.ctx.call("system_menu", {});
                        } else
                            p.powerMenu(powerButton);
                    }
                }
                IconButton {
                    id: restore
                    visible: p.ctx.editing
                    property bool armed: false
                    icon: "edit-undo-symbolic"
                    tooltip: armed ? "Click again to restore the default layout" : "Restore the default layout"
                    danger: armed
                    onClicked: {
                        if (armed) {
                            armed = false;
                            p.ctx.call("layout.restore", {}, () => p.draft = JSON.parse(JSON.stringify(p.lay.pages)));
                        } else {
                            armed = true;
                            disarm.restart();
                        }
                    }
                    Timer {
                        id: disarm
                        interval: 4000
                        onTriggered: restore.armed = false
                    }
                }
                IconButton {
                    visible: p.ctx.editing
                    primary: true
                    icon: "object-select-symbolic"
                    tooltip: "Done"
                    onClicked: p.setEditing(false)
                }
            }
        }
        Item {
            width: 1
            height: 12
        }

        // ---------- Dashboard or detail ----------
        Item {
            width: parent.width
            // A detail view is as tall as it needs (its head, what scrolls, its footer),
            // up to the room there is.
            height: p.detail === "" ? dash.implicitHeight : Math.min(p.roomForContent, headHost.height + detailLoader.implicitHeight + footHost.height)
            clip: true
            Behavior on height {
                enabled: p.animate
                NumberAnimation {
                    duration: Style.panelDuration
                    easing.type: Easing.OutCubic
                }
            }
            Dashboard {
                id: dash
                width: parent.width
                x: p.detail === "" ? 0 : -width - 24
                Behavior on x {
                    NumberAnimation {
                        duration: Style.panelDuration
                        easing.type: Easing.OutCubic
                    }
                }
                ctx: p.ctx
                pages: p.pages
                kinds: p.lay.kinds || []
                editing: p.ctx.editing
                onPageChanged: p.ctx.page = page
                onChanged: newPages => p.save(newPages)
                onVolumeMenu: a => p.volumeMenu(a)
            }
            Timer {
                id: scrollTest
                interval: 1500
                onTriggered: detailFlick.contentY = Math.max(0, detailFlick.contentHeight - detailFlick.height)
            }
            // The detail view: its head and footer stay in sight; between them it scrolls.
            Item {
                id: detailPane
                width: parent.width
                height: parent.height
                x: p.detail === "" ? width + 24 : 0
                Behavior on x {
                    NumberAnimation {
                        duration: Style.panelDuration
                        easing.type: Easing.OutCubic
                    }
                }
                Item {
                    id: headHost
                    width: parent.width
                    height: childrenRect.height
                }
                Item {
                    id: footHost
                    y: parent.height - height
                    width: parent.width
                    height: childrenRect.height
                }
                Flickable {
                    id: detailFlick
                    y: headHost.height
                    width: parent.width
                    height: parent.height - headHost.height - footHost.height
                    contentHeight: detailLoader.implicitHeight
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds
                    interactive: contentHeight > height
                    Loader {
                        id: detailLoader
                        width: parent.width
                        active: p.detail !== "" || detailPane.x < detailPane.width
                        property string which: p.detail
                        onWhichChanged: if (which !== "") {
                            shownDetail = which;
                            detailFlick.contentY = 0;
                        }
                        property string shownDetail: ""
                        source: {
                            const w = shownDetail;
                            if (w === "")
                                return "";
                            if (w.indexOf("plugin:") === 0)
                                return "details/PluginDetail.qml";
                            return "details/" + ({ wifi: "Wifi", bluetooth: "Bluetooth", vpn: "Vpn", tailscale: "Tailscale", display: "Display", battery: "Battery" })[w] + "Detail.qml";
                        }
                        onLoaded: {
                            item.ctx = Qt.binding(() => p.ctx);
                            if (item.tileId !== undefined)
                                item.tileId = shownDetail;
                            item.back.connect(p.back);
                            // Its head and footer stay put above and below what scrolls.
                            // (Out of the page's column they keep the place it gave them,
                            // so they go back to the top of their own.)
                            for (const c of item.children.slice()) {
                                if (c.detailHead || c.detailFoot) {
                                    c.parent = c.detailHead ? headHost : footHost;
                                    c.x = 0;
                                    c.y = 0;
                                }
                            }
                            // Screenshot hook: NYX_CC_TEST_SCROLL scrolls a detail view to its end.
                            if (Quickshell.env("NYX_CC_TEST_SCROLL"))
                                scrollTest.restart();
                        }
                    }
                }
            }
        }

        // ---------- Dots ----------
        Item {
            width: parent.width
            visible: p.detail === "" && (p.pages.length > 1 || p.ctx.editing)
            height: visible ? 18 : 0
            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                spacing: 6
                Repeater {
                    model: p.pages.length
                    delegate: Rectangle {
                        id: dot
                        required property int index
                        readonly property bool on: dash.page === index
                        width: on ? 16 : 6
                        height: 6
                        radius: 3
                        color: on ? Style.dimText : Style.alpha(Style.text, 0.1)
                        Behavior on width {
                            NumberAnimation {
                                duration: Style.panelDuration
                            }
                        }
                        MouseArea {
                            anchors.fill: parent
                            anchors.margins: -5
                            cursorShape: Qt.PointingHandCursor
                            onClicked: dash.page = dot.index
                        }
                        DropArea {
                            anchors.fill: parent
                            anchors.margins: -6
                            keys: ["cc-tile"]
                            enabled: p.ctx.editing
                            onDropped: drop => {
                                if (drop.source && drop.source.fromPage !== undefined) {
                                    drop.accept();
                                    dash.moveTile(drop.source.fromPage, drop.source.fromIndex, dot.index, -1);
                                }
                            }
                        }
                    }
                }
                IconButton {
                    visible: p.ctx.editing
                    width: 18
                    height: 18
                    iconSize: 12
                    icon: "list-add-symbolic"
                    tooltip: "Add a page"
                    onClicked: {
                        const copy = JSON.parse(JSON.stringify(p.pages));
                        copy.push({ tiles: [] });
                        p.draft = copy;
                        dash.page = copy.length - 1;
                    }
                }
            }
        }

        // ---------- Tray (edit mode) ----------
        Tray {
            width: parent.width
            visible: p.ctx.editing && p.detail === ""
            ctx: p.ctx
            pages: p.pages
            kinds: p.lay.kinds || []
            plugins: p.plugins
            onAdd: (id, size) => {
                const copy = JSON.parse(JSON.stringify(p.pages));
                const pg = Math.min(dash.page, copy.length - 1);
                copy[pg].tiles.push({ widget: id, size: size });
                p.save(copy);
            }
        }
    }

    function showPage(n) {
        dash.page = Math.max(0, Math.min(pages.length - 1, n));
    }
    function openDetail(which) {
        detail = which;
    }
    function reset(remember, detailWanted) {
        if (!remember)
            dash.page = 0;
        detail = detailWanted || "";
        if (ctx.editing)
            setEditing(false);
    }
}
