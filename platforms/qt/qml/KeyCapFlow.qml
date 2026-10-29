import QtQuick

// A set of KeyCaps that wraps when space runs out. `alignRight` keeps them against the trailing edge (in a ListRow);
// the reading order of `keys` is kept either way. Its implicit width is the width of all caps on one line.
Item {
    id: caps
    required property Theme theme
    property var keys: []
    property bool alignRight: true
    property int spacing: 6
    implicitWidth: keys.reduce((sum, key) => sum + Math.max(24, Math.ceil(metrics.advanceWidth(key)) + 16) + spacing, -spacing)
    implicitHeight: flow.implicitHeight
    Accessible.ignored: true
    FontMetrics {
        id: metrics
        font.pixelSize: caps.theme.detailSize
        font.weight: Font.Medium
    }
    Flow {
        id: flow
        width: caps.width
        layoutDirection: caps.alignRight ? Qt.RightToLeft : Qt.LeftToRight
        spacing: caps.spacing
        Repeater {
            // Right to left lays out the last key first, so the model is reversed to keep the reading order.
            model: caps.alignRight ? caps.keys.slice().reverse() : caps.keys
            delegate: KeyCap {
                required property string modelData
                theme: caps.theme
                text: modelData
                width: Math.min(implicitWidth, caps.width)
            }
        }
    }
}
