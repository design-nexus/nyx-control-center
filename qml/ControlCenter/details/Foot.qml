import QtQuick

// A detail view's footer: icon buttons at the right. The panel moves it below the
// scrolling view, so it's always in sight.
Row {
    readonly property bool detailFoot: true
    default property alias buttons: inner.data
    width: parent ? parent.width : 300
    layoutDirection: Qt.RightToLeft
    topPadding: 6
    Row {
        id: inner
        spacing: 2
    }
}
