import QtQuick
import QtQuick.Controls.Basic

// A multi-line field for writing or trying out text.
TextArea {
    id: area
    required property Theme theme
    color: theme.text
    placeholderTextColor: theme.tertiary
    selectionColor: theme.accent
    selectedTextColor: theme.accentText
    font.pixelSize: theme.displaySize
    padding: 14
    wrapMode: TextEdit.Wrap
    selectByMouse: true
    background: Rectangle {
        radius: area.theme.cardRadius - 2
        color: area.theme.field
        border.width: area.activeFocus ? 2 : 1
        border.color: area.activeFocus ? area.theme.focusRing : area.theme.fieldBorder
        Behavior on border.color {
            ColorAnimation {
                duration: area.theme.motionEnabled ? 120 : 0
            }
        }
    }
}
