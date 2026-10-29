import QtQuick
import QtQuick.Layouts

// One choice of a setting with its demo, inside an ⓘ card (ChoiceDemo on macOS): the choice's name, 「目前的設定」 on
// the one in use, and its MiniDemo.
ColumnLayout {
    id: choice
    required property Theme theme
    property string title: ""
    property bool selected: false
    property var frames: []
    property int pinned: -1
    Layout.fillWidth: true
    spacing: 6
    Row {
        spacing: 6
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: choice.title
            color: choice.theme.text
            font.pixelSize: choice.theme.detailSize
            font.weight: Font.DemiBold
        }
        Chip {
            visible: choice.selected
            anchors.verticalCenter: parent.verticalCenter
            theme: choice.theme
            tone: "accent"
            text: "目前的設定"
        }
    }
    MiniDemo {
        theme: choice.theme
        frames: choice.frames
        pinned: choice.pinned
    }
}
