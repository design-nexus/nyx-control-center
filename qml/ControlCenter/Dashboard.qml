import QtQuick
import Nyx.Shell
import Nyx.Ui

// Pages of tiles in a 4-column grid of 48 px rows, packed first-fit as nyx-widgets'
// `pack` does. Swipe or scroll sideways, or the dots, change page. In edit mode each
// tile can be removed, resized and dragged to another place or onto a page's dot.
Item {
    id: d

    property var ctx
    property var pages: []
    property var kinds: []
    property int page: 0
    property bool editing: false
    readonly property real cellW: (width - 3 * gap) / 4
    readonly property int rowH: 48
    readonly property int gap: 8
    readonly property bool hasPlayer: !!(media.value && media.value.player)
    readonly property bool hasBrightness: !!(bright.value && bright.value.screen)
    BackendTopic { id: media; client: d.ctx.client; name: "media"; active: !!(d.ctx && d.ctx.open) }
    BackendTopic { id: bright; client: d.ctx.client; name: "brightness"; active: !!(d.ctx && d.ctx.open) }

    signal changed(var pages)
    signal volumeMenu(var anchor)

    function parseSize(s) {
        const m = /^\s*(\d)\s*x\s*(\d)\s*$/.exec(String(s));
        return m ? { cols: Math.min(4, parseInt(m[1])), rows: parseInt(m[2]) } : { cols: 2, rows: 1 };
    }
    function sizesOf(widget) {
        const k = d.kinds.find(k => k.id === widget) || ((d.ctx.client.state.plugins || {}).kinds || []).find(k => k.id === widget);
        return k ? k.sizes : [];
    }
    // The size a tile is drawn at: its own if the kind offers it, else the kind's first;
    // Sound & brightness is one row shorter where no brightness can be set.
    function effective(t) {
        const sizes = sizesOf(t.widget);
        let s = sizes.length && sizes.indexOf(t.size) < 0 ? sizes[0] : t.size;
        let size = parseSize(s);
        if (t.widget === "sound" && !d.hasBrightness && !d.editing)
            size = { cols: size.cols, rows: Math.max(1, size.rows - 1) };
        return size;
    }
    function shown(t) {
        return d.editing || t.widget !== "media" || d.hasPlayer;
    }
    function pack(page) {
        const taken = [];
        const out = [];
        const tiles = (page.tiles || []);
        for (let i = 0; i < tiles.length; i++) {
            const t = tiles[i];
            if (!shown(t))
                continue;
            const s = effective(t);
            let row = 0, spot = null;
            while (spot === null) {
                while (taken.length < row + s.rows)
                    taken.push([false, false, false, false]);
                for (let c = 0; c <= 4 - s.cols && spot === null; c++) {
                    let free = true;
                    for (let r = row; r < row + s.rows && free; r++)
                        for (let cc = c; cc < c + s.cols; cc++)
                            if (taken[r][cc])
                                free = false;
                    if (free)
                        spot = [c, row];
                }
                if (spot === null)
                    row++;
            }
            for (let r = spot[1]; r < spot[1] + s.rows; r++)
                for (let c = spot[0]; c < spot[0] + s.cols; c++)
                    taken[r][c] = true;
            out.push({ index: i, tile: t, col: spot[0], row: spot[1], cols: s.cols, rows: s.rows });
        }
        return out;
    }
    function rowsOf(placed) {
        return placed.reduce((m, p) => Math.max(m, p.row + p.rows), 0);
    }
    readonly property var packed: pages.map(p => pack(p))
    readonly property int maxRows: Math.max(1, ...packed.map(p => rowsOf(p)))

    function edit(f) {
        const copy = JSON.parse(JSON.stringify(d.pages));
        f(copy);
        d.changed(copy);
    }
    function moveTile(fp, fi, tp, ti) {
        edit(pages => {
            if (fp >= pages.length || fi >= pages[fp].tiles.length || tp >= pages.length)
                return;
            if (tp === fp && ti !== -1 && fi < ti)
                ti -= 1;
            const t = pages[fp].tiles.splice(fi, 1)[0];
            const at = ti < 0 ? pages[tp].tiles.length : Math.min(ti, pages[tp].tiles.length);
            pages[tp].tiles.splice(at, 0, t);
        });
    }

    implicitHeight: maxRows * rowH + (maxRows - 1) * gap
    clip: true

    Row {
        id: strip
        x: -d.page * (d.width + 24)
        spacing: 24
        Behavior on x {
            NumberAnimation {
                duration: Style.panelDuration
                easing.type: Easing.OutCubic
            }
        }
        Repeater {
            model: d.pages.length
            delegate: Item {
                id: pageItem
                required property int index
                readonly property int pageIndex: index
                width: d.width
                height: d.implicitHeight

                Label {
                    anchors.centerIn: parent
                    visible: d.editing && (d.pages[pageItem.pageIndex].tiles || []).length === 0
                    text: "Add widgets from below, or drag one here"
                    size: 11.5
                    dim: true
                }
                DropArea {
                    anchors.fill: parent
                    keys: ["cc-tile"]
                    onDropped: drop => {
                        if (drop.source && drop.source.fromPage !== undefined && !drop.accepted)
                            d.moveTile(drop.source.fromPage, drop.source.fromIndex, pageItem.pageIndex, -1);
                    }
                }

                Repeater {
                    model: d.packed[pageItem.pageIndex] || []
                    delegate: Item {
                        id: cell
                        required property var modelData
                        readonly property int fromPage: pageItem.pageIndex
                        readonly property int fromIndex: modelData.index
                        readonly property real homeX: modelData.col * (d.cellW + d.gap)
                        readonly property real homeY: modelData.row * (d.rowH + d.gap)
                        x: homeX
                        y: homeY
                        width: modelData.cols * d.cellW + (modelData.cols - 1) * d.gap
                        height: modelData.rows * d.rowH + (modelData.rows - 1) * d.gap
                        z: drag.active ? 10 : 0
                        Drag.active: drag.active
                        Drag.keys: ["cc-tile"]
                        Drag.hotSpot.x: width / 2
                        Drag.hotSpot.y: height / 2

                        TileView {
                            anchors.fill: parent
                            ctx: d.ctx
                            tile: cell.modelData.tile
                            cols: cell.modelData.cols
                            rows: cell.modelData.rows
                            page: cell.fromPage
                            index: cell.fromIndex
                            enabled: !d.editing
                            onVolumeMenu: a => d.volumeMenu(a)
                        }

                        // Edit mode: a dashed outline, the whole tile a handle.
                        Rectangle {
                            anchors.fill: parent
                            visible: d.editing
                            color: "transparent"
                            radius: Style.radiusCard
                            border.width: 1
                            border.color: Style.alpha(Style.text, 0.25)
                        }
                        MouseArea {
                            id: drag
                            anchors.fill: parent
                            visible: d.editing
                            property bool active: drag.drag.active
                            drag.target: cell
                            cursorShape: drag.drag.active ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                            onReleased: {
                                if (drag.drag.active)
                                    cell.Drag.drop();
                                cell.x = Qt.binding(() => cell.homeX);
                                cell.y = Qt.binding(() => cell.homeY);
                            }
                        }
                        DropArea {
                            anchors.fill: parent
                            keys: ["cc-tile"]
                            enabled: d.editing
                            onDropped: drop => {
                                const s = drop.source;
                                if (!s || s === cell || s.fromPage === undefined)
                                    return;
                                drop.accept();
                                d.moveTile(s.fromPage, s.fromIndex, cell.fromPage, cell.fromIndex);
                            }
                        }
                        Rectangle {
                            visible: d.editing
                            x: -6
                            y: -6
                            width: 22
                            height: 22
                            radius: 11
                            color: Style.raised
                            border.width: 1
                            border.color: Style.border
                            Icon {
                                anchors.centerIn: parent
                                name: "window-close-symbolic"
                                size: 12
                                recolor: true
                                color: removeArea.containsMouse ? Style.danger : Style.text
                            }
                            MouseArea {
                                id: removeArea
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: d.edit(pages => pages[cell.fromPage].tiles.splice(cell.fromIndex, 1))
                            }
                        }
                        Rectangle {
                            readonly property var sizes: d.sizesOf(cell.modelData.tile.widget)
                            visible: d.editing && sizes.length > 1
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            anchors.margins: 4
                            width: 22
                            height: 22
                            radius: 11
                            color: "transparent"
                            Icon {
                                anchors.centerIn: parent
                                name: "view-fullscreen-symbolic"
                                size: 12
                                recolor: true
                                color: Style.tertiary
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: d.edit(pages => {
                                    const t = pages[cell.fromPage].tiles[cell.fromIndex];
                                    const sizes = parent.sizes;
                                    const i = sizes.indexOf(t.size);
                                    t.size = i < 0 ? sizes[0] : sizes[(i + 1) % sizes.length];
                                })
                            }
                        }
                    }
                }
            }
        }
    }

    // Sideways scrolling (a touchpad swipe) changes page past 40 px.
    WheelHandler {
        orientation: Qt.Horizontal
        property real acc: 0
        onWheel: e => {
            acc += e.pixelDelta.x !== 0 ? e.pixelDelta.x : e.angleDelta.x / 3;
            if (Math.abs(acc) > 40) {
                const next = d.page + (acc < 0 ? 1 : -1);
                if (next >= 0 && next < d.pages.length)
                    d.page = next;
                acc = 0;
            }
        }
    }
}
