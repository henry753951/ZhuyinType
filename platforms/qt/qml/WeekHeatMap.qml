pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts

// Characters by weekday and hour (macOS WeekHeatMap): seven rows from Sunday, 24 cells each, one colour that is darker
// for more. `hours`: 168 counts, index weekday × 24 + hour.
ColumnLayout {
    id: map
    required property Theme theme
    property var hours: []
    readonly property real peak: Math.max(1, ...hours)
    readonly property var weekdays: ["日", "一", "二", "三", "四", "五", "六"]
    Layout.fillWidth: true
    spacing: 2
    Accessible.role: Accessible.Graphic
    Accessible.name: "一週中每小時打的字數"
    Repeater {
        model: 7
        delegate: RowLayout {
            id: day
            required property int index
            Layout.fillWidth: true
            spacing: 2
            Text {
                Layout.preferredWidth: 14
                text: map.weekdays[day.index]
                color: map.theme.secondary
                font.pixelSize: map.theme.captionSize - 1
            }
            Repeater {
                model: 24
                delegate: Rectangle {
                    required property int index
                    readonly property real value: map.hours[day.index * 24 + index] || 0
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    implicitHeight: 11
                    radius: 2
                    color: value === 0 ? map.theme.track : map.theme.accent
                    opacity: value === 0 ? 0.35 : 0.15 + 0.85 * value / map.peak
                }
            }
        }
    }
    RowLayout {
        Layout.fillWidth: true
        spacing: 0
        Item {
            Layout.preferredWidth: 16
        }
        Repeater {
            model: [0, 6, 12, 18]
            delegate: Text {
                required property int modelData
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                text: modelData
                color: map.theme.tertiary
                font.pixelSize: map.theme.captionSize - 1
            }
        }
    }
}
