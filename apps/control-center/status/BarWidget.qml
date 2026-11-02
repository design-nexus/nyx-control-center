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
      text: "󰻠 " + root.cpuPercent + "%  󰍛 " + root.memPercent + "%"
      horizontalMargin: 6
      tooltipText: "CPU " + root.cpuPercent + "% · Memory " + root.memTip
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
  Timer {
    interval: 3000
    repeat: true
    running: root.setting("system", true)
    triggeredOnStart: true
    onTriggered: { stat.reload(); meminfo.reload() }
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
