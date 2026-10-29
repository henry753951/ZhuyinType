import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// One entry of the window's sidebar: a line icon and a title. The current page is tinted with the accent.
Button {
    id: item
    required property Theme theme
    property string iconName: ""
    property string title: ""
    Layout.fillWidth: true
    implicitHeight: 32
    leftPadding: 10
    rightPadding: 10
    topPadding: 0
    bottomPadding: 0
    Accessible.name: title
    background: Rectangle {
        radius: item.theme.controlRadius
        color: item.checked ? item.theme.accentSoft : item.down ? item.theme.controlPressed : item.hovered ? item.theme.hover : "transparent"
        border.width: item.visualFocus ? 2 : 0
        border.color: item.theme.focusRing
        Behavior on color {
            ColorAnimation {
                duration: item.theme.motionEnabled ? 110 : 0
            }
        }
    }
    contentItem: RowLayout {
        spacing: 10
        Icon {
            name: item.iconName
            color: item.checked ? item.theme.accent : item.theme.secondary
            size: 18
        }
        Text {
            text: item.title
            color: item.checked ? item.theme.accent : item.theme.text
            font {
                pixelSize: item.theme.bodySize
                weight: item.checked ? Font.DemiBold : Font.Normal
            }
            Layout.fillWidth: true
            elide: Text.ElideRight
        }
    }
}
