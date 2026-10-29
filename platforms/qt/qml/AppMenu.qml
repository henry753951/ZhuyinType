import QtQuick
import QtQuick.Controls.Basic

// A context menu: a rounded, shadowed card whose items highlight in the accent.
Menu {
    id: menu
    required property Theme theme
    padding: 5
    font.pixelSize: theme.bodySize
    delegate: MenuItem {
        id: item
        implicitHeight: 28
        implicitWidth: Math.max(160, label.implicitWidth + 28)
        leftPadding: 10
        rightPadding: 10
        contentItem: Text {
            id: label
            text: item.text
            font: menu.font
            color: !item.enabled ? menu.theme.tertiary : item.highlighted ? menu.theme.accentText : menu.theme.text
            verticalAlignment: Text.AlignVCenter
        }
        background: Rectangle {
            radius: 6
            color: item.highlighted && item.enabled ? menu.theme.accent : "transparent"
        }
    }
    background: Rectangle {
        implicitWidth: 170
        radius: 10
        color: menu.theme.surface
        border.color: menu.theme.line
        Rectangle {
            z: -1
            anchors.fill: parent
            anchors.margins: -1
            anchors.topMargin: 4
            radius: 11
            color: menu.theme.shadow
        }
    }
}
