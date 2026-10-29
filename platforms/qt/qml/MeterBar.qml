import QtQuick

// A part of a whole as a thin bar in the accent, for the trailing side of a SettingsRow: `value` of `maximum`
// (Breakdown on macOS), or with `second` two parts of one whole — the first in the accent, the rest grey (SplitBar).
Item {
    id: meter
    required property Theme theme
    property real value: 0
    property real maximum: 1
    /// When set (≥ 0), the bar is split: `value` against `second`.
    property real second: -1
    implicitWidth: second >= 0 ? 160 : 140
    implicitHeight: 14
    Accessible.ignored: true
    readonly property real fraction: second >= 0 ? value / Math.max(1, value + second) : value / Math.max(1, maximum)
    Rectangle {
        visible: meter.second >= 0
        anchors.verticalCenter: parent.verticalCenter
        x: first.width + (first.width > 0 ? 2 : 0)
        width: parent.width - x
        height: 6
        radius: 3
        color: meter.theme.track
    }
    Rectangle {
        id: first
        anchors.verticalCenter: parent.verticalCenter
        width: meter.value > 0 ? Math.max(3, (meter.width - (meter.second >= 0 ? 2 : 0)) * meter.fraction) : 0
        height: 6
        radius: 3
        color: meter.theme.accent
        opacity: 0.8
    }
}
