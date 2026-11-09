import QtQuick

// A detail view's footer: icon buttons at the right.
Row {
    default property alias buttons: inner.data
    width: parent ? parent.width : 300
    layoutDirection: Qt.RightToLeft
    topPadding: 6
    Row {
        id: inner
        spacing: 2
    }
}
