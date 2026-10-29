pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// One choice of the setup wizard (ChoiceCard and StyleGroupView's options on macOS): a card with a title and a few
// lines — keys drawn as keycaps, then words — and a check at its trailing edge (`lines` given), or a single-line
// option with a radio (`multi`: a checkbox) in front. The chosen one is outlined in the accent.
AbstractButton {
    id: tile
    required property Theme theme
    property string title: ""
    /// [{ keys: ["Shift"], text: "單按切換中英" }]; empty: a single-line option.
    property var lines: []
    property string iconName: ""
    property bool selected: false
    /// Several may be chosen at once: a checkbox instead of a radio.
    property bool multi: false
    readonly property bool card: lines.length > 0
    checkable: false
    focusPolicy: Qt.StrongFocus
    implicitWidth: content.implicitWidth + leftPadding + rightPadding
    implicitHeight: content.implicitHeight + topPadding + bottomPadding
    padding: card ? 12 : 8
    leftPadding: 12
    rightPadding: 12
    Accessible.role: multi ? Accessible.CheckBox : Accessible.RadioButton
    Accessible.checkable: true
    Accessible.checked: selected
    Accessible.name: title
    background: Rectangle {
        radius: 9
        color: tile.selected ? tile.theme.accentSoft : tile.down ? tile.theme.controlPressed : tile.hovered ? tile.theme.hover : tile.theme.card
        border.width: tile.visualFocus ? 2 : tile.selected ? 1.5 : 1
        border.color: tile.visualFocus ? tile.theme.focusRing : tile.selected ? tile.theme.accent : tile.theme.line
        Behavior on color {
            ColorAnimation {
                duration: tile.theme.motionEnabled ? 140 : 0
            }
        }
    }
    component Indicator: Rectangle {
        implicitWidth: 16
        implicitHeight: 16
        radius: tile.multi ? 4 : 8
        color: tile.selected ? tile.theme.accent : "transparent"
        border.width: tile.selected ? 0 : 1.5
        border.color: tile.theme.tertiary
        Icon {
            visible: tile.selected && (tile.multi || tile.card)
            anchors.centerIn: parent
            name: "check"
            size: 11
            weight: 2.4
            color: tile.theme.accentText
        }
        Rectangle {
            visible: tile.selected && !tile.multi && !tile.card
            anchors.centerIn: parent
            width: 6
            height: 6
            radius: 3
            color: tile.theme.accentText
        }
    }
    contentItem: ColumnLayout {
        id: content
        spacing: 8
        RowLayout {
            Layout.fillWidth: true
            spacing: 10
            Indicator {
                visible: !tile.card
            }
            Icon {
                visible: tile.iconName.length > 0
                name: tile.iconName
                size: 16
                color: tile.theme.secondary
            }
            Text {
                Layout.fillWidth: true
                text: tile.title
                color: tile.theme.text
                font.pixelSize: tile.theme.bodySize
                font.weight: tile.selected || tile.card ? Font.DemiBold : Font.Normal
                elide: Text.ElideRight
            }
            Indicator {
                visible: tile.card
            }
        }
        Repeater {
            model: tile.lines
            delegate: RowLayout {
                id: lineRow
                required property var modelData
                Layout.fillWidth: true
                spacing: 6
                Repeater {
                    model: lineRow.modelData.keys || []
                    delegate: KeyCap {
                        required property string modelData
                        theme: tile.theme
                        text: modelData
                    }
                }
                Text {
                    Layout.fillWidth: true
                    text: lineRow.modelData.text
                    color: tile.theme.secondary
                    font.pixelSize: tile.theme.detailSize
                    wrapMode: Text.Wrap
                }
            }
        }
    }
}
