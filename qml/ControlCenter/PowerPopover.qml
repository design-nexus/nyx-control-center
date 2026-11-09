import QtQuick
import Quickshell
import Nyx.Ui

// Lock, log out, restart, shut down (without Omarchy's system menu).
Menu {
    id: m
    property var ctx
    entries: [{ id: "lock", label: "Lock", icon: "system-lock-screen-symbolic" }, { id: "logout", label: "Log out", icon: "system-log-out-symbolic" }, { id: "reboot", label: "Restart", icon: "system-reboot-symbolic" }, { id: "poweroff", label: "Shut down", icon: "system-shutdown-symbolic" }]
    onTriggered: id => {
        ctx.hide();
        ctx.call("power_action", { action: id });
    }
}
