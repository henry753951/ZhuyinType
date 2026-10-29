pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts

// The controls under an animated demonstration: play/pause and one dot per step. Use it for every demo, so they all
// look and behave the same. The owner keeps `current` and `playing`; the controls only ask through the signals.
RowLayout {
    id: controls
    required property Theme theme
    property int count: 0
    property int current: 0
    property bool playing: true
    property bool playEnabled: true
    property var stepNames: []
    property string subject: "示意動畫"
    signal playToggled
    signal stepChosen(int index)
    spacing: 2
    IconButton {
        theme: controls.theme
        iconName: controls.playing && controls.playEnabled ? "pause" : "play"
        iconSize: 15
        accented: true
        enabled: controls.playEnabled
        Accessible.name: (controls.playing && controls.playEnabled ? "暫停" : "播放") + controls.subject
        onClicked: controls.playToggled()
    }
    Repeater {
        model: controls.count
        delegate: IconButton {
            id: step
            required property int index
            theme: controls.theme
            implicitWidth: 18
            Accessible.name: "第 " + (index + 1) + " 步" + (controls.stepNames[index] ? "：" + controls.stepNames[index] : "")
            contentItem: Item {
                Rectangle {
                    anchors.centerIn: parent
                    width: step.index === controls.current ? 16 : 7
                    height: 7
                    radius: 3.5
                    color: step.index === controls.current ? controls.theme.accent : step.hovered ? controls.theme.secondary : controls.theme.track
                    Behavior on width {
                        NumberAnimation {
                            duration: controls.theme.motionEnabled ? 160 : 0
                            easing.type: Easing.OutCubic
                        }
                    }
                }
            }
            background: Item {}
            onClicked: controls.stepChosen(index)
        }
    }
}
