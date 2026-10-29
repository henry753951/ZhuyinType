import QtQuick

// 常用程度 of a learned word, 1–5: five small rising bars, the used ones in the secondary colour (UsageMeter on macOS).
// The engine keeps a weight that fades with time, not a count, so no number is shown.
Row {
    id: meter
    required property Theme theme
    property int level: 1
    readonly property var labels: ["很少", "偶爾", "有時", "常用", "很常用"]
    spacing: 2
    height: 12
    Accessible.role: Accessible.Indicator
    Accessible.name: "常用程度 " + level + " / 5"
    Repeater {
        model: 5
        delegate: Rectangle {
            required property int index
            anchors.bottom: parent ? parent.bottom : undefined
            width: 3
            height: 4 + (index + 1) * 1.6
            radius: 1
            color: index < meter.level ? meter.theme.secondary : meter.theme.track
        }
    }
}
