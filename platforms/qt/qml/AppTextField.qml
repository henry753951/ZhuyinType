import QtQuick
import QtQuick.Controls.Basic

// A single-line field. `search` adds the magnifier and a clear button.
TextField {
    id: field
    required property Theme theme
    property bool search: false
    implicitHeight: theme.controlHeight + 2
    leftPadding: search ? 32 : 10
    rightPadding: search && text.length > 0 ? 32 : 10
    color: theme.text
    placeholderTextColor: theme.tertiary
    selectionColor: theme.accent
    selectedTextColor: theme.accentText
    selectByMouse: true
    font.pixelSize: theme.bodySize
    verticalAlignment: TextInput.AlignVCenter
    opacity: enabled ? 1 : 0.5
    background: Rectangle {
        radius: field.theme.controlRadius
        color: field.theme.field
        border.width: field.activeFocus ? 2 : 1
        border.color: field.activeFocus ? field.theme.focusRing : field.theme.fieldBorder
        Icon {
            visible: field.search
            name: "search"
            color: field.theme.tertiary
            size: 16
            x: 10
            anchors.verticalCenter: parent.verticalCenter
        }
    }
    AbstractButton {
        objectName: "searchClear"
        visible: field.search && field.text.length > 0
        anchors.right: parent.right
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        width: 22
        height: 22
        focusPolicy: Qt.NoFocus
        Accessible.name: "清除搜尋"
        background: Rectangle {
            radius: 11
            color: parent.hovered ? field.theme.controlHover : "transparent"
        }
        contentItem: Icon {
            name: "close"
            color: field.theme.secondary
            size: 14
        }
        onClicked: field.clear()
    }
}
