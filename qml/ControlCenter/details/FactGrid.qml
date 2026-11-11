import QtQuick

// Facts as small cards, two to a row.
Flow {
    readonly property bool factGrid: true
    width: parent ? parent.width : 300
    spacing: 6
}
