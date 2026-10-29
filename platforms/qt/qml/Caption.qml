import QtQuick
import QtQuick.Layouts

// A note under a card or a control: one or two lines of small secondary text that wrap.
Text {
    required property Theme theme
    Layout.fillWidth: true
    Layout.leftMargin: 4
    color: theme.secondary
    font.pixelSize: theme.detailSize
    lineHeight: 1.15
    wrapMode: Text.Wrap
}
