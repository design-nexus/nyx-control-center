import QtQuick
import Quickshell
import Quickshell.Wayland
import Nyx.Shell
import Nyx.Ui

// Nyx Control Center in the shared Quickshell runtime. Its backend
// (nyx-control-center-backend) owns the bus name and Panel1 and decides when the panel
// is open; this draws it: a 400 px card at the top right of the focused screen, on a
// clear surface over the whole screen that closes it on a click anywhere else. The
// layer namespace is the GTK panel's, so Shell Core and other tools see the same surface.
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

    // The focused monitor, or the first screen when that name isn't one (a headless or
    // virtual output): the panel always has somewhere to show.
    readonly property var targetScreen: Quickshell.screens.find(s => s.name === screenName) || Quickshell.screens[0] || null

    PanelWindow {
        id: win
        screen: root.targetScreen
        visible: root.open && root.targetScreen !== null
        // The whole screen, over the bar and the dock, and it never changes size: the
        // card inside grows and shrinks (a detail view, a page of a different height)
        // with an animation. Resizing the surface itself, frame by frame, made the
        // compositor catch up a frame late each time: a jerky bounce. A click anywhere
        // around the card closes it, here and not on a surface of its own: Hyprland
        // gives the pointer only to a layer that holds the keyboard exclusively.
        color: Qt.rgba(0, 0, 0, 0.004)
        anchors.top: true
        anchors.bottom: true
        anchors.left: true
        anchors.right: true
        exclusionMode: ExclusionMode.Ignore
        // The card sits below the bar, as the GTK panel did: others' reserved space is kept.
        readonly property var monitor: root.targetScreen ? DesktopState.monitor(root.targetScreen.name) : null
        readonly property var reserved: (monitor && monitor.reserved) || [0, 0, 0, 0]
        // Only once the card is on screen: a second press while it's still appearing
        // doesn't close it unseen. On the release, not the press: a surface gone with a
        // button still down left that button held for the next opening, which then saw
        // no hovering (no tooltips) until a click.
        MouseArea {
            anchors.fill: parent
            enabled: win.settled
            acceptedButtons: Qt.AllButtons
            onReleased: ctx.hide()
        }
        // The card shows once the first layout has settled (rows sized, topics in),
        // so it opens at its size instead of growing into it.
        property bool settled: false
        onVisibleChanged: {
            settled = false;
            if (visible) {
                settle.restart();
                Qt.callLater(() => body.forceActiveFocus());
            }
        }
        Timer {
            id: settle
            interval: 80
            onTriggered: {
                win.settled = true;
                // The backend's Toggle closes only an opening that was on screen.
                cc.request("Shown", { serial: root.shownSerial }, null);
            }
        }
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.namespace: "nyx-control-center"
        // Exclusive while open, as Glimpse's: on-demand focus goes back to the last
        // window whenever another layer closes, so the panel had the keyboard only once
        // clicked. Clicks around the card close it, so it's modal anyway.
        WlrLayershell.keyboardFocus: root.keyboardOff ? WlrKeyboardFocus.None : WlrKeyboardFocus.Exclusive

        Surface {
            id: card
            readonly property real room: win.height - win.reserved[1] - win.reserved[3] - 12
            x: win.width - width - win.reserved[2] - 6
            y: win.reserved[1] + 6
            width: 400
            height: Math.min(room, body.implicitHeight + 30)
            shadow: false
            outline: true
            opacity: win.settled ? 1 : 0
            Behavior on opacity {
                NumberAnimation {
                    duration: Style.reducedMotion ? 0 : 120
                    easing.type: Easing.OutCubic
                }
            }
            // A click on the card itself, between its controls, doesn't close it.
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.AllButtons
            }
            Panel {
                id: body
                x: 14
                y: 14
                width: parent.width - 28
                animate: win.settled
                maxHeight: card.room - 30
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
