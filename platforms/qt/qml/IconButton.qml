import QtQuick
import QtQuick.Controls.Basic

// A round, borderless button that is only an icon (help, clear, close). Always give it an Accessible.name.
AbstractButton {
    id: button
    required property Theme theme
    property string iconName: ""
    property real iconSize: 17
    property bool accented: false
    implicitWidth: 28
    implicitHeight: 28
    focusPolicy: Qt.TabFocus
    opacity: enabled ? 1 : 0.4
    ToolTip.visible: hovered && Accessible.name.length > 0
    ToolTip.text: Accessible.name
    ToolTip.delay: 600
    background: Rectangle {
        radius: width / 2
        color: button.down ? button.theme.controlPressed : button.hovered ? button.theme.hover : "transparent"
        border.width: button.visualFocus ? 2 : 0
        border.color: button.theme.focusRing
    }
    contentItem: Item {
        Icon {
            anchors.centerIn: parent
            name: button.iconName
            size: button.iconSize
            color: button.accented || button.hovered ? button.theme.accent : button.theme.tertiary
        }
    }
}
