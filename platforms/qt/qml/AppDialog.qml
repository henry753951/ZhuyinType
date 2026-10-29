import QtQuick
import QtQuick.Controls.Basic

// A centred modal sheet with a rounded card, a dimmed backdrop and a title in the body's type scale. Footers built
// from DialogButtonBox get the same right-aligned layout through `buttons`.
Dialog {
    id: dialog
    required property Theme theme
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(420, parent ? parent.width - 48 : 420)
    modal: true
    padding: 22
    topPadding: 4
    bottomPadding: 18
    palette.window: theme.card
    palette.windowText: theme.text
    palette.text: theme.text
    palette.base: theme.field
    palette.button: theme.control
    palette.buttonText: theme.text
    palette.highlight: theme.accent
    font.pixelSize: theme.bodySize
    header: Text {
        visible: dialog.title.length > 0
        text: dialog.title
        color: dialog.theme.text
        font.pixelSize: dialog.theme.headingSize + 1
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        padding: 22
        bottomPadding: 10
    }
    background: Rectangle {
        radius: 14
        color: dialog.theme.card
        border.color: dialog.theme.line
        Rectangle {
            z: -1
            anchors.fill: parent
            anchors.margins: -1
            anchors.topMargin: 6
            radius: 15
            color: dialog.theme.shadow
        }
    }
    Overlay.modal: Rectangle {
        color: dialog.theme.dark ? Qt.rgba(0, 0, 0, 0.5) : Qt.rgba(0.07, 0.08, 0.12, 0.28)
    }
    enter: Transition {
        ParallelAnimation {
            NumberAnimation {
                property: "opacity"
                from: 0
                to: 1
                duration: dialog.theme.motionEnabled ? 140 : 0
                easing.type: Easing.OutCubic
            }
            NumberAnimation {
                property: "scale"
                from: dialog.theme.reduceMotion ? 1 : 0.97
                to: 1
                duration: dialog.theme.motionEnabled ? 140 : 0
                easing.type: Easing.OutCubic
            }
        }
    }
    exit: Transition {
        NumberAnimation {
            property: "opacity"
            to: 0
            duration: dialog.theme.motionEnabled ? 100 : 0
        }
    }
}
