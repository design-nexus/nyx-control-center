import QtQuick
import Nyx.Shell
import Nyx.Ui
import ".."

// Backup's grant-scoped status (no Backup internals).
TileFrame {
    id: t
    property int cols: 2
    property var tile
    readonly property var s: bk.value || ({})
    BackendTopic { id: bk; client: t.ctx.client; name: "backup"; active: t.ctx.open && t.visible }

    function when(ts, prefix) {
        return prefix + new Date(ts * 1000).toLocaleString(Qt.locale(), Locale.ShortFormat);
    }

    Column {
        anchors.fill: parent
        anchors.margins: 10
        spacing: 4
        Row {
            width: parent.width
            spacing: 8
            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: "drive-harddisk-symbolic"
                size: 20
                recolor: true
                color: Style.text
            }
            Label {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - 20 - 32 - 16
                text: t.s.name || "Backup"
                size: 12.5
                font.weight: Font.Medium
            }
            IconButton {
                icon: "window-new-symbolic"
                tooltip: "Open Backup"
                onClicked: t.ctx.call("backup.open", {})
            }
        }
        Label {
            width: parent.width
            text: t.s.problem || ({ working: "Backup job running", protected: "Last backup succeeded", attention: "Last backup needs attention", destination_unavailable: "Destination unavailable" })[t.s.health] || (t.s.health === undefined ? "Waiting for Backup" : "No successful backup recorded")
            role: "secondary"
            dim: true
        }
        Label {
            width: parent.width
            text: t.s.last_success ? t.when(t.s.last_success, "Last success: ") : "No successful backup recorded"
            role: "secondary"
            dim: true
        }
        Label {
            width: parent.width
            text: t.s.next_schedule ? t.when(t.s.next_schedule, "Next: ") : (t.s.schedule ? "Schedule saved; no active next run reported" : "No schedule configured")
            role: "secondary"
            dim: true
        }
        ProgressBar {
            width: parent.width
            visible: !!t.s.active
            value: t.s.fraction || 0
        }
        Row {
            spacing: 6
            IconButton {
                icon: "media-playback-start-symbolic"
                tooltip: "Start the granted backup profile"
                enabled: !!t.s.can_backup && !t.s.active && !t.s.problem
                onClicked: t.ctx.call("backup.action", { grant: t.s.grant, name: "backup.start" })
            }
            IconButton {
                icon: "process-stop-symbolic"
                tooltip: "Cancel this profile's active job"
                enabled: !!t.s.can_cancel && !!t.s.active && !t.s.problem
                onClicked: t.ctx.call("backup.action", { grant: t.s.grant, name: "backup.cancel", job: t.s.active })
            }
        }
    }
}
