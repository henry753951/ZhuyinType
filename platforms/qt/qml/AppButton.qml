import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The one push button of the desktop client.
//   secondary (default) — a soft grey fill for ordinary actions
//   primary             — the accent fill, at most one per view or dialog
//   plain               — text only, for in-line and low-weight actions
//   danger              — a destructive action, red text on the grey fill
Button {
    id: button
    required property Theme theme
    property string kind: "secondary"
    property string iconName: ""
    property bool compact: false
    readonly property color fill: kind === "primary" ? (down ? Qt.darker(theme.accent, 1.12) : hovered ? theme.accentHover : theme.accent) : kind === "plain" ? (down ? theme.controlPressed : hovered ? theme.hover : "transparent") : down ? theme.controlPressed : hovered ? theme.controlHover : theme.control
    readonly property color ink: kind === "primary" ? theme.accentText : kind === "danger" ? theme.danger : kind === "plain" ? theme.accent : theme.text
    implicitHeight: compact ? 26 : theme.controlHeight
    implicitWidth: Math.max(implicitHeight, implicitContentWidth + leftPadding + rightPadding)
    leftPadding: text.length ? (compact ? 10 : 14) : 6
    rightPadding: text.length ? (compact ? 10 : 14) : 6
    topPadding: 0
    bottomPadding: 0
    opacity: enabled ? 1 : 0.45
    font.pixelSize: compact ? theme.detailSize : theme.bodySize
    Accessible.name: text
    contentItem: RowLayout {
        spacing: 6
        Icon {
            visible: button.iconName.length > 0
            name: button.iconName
            color: button.ink
            size: button.compact ? 14 : 16
            Layout.alignment: Qt.AlignVCenter
        }
        Text {
            visible: button.text.length > 0
            text: button.text
            color: button.ink
            font: button.font
            elide: Text.ElideRight
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
        }
    }
    background: Rectangle {
        radius: button.theme.controlRadius
        color: button.fill
        border.width: button.visualFocus ? 2 : button.kind === "secondary" || button.kind === "danger" ? 1 : 0
        border.color: button.visualFocus ? button.theme.focusRing : button.theme.controlBorder
        Behavior on color {
            ColorAnimation {
                duration: button.theme.motionEnabled ? 110 : 0
            }
        }
    }
}
