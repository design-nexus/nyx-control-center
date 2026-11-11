import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

Column {
    id: v
    property var ctx
    signal back
    readonly property var st: pw.value || ({ profiles: [] })
    readonly property var b: st.battery || null
    readonly property var d: det.value
    BackendTopic { id: pw; client: v.ctx.client; name: "power"; active: !!(v.ctx && v.ctx.open) }
    BackendTopic { id: det; client: v.ctx.client; name: "power-details"; active: !!(v.ctx && v.ctx.open) }
    BackendTopic { id: hist; client: v.ctx.client; name: "power-history"; active: !!(v.ctx && v.ctx.open && v.b) }
    readonly property var h: hist.value || ({ points: [], energy: [] })
    // The day's charge as steps (each record holds until the next), ending now.
    readonly property var steps: {
        const now = Date.now() / 1000;
        const pts = (h.points || []).filter(p => p.time >= now - 86400);
        if (v.b)
            pts.push({ time: now, percent: v.b.percent, charging: v.b.charging });
        return pts;
    }

    DetailHead {
        title: "Battery"
        hasSwitch: false
        onBack: v.back()
    }
    Label {
        width: parent.width
        visible: !v.b
        horizontalAlignment: Text.AlignHCenter
        topPadding: 40
        bottomPadding: 40
        text: "No battery"
        dim: true
    }
    CardBox {
        visible: !!v.b
        Item {
            width: parent.width
            height: 64
            Label {
                x: 8
                y: 6
                text: v.b ? Math.round(v.b.percent) + "%" : ""
                role: "readout"
            }
            Label {
                x: 8
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 6
                text: !v.b ? "" : v.b.full ? "Fully charged" : v.b.charging ? (v.b.duration ? "Charging, full in " + v.b.duration : "Charging") : (v.b.duration ? "On battery, " + v.b.duration + " left" : "On battery")
                dim: true
            }
        }
    }
    Column {
        width: parent.width
        visible: !!v.b && v.steps.length > 1
        Group {
            text: "Last 24 hours"
        }
        CardBox {
            Item {
                width: parent.width
                height: 96
                Canvas {
                    id: graph
                    x: 30
                    y: 6
                    width: parent.width - 36
                    height: 68
                    property var steps: v.steps
                    onStepsChanged: requestPaint()
                    onWidthChanged: requestPaint()
                    onPaint: {
                        const c = getContext("2d");
                        c.reset();
                        const now = Date.now() / 1000, from = now - 86400;
                        const X = t => (Math.max(t, from) - from) / 86400 * width;
                        const Y = p => height - p / 100 * height;
                        // Grid at 0, 50 and 100 %.
                        c.strokeStyle = Style.alpha(Style.text, 0.08);
                        c.lineWidth = 1;
                        for (const p of [0, 50, 100]) {
                            c.beginPath();
                            c.moveTo(0, Math.round(Y(p)) + 0.5);
                            c.lineTo(width, Math.round(Y(p)) + 0.5);
                            c.stroke();
                        }
                        const s = steps;
                        if (s.length < 2)
                            return;
                        // Charging stretches tinted green beneath.
                        c.fillStyle = Style.alpha(Style.success, 0.18);
                        for (let i = 0; i < s.length - 1; i++)
                            if (s[i].charging)
                                c.fillRect(X(s[i].time), 0, X(s[i + 1].time) - X(s[i].time), height);
                        c.beginPath();
                        c.moveTo(X(s[0].time), Y(s[0].percent));
                        for (let i = 1; i < s.length; i++) {
                            c.lineTo(X(s[i].time), Y(s[i - 1].percent));
                            c.lineTo(X(s[i].time), Y(s[i].percent));
                        }
                        c.strokeStyle = Style.accent;
                        c.lineWidth = 2;
                        c.stroke();
                        c.lineTo(X(s[s.length - 1].time), height);
                        c.lineTo(X(s[0].time), height);
                        c.closePath();
                        c.fillStyle = Style.alpha(Style.accent, 0.14);
                        c.fill();
                    }
                }
                Repeater {
                    model: [100, 50, 0]
                    delegate: Label {
                        required property int modelData
                        x: 4
                        y: graph.y + graph.height * (1 - modelData / 100) - height / 2
                        text: modelData + "%"
                        size: 9.5
                        color: Style.tertiary
                    }
                }
                Repeater {
                    model: [["24 h ago", 0], ["12 h", 0.5], ["Now", 1]]
                    delegate: Label {
                        required property var modelData
                        x: graph.x + graph.width * modelData[1] - (modelData[1] === 0 ? 0 : modelData[1] === 1 ? width : width / 2)
                        y: graph.y + graph.height + 6
                        text: modelData[0]
                        size: 9.5
                        color: Style.tertiary
                    }
                }
            }
        }
    }
    Column {
        width: parent.width
        visible: !!v.b && (v.h.energy || []).length > 0
        Group {
            text: "Using significant energy"
        }
        CardBox {
            Repeater {
                model: v.h.energy || []
                delegate: CcRow {
                    required property var modelData
                    icon: String(modelData.app).toLowerCase()
                    title: modelData.app
                    clickable: false
                    Label {
                        anchors.verticalCenter: parent.verticalCenter
                        text: Math.round(modelData.cpu) + "% CPU"
                        role: "secondary"
                        color: modelData.cpu >= 50 ? Style.warning : Style.dimText
                        font.features: { "tnum": 1 }
                    }
                }
            }
        }
        Label {
            width: parent.width
            leftPadding: 8
            topPadding: 6
            wrapMode: Text.Wrap
            text: "An estimate from each app's share of the processor over the last minute."
            role: "secondary"
            dim: true
        }
    }
    Column {
        width: parent.width
        visible: !!v.d
        Group {
            text: "Details"
        }
        FactGrid {
            Fact { visible: !!(v.d && v.d.rate); name: v.b && v.b.charging ? "Charging at" : "Using"; value: v.d ? v.d.rate : "" }
            Fact { visible: !!(v.d && v.d.size); name: "Capacity"; value: v.d ? v.d.size : "" }
            Fact { visible: !!(v.d && v.d.cycles); name: "Charge cycles"; value: v.d && v.d.cycles ? String(v.d.cycles) : "" }
            Fact { visible: !!(v.d && v.d.threshold); name: "Charge limit"; value: v.d ? v.d.threshold : "" }
        }
        Label {
            visible: !!(v.d && v.d.threshold)
            leftPadding: 8
            topPadding: 6
            width: parent.width
            wrapMode: Text.Wrap
            text: "Charging stops inside the limit to spare the battery."
            role: "secondary"
            dim: true
        }
    }
    Group {
        text: "Power mode"
    }
    CardBox {
        Repeater {
            model: [["power-saver", Qt.resolvedUrl("../icons/leaf.svg"), "Power saver", "Longer battery life, slower"], ["balanced", Qt.resolvedUrl("../icons/gauge.svg"), "Balanced", "The everyday setting"], ["performance", Qt.resolvedUrl("../icons/bolt.svg"), "Performance", "Fastest; more power, more heat"]].filter(p => v.st.profiles.length === 0 || v.st.profiles.indexOf(p[0]) >= 0)
            delegate: CcRow {
                required property var modelData
                icon: modelData[1]
                title: modelData[2]
                sub: modelData[3]
                selected: v.st.profile === modelData[0]
                onClicked: v.ctx.call("power.set_profile", { profile: modelData[0] })
            }
        }
    }
    Foot {
        IconButton {
            icon: "nyx-settings-symbolic"
            tooltip: "Power settings: suspend, lid, power button"
            onClicked: {
                v.ctx.hide();
                v.ctx.call("open_settings", { page: "power" });
            }
        }
    }
}
