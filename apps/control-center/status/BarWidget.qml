import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Networking
import Quickshell.Bluetooth
import Quickshell.Services.Pipewire
import Quickshell.Services.UPower
import qs.Commons
import qs.Ui

// Nyx Status: what the bar should show at a glance, with the controls left to the Nyx
// Control Center. Network, Bluetooth, sound and battery come from the shell's own
// services (nothing extra runs); CPU and memory from /proc every 3 seconds; the weather
// from Nyx Weather's cache every 10 minutes.
BarWidget {
  id: root
  moduleName: "design-nexus.status"

  function setting(key, fallback) {
    var v = root.settings ? root.settings[key] : undefined
    return v === undefined ? fallback : v
  }
  function run(cmd) { if (root.bar) root.bar.run(cmd) }

  // ---- Weather
  property string weatherText: ""
  property string weatherTip: ""

  // ---- CPU and memory
  property var cpuPrev: null
  property int cpuPercent: -1
  property int memPercent: -1
  property string memTip: ""
  // Found once at start: the CPU package temperature, and a GPU's busy counter and
  // temperature where sysfs has them (AMD, Intel Xe; not NVIDIA's driver, which would
  // need nvidia-smi and wake the GPU).
  property string cpuTempPath: ""
  property string gpuBusyPath: ""
  property string gpuTempPath: ""
  property int cpuTemp: -1
  property int gpuPercent: -1
  property int gpuTemp: -1
  property int diskPercent: -1
  property string diskTip: ""

  function readCpu(raw) {
    var f = String(raw || "").split("\n")[0].trim().split(/\s+/)
    if (f[0] !== "cpu") return
    var total = 0
    for (var i = 1; i <= 8; i++) total += Number(f[i] || 0)
    var idle = Number(f[4] || 0) + Number(f[5] || 0)
    if (cpuPrev && total > cpuPrev.total)
      cpuPercent = Math.round(100 * (1 - (idle - cpuPrev.idle) / (total - cpuPrev.total)))
    cpuPrev = { total: total, idle: idle }
  }
  function readMemory(raw) {
    var v = ({})
    var lines = String(raw || "").split("\n")
    for (var i = 0; i < lines.length; i++) {
      var m = lines[i].match(/^(\w+):\s+(\d+)/)
      if (m) v[m[1]] = Number(m[2])
    }
    if (!v.MemTotal) return
    var used = v.MemTotal - (v.MemAvailable || 0)
    memPercent = Math.round(100 * used / v.MemTotal)
    memTip = (used / 1048576).toFixed(1) + " of " + (v.MemTotal / 1048576).toFixed(0) + " GB"
  }

  // ---- Network (as Omarchy's own network widget reads it)
  readonly property var netDevices: Networking.devices ? Networking.devices.values : []
  function device(type) {
    var fallback = null
    for (var i = 0; i < netDevices.length; i++) {
      var d = netDevices[i]
      if (!d || d.type !== type) continue
      if (d.connected) return d
      if (!fallback) fallback = d
    }
    return fallback
  }
  readonly property var wired: device(DeviceType.Wired)
  readonly property var wifi: device(DeviceType.Wifi)
  readonly property var wifiNetwork: {
    var nets = wifi && wifi.networks ? wifi.networks.values : []
    for (var i = 0; i < nets.length; i++) if (nets[i] && nets[i].connected) return nets[i]
    return null
  }
  readonly property string netKind: wired && wired.connected ? "ethernet"
    : (wifiNetwork || (wifi && wifi.connected)) ? "wifi" : "none"
  readonly property bool restricted: Networking.connectivity === NetworkConnectivity.Portal
    || Networking.connectivity === NetworkConnectivity.Limited
  readonly property string netIcon: {
    if (netKind === "ethernet") return restricted ? "󰈂" : "󰈀"
    if (netKind === "wifi") {
      if (restricted) return "󰤩"
      var s = wifiNetwork ? Math.round(wifiNetwork.signalStrength * 100) : 60
      return ["󰤯", "󰤟", "󰤢", "󰤥", "󰤨"][Math.max(0, Math.min(4, Math.ceil(s / 20) - 1))]
    }
    return "󰤮"
  }
  readonly property string netTip: netKind === "ethernet" ? "Ethernet"
    : netKind === "wifi" ? (wifiNetwork ? wifiNetwork.name : "Wi-Fi")
    : "Not connected"

  // ---- Bluetooth
  readonly property var adapter: Bluetooth.defaultAdapter
  readonly property var btConnected: {
    var out = []
    var ds = Bluetooth.devices ? Bluetooth.devices.values : []
    for (var i = 0; i < ds.length; i++) if (ds[i] && ds[i].connected) out.push(ds[i].name || ds[i].address)
    return out
  }
  readonly property string btIcon: !adapter ? "" : !adapter.enabled ? "󰂲" : btConnected.length > 0 ? "󰂱" : "󰂯"

  // ---- Sound
  readonly property var sink: Pipewire.defaultAudioSink
  PwObjectTracker { objects: root.sink ? [root.sink] : [] }
  readonly property bool muted: !!(sink && sink.audio && sink.audio.muted)
  readonly property real volume: sink && sink.audio ? sink.audio.volume : 0
  readonly property string soundIcon: {
    if (!sink || !sink.audio) return ""
    if (muted) return "\ueee8"
    if (volume >= 0.67) return "\uf028"
    if (volume >= 0.34) return "\uf027"
    if (volume > 0) return "\uf026"
    return "\ueee8"
  }

  // ---- Battery
  readonly property var battery: UPower.displayDevice
  readonly property bool hasBattery: !!(battery && battery.isPresent)
  readonly property string batteryIcon: {
    if (!hasBattery) return ""
    var i = Math.max(0, Math.min(9, Math.floor(battery.percentage * 10)))
    if (battery.state === UPowerDeviceState.FullyCharged) return "󰂅"
    if (!UPower.onBattery) return ["󰢜", "󰂆", "󰂇", "󰂈", "󰢝", "󰂉", "󰢞", "󰂊", "󰂋", "󰂅"][i]
    return ["󰁺", "󰁻", "󰁼", "󰁽", "󰁾", "󰁿", "󰂀", "󰂁", "󰂂", "󰁹"][i]
  }

  implicitWidth: items.implicitWidth
  implicitHeight: items.implicitHeight

  Grid {
    id: items
    anchors.centerIn: parent
    columns: root.vertical ? 1 : 8
    rows: root.vertical ? 8 : 1

    WidgetButton {
      visible: root.setting("weather", true) && root.weatherText !== ""
      bar: root.bar
      text: root.vertical ? root.weatherText.split(" ")[0] : root.weatherText
      horizontalMargin: 6
      tooltipText: root.weatherTip
      onPressed: function(b) {
        root.run(b === Qt.RightButton ? "nyx-weather --section settings" : "nyx-weather --toggle")
      }
    }

    WidgetButton {
      visible: root.setting("system", true) && root.cpuPercent >= 0 && !root.vertical
      bar: root.bar
      text: {
        var parts = ["\u{f0ee0} " + root.cpuPercent + "%"]
        if (root.setting("temperature", true) && root.cpuTemp > 0) parts[0] += " " + root.cpuTemp + "°"
        parts.push("\u{f035b} " + root.memPercent + "%")
        if (root.setting("gpu", true) && root.gpuPercent >= 0)
          parts.push("\u{f08ae} " + root.gpuPercent + "%" + (root.gpuTemp > 0 ? " " + root.gpuTemp + "°" : ""))
        if (root.setting("disk", false) && root.diskPercent >= 0) parts.push("\u{f02ca} " + root.diskPercent + "%")
        return parts.join("  ")
      }
      horizontalMargin: 6
      tooltipText: {
        var lines = ["CPU " + root.cpuPercent + "%" + (root.cpuTemp > 0 ? " at " + root.cpuTemp + " °C" : ""),
                     "Memory " + root.memTip]
        if (root.gpuPercent >= 0) lines.push("GPU " + root.gpuPercent + "%" + (root.gpuTemp > 0 ? " at " + root.gpuTemp + " °C" : ""))
        if (root.diskPercent >= 0) lines.push("Disk " + root.diskTip)
        lines.push("Click for Activity; right-click for its processes")
        return lines.join("\n")
      }
      onPressed: function(b) {
        root.run(b === Qt.RightButton ? "nyx-activity --section processes" : "nyx-activity --toggle")
      }
    }

    BarIconButton {
      visible: root.setting("network", true)
      bar: root.bar
      text: root.netIcon
      active: root.restricted
      tooltipText: root.restricted ? root.netTip + " · limited access" : root.netTip
      onPressed: function(b) { root.run("nyx-control-center --toggle wifi") }
    }

    BarIconButton {
      visible: root.setting("bluetooth", true) && root.btIcon !== ""
      bar: root.bar
      text: root.btIcon
      dimmed: !!root.adapter && !root.adapter.enabled
      tooltipText: !root.adapter || !root.adapter.enabled ? "Bluetooth off"
        : root.btConnected.length > 0 ? root.btConnected.join(", ") : "Bluetooth on"
      onPressed: function(b) { root.run("nyx-control-center --toggle bluetooth") }
    }

    BarIconButton {
      visible: root.setting("sound", true) && root.soundIcon !== ""
      bar: root.bar
      text: root.soundIcon
      tooltipText: root.muted ? "Muted" : "Volume " + Math.round(root.volume * 100) + "%"
      onPressed: function(b) {
        if (b === Qt.RightButton) { if (root.sink && root.sink.audio) root.sink.audio.muted = !root.muted }
        else root.run("nyx-control-center --toggle")
      }
      onWheelMoved: function(delta) {
        if (!root.sink || !root.sink.audio) return
        var v = Math.max(0, Math.min(1, root.volume + (delta > 0 ? 0.05 : -0.05)))
        root.sink.audio.volume = Math.round(v * 20) / 20
      }
    }

    BarIconButton {
      visible: root.setting("battery", true) && root.hasBattery
      bar: root.bar
      text: root.batteryIcon
      labelText: root.setting("batteryPercent", true) && !root.vertical && root.hasBattery
        ? Math.round(root.battery.percentage * 100) + "%" : ""
      slotSize: Style.bar.iconSlot * (labelText !== "" ? 2 : 1)
      active: root.hasBattery && UPower.onBattery && root.battery.percentage < 0.15
      tooltipText: !root.hasBattery ? "" : UPower.onBattery ? "On battery" : "Plugged in"
      onPressed: function(b) { root.run("nyx-control-center --toggle") }
    }
  }

  FileView { id: stat; path: "/proc/stat"; watchChanges: false; printErrors: false; onLoaded: root.readCpu(text()) }
  FileView { id: meminfo; path: "/proc/meminfo"; watchChanges: false; printErrors: false; onLoaded: root.readMemory(text()) }
  FileView {
    id: cpuTempFile; path: root.cpuTempPath; watchChanges: false; printErrors: false
    onLoaded: { var v = Number(String(text()).trim()) / 1000; root.cpuTemp = isFinite(v) && v > 0 ? Math.round(v) : -1 }
  }
  FileView {
    id: gpuBusyFile; path: root.gpuBusyPath; watchChanges: false; printErrors: false
    onLoaded: { var v = Number(String(text()).trim()); root.gpuPercent = isFinite(v) && v >= 0 ? Math.round(Math.min(v, 100)) : -1 }
  }
  FileView {
    id: gpuTempFile; path: root.gpuTempPath; watchChanges: false; printErrors: false
    onLoaded: { var v = Number(String(text()).trim()) / 1000; root.gpuTemp = isFinite(v) && v > 0 ? Math.round(v) : -1 }
  }
  Timer {
    interval: 3000
    repeat: true
    running: root.setting("system", true)
    triggeredOnStart: true
    onTriggered: {
      stat.reload(); meminfo.reload()
      if (root.cpuTempPath !== "" && root.setting("temperature", true)) cpuTempFile.reload()
      if (root.gpuBusyPath !== "" && root.setting("gpu", true)) gpuBusyFile.reload()
      if (root.gpuTempPath !== "" && root.setting("gpu", true)) gpuTempFile.reload()
    }
  }

  // Where the sensors are (once).
  Process {
    running: true
    command: ["bash", "-c",
      "for h in /sys/class/hwmon/hwmon*; do n=$(cat $h/name 2>/dev/null); case $n in coretemp|k10temp|zenpower) " +
      "for l in $h/temp*_label; do case $(cat $l) in 'Package id 0'|Tctl) echo cpu ${l%_label}_input; break 2;; esac; done; " +
      "echo cpu $h/temp1_input; break;; esac; done; " +
      "for c in /sys/class/drm/card[0-9]; do d=$c/device; [ -r $d/gpu_busy_percent ] && echo busy $d/gpu_busy_percent; " +
      "for t in $d/hwmon/hwmon*/temp1_input $d/hwmon/hwmon*/temp2_input; do [ -r $t ] && [ -r $d/gpu_busy_percent ] && echo gputemp $t && break; done; done"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var lines = String(text).split("\n")
        for (var i = 0; i < lines.length; i++) {
          var f = lines[i].split(" ")
          if (f[0] === "cpu" && root.cpuTempPath === "") root.cpuTempPath = f[1]
          if (f[0] === "busy" && root.gpuBusyPath === "") root.gpuBusyPath = f[1]
          if (f[0] === "gputemp" && root.gpuTempPath === "") root.gpuTempPath = f[1]
        }
      }
    }
  }

  // How full the root disk is, once a minute.
  Process {
    id: diskPoll
    command: ["df", "-P", "-B1", "/"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var f = String(text).trim().split("\n").pop().split(/\s+/)
        var used = Number(f[2]), total = Number(f[1])
        if (total > 0) {
          root.diskPercent = Math.round(100 * used / total)
          root.diskTip = (used / 1e9).toFixed(0) + " of " + (total / 1e9).toFixed(0) + " GB used"
        }
      }
    }
  }
  Timer {
    interval: 60000
    repeat: true
    running: root.setting("system", true) && root.setting("disk", false)
    triggeredOnStart: true
    onTriggered: diskPoll.running = true
  }

  Process {
    id: weatherPoll
    command: ["nyx-weather", "--waybar"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var out = JSON.parse(text)
          if (out.text) { root.weatherText = out.text; root.weatherTip = out.tooltip || "" }
        } catch (e) {}
      }
    }
  }
  Timer {
    interval: 10 * 60 * 1000
    repeat: true
    running: root.setting("weather", true)
    triggeredOnStart: true
    onTriggered: weatherPoll.running = true
  }
}
