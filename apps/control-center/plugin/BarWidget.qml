import QtQuick
import qs.Ui

// Opens or closes the Nyx Control Center (a running user service). Nothing polls here.
BarWidget {
  id: root
  moduleName: "design-nexus.control-center"

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: ""
    horizontalMargin: 7.5
    onPressed: function(button) {
      if (root.bar) root.bar.run("nyx-control-center --toggle")
    }
  }
}
