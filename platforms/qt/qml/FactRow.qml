import QtQuick
import QtQuick.Layouts

// One fact of an ⓘ card (FactRow on macOS): a line icon in the accent and a sentence that wraps.
RowLayout {
    id: fact
    required property Theme theme
    property string iconName: ""
    property string text: ""
    Layout.fillWidth: true
    spacing: 10
    Icon {
        Layout.alignment: Qt.AlignTop
        Layout.topMargin: 1
        name: fact.iconName
        size: 16
        color: fact.theme.accent
    }
    Text {
        Layout.fillWidth: true
        text: fact.text
        color: fact.theme.text
        font.pixelSize: fact.theme.bodySize
        wrapMode: Text.Wrap
        textFormat: Text.PlainText
    }
}
