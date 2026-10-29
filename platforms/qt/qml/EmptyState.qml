import QtQuick
import QtQuick.Layouts

// What a list or a chart shows when it has nothing yet: an optional icon, an optional heading, and one short
// sentence, centred and quiet (the empty list card on macOS: tray, 「還沒有…」, what to do).
ColumnLayout {
    id: empty
    required property Theme theme
    property string title: ""
    property string text: ""
    property string iconName: ""
    spacing: 6
    Icon {
        visible: empty.iconName.length > 0
        Layout.alignment: Qt.AlignHCenter
        Layout.bottomMargin: 2
        name: empty.iconName
        size: 26
        weight: 1.4
        color: empty.theme.tertiary
    }
    Text {
        Layout.fillWidth: true
        visible: text.length > 0
        horizontalAlignment: Text.AlignHCenter
        text: empty.title
        color: empty.theme.secondary
        font.pixelSize: empty.theme.bodySize
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
    }
    Text {
        Layout.fillWidth: true
        visible: text.length > 0
        horizontalAlignment: Text.AlignHCenter
        text: empty.text
        color: empty.theme.tertiary
        font.pixelSize: empty.title.length > 0 ? empty.theme.detailSize : empty.theme.bodySize
        wrapMode: Text.Wrap
    }
}
