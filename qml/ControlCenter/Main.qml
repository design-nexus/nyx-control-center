import QtQuick
import Quickshell
import Quickshell.Wayland
import Nyx.Shell
import Nyx.Ui

// Nyx Control Center in the shared Quickshell runtime. Its backend
// (nyx-control-center-backend) owns the bus name and Panel1 and decides when the panel
// is open; this draws it: a 400 px card at the top right of the focused screen, with a
// clear catcher behind it that closes it on a click anywhere else. The layer
// namespaces are the GTK panel's, so Shell Core and other tools see the same surfaces.
Scope {
    id: root

    readonly property string pinnedScreen: Quickshell.env("NYX_SHELL_SCREEN") || ""
    readonly property bool keyboardOff: Quickshell.env("NYX_SHELL_NO_KEYBOARD") === "1"
    readonly property var panelState: cc.state.panel || ({ open: false, detail: null, serial: 0 })
    readonly property bool open: panelState.open === true && cc.connected
    property string screenName: ""

    BackendClient {
        id: cc
        app: "nyx-control-center"
        topics: ["panel", "layout", "env", "plugins"]
    }

    Ctx {
        id: ctx
        client: cc
        open: root.open
        onOpenDetail: which => panel.openDetail(which)
        onHide: cc.request("Hide", {}, null)
    }

    // A new opening: pick the screen, start on the first page unless remembered, on the
    // detail view asked for.
    property int serial: panelState.serial || 0
    property int shownSerial: -1
    function opening() {
        if (!open || shownSerial === serial)
            return;
        shownSerial = serial;
        screenName = pinnedScreen || DesktopState.activeMonitorName || (Quickshell.screens[0] ? Quickshell.screens[0].name : "");
        const layout = cc.state.layout || {};
        panel.reset(layout.remember_page !== false, panelState.detail || "");
        // Screenshot hooks, as the GTK panel had: NYX_CC_TEST_PAGE, NYX_CC_TEST_EDIT.
        const testPage = Quickshell.env("NYX_CC_TEST_PAGE");
        if (testPage)
            panel.showPage(parseInt(testPage));
        if (Quickshell.env("NYX_CC_TEST_EDIT"))
            panel.setEditing(true);
    }
    onSerialChanged: Qt.callLater(opening)
    onOpenChanged: {
        if (open)
            Qt.callLater(opening);
        else {
            panel.reset(true, "");
            volume.visible = false;
            powerPop.visible = false;
        }
        reportPlugins();
    }

    // Plugins whose tiles or detail view are on screen (they run while they are).
    function reportPlugins() {
        const ids = new Set();
        if (open) {
            const pages = (cc.state.layout || {}).pages || [];
            const pg = pages[ctx.page];
            for (const t of (pg ? pg.tiles : []))
                if (String(t.widget).indexOf("plugin:") === 0)
                    ids.add(String(t.widget).substr(7).split("/")[0]);
            if (panel.detail.indexOf("plugin:") === 0)
                ids.add(panel.detail.substr(7).split("/")[0]);
        }
        cc.request("plugins.on_screen", { plugins: Array.from(ids) }, null);
    }
    Connections {
        target: ctx
        function onPageChanged() {
            root.reportPlugins();
        }
    }
    Connections {
        target: panel
        function onDetailChanged() {
            root.reportPlugins();
        }
    }
    Connections {
        target: cc
        function onReconnected() {
            root.reportPlugins();
        }
    }

    readonly property var targetScreen: Quickshell.screens.find(s => s.name === screenName) || null

    PanelWindow {
        id: catcher
        screen: root.targetScreen
        visible: root.open && root.targetScreen !== null
        color: Qt.rgba(0, 0, 0, 0.004)
        anchors.top: true
        anchors.bottom: true
        anchors.left: true
        anchors.right: true
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.namespace: "nyx-control-center-catcher"
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.AllButtons
            onPressed: ctx.hide()
        }
    }

    PanelWindow {
        id: win
        screen: root.targetScreen
        // After the catcher, so it stacks above it.
        visible: catcher.visible && catcherShown
        property bool catcherShown: false
        Connections {
            target: catcher
            function onVisibleChanged() {
                if (catcher.visible)
                    Qt.callLater(() => win.catcherShown = true);
                else
                    win.catcherShown = false;
            }
        }
        color: "transparent"
        // The surface is the whole height and never changes size: the card inside grows
        // and shrinks (a detail view, a page of a different height) with an animation,
        // and only the card takes input. Resizing the surface itself, frame by frame,
        // made the compositor catch up a frame late each time: a jerky bounce.
        anchors.top: true
        anchors.bottom: true
        anchors.right: true
        margins.top: 6
        margins.right: 6
        margins.bottom: 6
        // Below the bar, as the GTK panel sat: others' reserved space is kept.
        exclusionMode: ExclusionMode.Normal
        exclusiveZone: 0
        implicitWidth: 400
        mask: Region {
            item: card
        }
        // The card shows once the first layout has settled (rows sized, topics in),
        // so it opens at its size instead of growing into it.
        property bool settled: false
        onVisibleChanged: {
            settled = false;
            if (visible)
                settle.restart();
        }
        Timer {
            id: settle
            interval: 80
            onTriggered: win.settled = true
        }
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.namespace: "nyx-control-center"
        WlrLayershell.keyboardFocus: root.keyboardOff ? WlrKeyboardFocus.None : WlrKeyboardFocus.OnDemand

        Surface {
            id: card
            width: parent.width
            height: Math.min(parent.height, body.implicitHeight + 30)
            shadow: false
            outline: true
            opacity: win.settled ? 1 : 0
            Behavior on opacity {
                NumberAnimation {
                    duration: Style.reducedMotion ? 0 : 120
                    easing.type: Easing.OutCubic
                }
            }
            Panel {
                id: body
                x: 14
                y: 14
                width: parent.width - 28
                animate: win.settled
                maxHeight: win.height - 30
                ctx: ctx
                focus: true
                onVolumeMenu: a => {
                    volume.anchorItem = a;
                    volume.openAt(0, a.height + 4);
                }
                onPowerMenu: a => {
                    powerPop.anchorItem = a;
                    powerPop.openAt(a.width - powerPop.implicitWidth, a.height + 4);
                }
            }
        }
        VolumePopover {
            id: volume
            ctx: ctx
        }
        PowerPopover {
            id: powerPop
            ctx: ctx
        }
    }

    property alias panel: body
}
