import QtQuick
import Nyx.Ui

// A detail view's head: back, the title, and its switch.
Item {
    id: h
    property string title: ""
    property bool hasSwitch: true
    property bool checked: false
    property bool switchEnabled: true

    signal back
    signal toggled(bool on)

    width: parent ? parent.width : 300
    height: 40
    IconButton {
        id: backButton
        anchors.verticalCenter: parent.verticalCenter
        icon: "go-previous-symbolic"
        tooltip: "Back (Esc)"
        onClicked: h.back()
    }
    Label {
        anchors.left: backButton.right
        anchors.leftMargin: 4
        anchors.verticalCenter: parent.verticalCenter
        text: h.title
        size: 13.5
        font.weight: Font.DemiBold
    }
    Toggle {
        visible: h.hasSwitch
        enabled: h.switchEnabled
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        checked: h.checked
        onToggled: on => h.toggled(on)
    }
}
