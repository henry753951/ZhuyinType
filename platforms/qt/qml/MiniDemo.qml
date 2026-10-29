pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts

// A small text field that plays `frames` in a loop, with the key just pressed and a caption under it (MiniDemo on
// macOS). Use it for every short demo: the ⓘ cards, 模糊音. A frame is
// { keys: ["ㄙ"], line: [{ text, style }] (see DemoLine), caret, list: ["今 ㄐㄧㄣ", …], caption, hold (ms) }.
// Under reduced motion it rests on `restingFrame` (the last frame when unset) until played; `pinned` draws one
// frame still, for the renders.
ColumnLayout {
    id: demo
    required property Theme theme
    property var frames: []
    property int restingFrame: -1
    property int pinned: -1
    property int frameIndex: 0
    property bool playing: !theme.reduceMotion && theme.motionEnabled
    readonly property var frame: frames.length === 0 ? ({ line: [], caption: "" }) : frames[Math.max(0, Math.min(pinned >= 0 ? pinned : frameIndex, frames.length - 1))]
    readonly property bool hasList: frames.some(item => !!item.list)
    objectName: "miniDemo"
    spacing: 8
    Layout.fillWidth: true
    Accessible.role: Accessible.Animation
    Accessible.name: frames.map(item => item.caption).join("，")
    onFramesChanged: frameIndex = playing ? 0 : rest()
    onPlayingChanged: if (!playing) frameIndex = rest()
    Component.onCompleted: if (!playing) frameIndex = rest()

    function rest() {
        return restingFrame >= 0 ? restingFrame : Math.max(0, frames.length - 1);
    }
    Timer {
        interval: demo.frame.hold || 900
        running: demo.visible && demo.playing && demo.pinned < 0 && demo.frames.length > 1
        repeat: true
        onTriggered: demo.frameIndex = (demo.frameIndex + 1) % demo.frames.length
    }
    Rectangle {
        Layout.fillWidth: true
        implicitHeight: demo.hasList ? 112 : 38
        radius: 7
        color: demo.theme.field
        border.color: demo.theme.line
        clip: true
        Column {
            x: 10
            y: 8
            spacing: 4
            DemoLine {
                theme: demo.theme
                segments: demo.frame.line || []
                caret: demo.frame.caret === undefined ? -1 : demo.frame.caret
                blinks: demo.pinned < 0
            }
            MiniList {
                visible: !!demo.frame.list
                theme: demo.theme
                items: demo.frame.list || []
            }
        }
    }
    RowLayout {
        Layout.fillWidth: true
        spacing: 8
        IconButton {
            visible: demo.pinned < 0
            theme: demo.theme
            iconName: demo.playing ? "pause" : "play"
            iconSize: 12
            implicitWidth: 22
            implicitHeight: 22
            Accessible.name: demo.playing ? "暫停示範" : "播放示範"
            onClicked: demo.playing = !demo.playing
        }
        Row {
            spacing: 3
            opacity: (demo.frame.keys || []).length > 0 ? 1 : 0
            Repeater {
                model: (demo.frame.keys || []).length > 0 ? demo.frame.keys : [" "]
                delegate: KeyCap {
                    required property string modelData
                    theme: demo.theme
                    text: modelData
                    small: true
                    active: true
                }
            }
        }
        Text {
            Layout.fillWidth: true
            text: demo.frame.caption || ""
            color: demo.theme.secondary
            font.pixelSize: demo.theme.detailSize
            elide: Text.ElideRight
        }
    }
}
