pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

// One series of bars in the accent (macOS Charts' BarMark): a leading axis with three gridlines, labels under some bars,
// the largest bar's number above it, and hover showing a bar's own text. `bars`: [{value, label (axis, may be ""),
// tip (hover)}]. Put it in a padded SectionCard and give it a height.
Item {
    id: chart
    required property Theme theme
    property var bars: []
    /// Annotate the largest bar with its number.
    property bool annotateMaximum: true
    /// A note at the trailing end under the axis, such as "毫秒以內".
    property string axisNote: ""
    property int hovered: -1
    readonly property real maximum: Math.max(1, ...bars.map(bar => bar.value))
    readonly property real step: niceStep(maximum / 2)
    readonly property real ceiling: Math.max(step * 2, Math.ceil(maximum / step) * step)
    readonly property int best: bars.reduce((at, bar, index) => bar.value > (at < 0 ? 0 : bars[at].value) ? index : at, -1)
    readonly property real axisWidth: axisMetrics.advanceWidth(format(ceiling)) + 8
    readonly property real plotTop: 16
    readonly property real plotHeight: height - plotTop - 18 - (axisNote ? 14 : 0)
    implicitHeight: 150

    function niceStep(raw) {
        if (raw <= 0)
            return 1;
        const power = Math.pow(10, Math.floor(Math.log10(raw)));
        const scaled = raw / power;
        return (scaled <= 1 ? 1 : scaled <= 2 ? 2 : scaled <= 2.5 ? 2.5 : scaled <= 5 ? 5 : 10) * power;
    }
    function format(value) {
        return Number(value).toLocaleString(Qt.locale("en_US"), "f", 0);
    }
    FontMetrics {
        id: axisMetrics
        font.pixelSize: chart.theme.captionSize
    }
    // Gridlines and their values.
    Repeater {
        model: 3
        delegate: Item {
            id: grid
            required property int index
            readonly property real value: chart.step * index
            y: chart.plotTop + chart.plotHeight * (1 - value / chart.ceiling)
            width: chart.width
            Rectangle {
                x: chart.axisWidth
                width: chart.width - chart.axisWidth
                height: 1
                color: chart.theme.line
            }
            Text {
                y: -height / 2
                width: chart.axisWidth - 6
                horizontalAlignment: Text.AlignRight
                text: chart.format(grid.value)
                color: chart.theme.tertiary
                font.pixelSize: chart.theme.captionSize
                font.features: ({ "tnum": 1 })
            }
        }
    }
    Row {
        id: plot
        x: chart.axisWidth
        y: chart.plotTop
        width: chart.width - chart.axisWidth
        height: chart.plotHeight
        Repeater {
            model: chart.bars
            delegate: Item {
                id: slot
                required property var modelData
                required property int index
                width: plot.width / Math.max(1, chart.bars.length)
                height: plot.height
                Rectangle {
                    anchors.bottom: parent.bottom
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: Math.max(1, parent.width * 0.62)
                    height: slot.modelData.value > 0 ? Math.max(2, slot.modelData.value / chart.ceiling * plot.height) : 0
                    radius: Math.min(3, width / 2)
                    color: chart.theme.accent
                    opacity: chart.hovered < 0 || chart.hovered === slot.index ? 0.85 : 0.35
                }
                Text {
                    visible: chart.annotateMaximum && chart.hovered < 0 && slot.index === chart.best && slot.modelData.value > 0
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: plot.height - slot.modelData.value / chart.ceiling * plot.height - height - 2
                    text: chart.format(slot.modelData.value)
                    color: chart.theme.secondary
                    font.pixelSize: chart.theme.captionSize
                    font.features: ({ "tnum": 1 })
                }
                Text {
                    visible: (slot.modelData.label || "").length > 0
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: plot.height + 4
                    text: slot.modelData.label || ""
                    color: chart.theme.tertiary
                    font.pixelSize: chart.theme.captionSize
                    font.features: ({ "tnum": 1 })
                }
                HoverHandler {
                    onHoveredChanged: chart.hovered = hovered ? slot.index : (chart.hovered === slot.index ? -1 : chart.hovered)
                }
                ToolTip.visible: chart.hovered === slot.index && (slot.modelData.tip || "").length > 0
                ToolTip.text: slot.modelData.tip || ""
                ToolTip.delay: 0
                Accessible.role: Accessible.StaticText
                Accessible.name: slot.modelData.tip || ""
            }
        }
    }
    Text {
        visible: chart.axisNote.length > 0
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        text: chart.axisNote
        color: chart.theme.tertiary
        font.pixelSize: chart.theme.captionSize
    }
}
