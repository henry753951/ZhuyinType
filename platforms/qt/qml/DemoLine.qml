pragma ComponentBehavior: Bound

import QtQuick

// One line of text in a demo (DemoLine on macOS): sent text, the composition with its underlines, and the caret.
// `segments` are { text, style } with style
// - "document": text already sent;
// - "marked" (the default): composed text, a thin underline per word;
// - "focus": what the candidate window changes, a thick accent underline on an accent wash;
// - "target": the character ↓ would change, a thick accent underline;
// - "highlight": what the demo points at (the text read before the cursor), an accent wash;
// - "faded": text no longer used.
// The caret follows segment `caret` (-1: after everything).
Row {
    id: line
    required property Theme theme
    property var segments: []
    property int caret: -1
    property int fontSize: 15
    property bool showsCaret: true
    property bool blinks: true
    readonly property var composed: ["marked", "focus", "target", undefined, ""]
    spacing: 0
    Accessible.role: Accessible.StaticText
    Accessible.name: segments.map(segment => segment.text).join("")

    Repeater {
        model: line.segments
        delegate: Row {
            id: piece
            required property var modelData
            required property int index
            readonly property string style: modelData.style || "marked"
            readonly property bool isComposed: style === "marked" || style === "focus" || style === "target"
            readonly property var next: line.segments[index + 1]
            spacing: 0
            Item {
                implicitWidth: label.implicitWidth
                    // Words of a composition are underlined separately, with a small gap between them.
                    + (piece.isComposed && piece.next && line.composed.includes(piece.next.style) ? 2 : 0)
                implicitHeight: label.implicitHeight + 4
                Rectangle {
                    visible: piece.style === "focus" || piece.style === "highlight"
                    width: label.implicitWidth
                    height: label.implicitHeight
                    radius: piece.style === "highlight" ? 3 : 0
                    color: Qt.rgba(line.theme.accent.r, line.theme.accent.g, line.theme.accent.b, piece.style === "highlight" ? 0.22 : 0.18)
                }
                Text {
                    id: label
                    // A space keeps its width, as an en space does in the macOS demo.
                    text: String(piece.modelData.text).replace(/ /g, " ")
                    color: piece.style === "faded" ? line.theme.tertiary : line.theme.text
                    font.pixelSize: line.fontSize
                }
                Rectangle {
                    visible: piece.isComposed
                    y: label.implicitHeight + 1
                    width: label.implicitWidth
                    height: piece.style === "marked" ? 1 : 2.5
                    color: piece.style === "marked" ? Qt.rgba(line.theme.text.r, line.theme.text.g, line.theme.text.b, 0.75) : line.theme.accent
                }
            }
            Loader {
                active: line.showsCaret && line.caret === piece.index
                sourceComponent: caretBar
            }
        }
    }
    Loader {
        active: line.showsCaret && line.caret < 0
        sourceComponent: caretBar
    }
    Component {
        id: caretBar
        Item {
            implicitWidth: 2.5
            implicitHeight: line.fontSize * 1.25
            Rectangle {
                id: bar
                x: 0.5
                width: 1.5
                height: parent.height
                color: line.theme.text
                Timer {
                    interval: 530
                    repeat: true
                    running: line.blinks && line.theme.motionEnabled && !line.theme.reduceMotion
                    onTriggered: bar.visible = !bar.visible
                    onRunningChanged: bar.visible = true
                }
            }
        }
    }
}
