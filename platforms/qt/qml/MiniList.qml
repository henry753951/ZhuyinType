pragma ComponentBehavior: Bound

import QtQuick

// A few candidates under a demo line, in the candidate window's look (MiniList on macOS): numbered, the first one
// selected. "今 ㄐㄧㄣ" is a candidate with its corrected reading after it.
Rectangle {
    id: list
    required property Theme theme
    property var items: []
    implicitWidth: Math.max(92, column.implicitWidth + 8)
    implicitHeight: column.implicitHeight + 8
    radius: 9
    color: theme.surface
    border.color: theme.line
    Column {
        id: column
        x: 4
        y: 4
        Repeater {
            model: list.items
            delegate: Rectangle {
                id: item
                required property string modelData
                required property int index
                readonly property bool selected: index === 0
                readonly property var parts: modelData.split(" ")
                width: Math.max(84, content.implicitWidth + 12)
                height: 20
                radius: 5
                color: selected ? list.theme.selection : "transparent"
                Row {
                    id: content
                    x: 6
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 6
                    Text {
                        anchors.baseline: word.baseline
                        text: item.index + 1
                        color: item.selected ? Qt.rgba(1, 1, 1, 0.85) : list.theme.secondary
                        font.pixelSize: 10
                        font.weight: Font.Medium
                        font.features: ({ "tnum": 1 })
                    }
                    Text {
                        id: word
                        text: item.parts[0]
                        color: item.selected ? list.theme.accentText : list.theme.text
                        font.pixelSize: 13
                    }
                    Text {
                        visible: item.parts.length > 1
                        anchors.baseline: word.baseline
                        text: item.parts.slice(1).join(" ")
                        color: item.selected ? Qt.rgba(1, 1, 1, 0.8) : list.theme.tertiary
                        font.pixelSize: 10
                    }
                }
            }
        }
    }
}
