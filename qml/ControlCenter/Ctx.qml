import QtQuick
import Nyx.Theme
import Nyx.Ui

// What every tile and detail view gets: the backend, the panel's state, and the
// Control Center's colours (theme swatches, as the GTK panel's `cc_*` colours).
QtObject {
    id: ctx

    required property var client
    property bool open: false
    property bool editing: false
    property int page: 0
    property string detail: ""
    property var env: client.state.env || ({ omarchy: false, name: "", have: ({}) })

    signal openDetail(string which)
    signal hide

    function call(op, body, cb) {
        client.request(op, body || {}, cb || ((ok, err) => {
            if (err)
                console.warn("nyx-control-center:", op, err.code, err.msg);
        }));
    }
    function have(program) {
        return !!(env.have && env.have[program]);
    }
    function swatch(name) {
        const s = NyxTheme.swatches && NyxTheme.swatches[name];
        return s ? NyxTheme.qmlColor(s) : Style.accent;
    }
    function pct(v) {
        return Math.round(v * 100) + "%";
    }
}
