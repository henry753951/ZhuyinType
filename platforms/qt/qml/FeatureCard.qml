import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// A clickable card that introduces a feature and opens its page: line icon in the accent, title, one sentence and
// a chevron (or, with `link`, the destination's name and a chevron at the bottom, as macOS's PlaceLink). Lay several out in a GridLayout (1 column when narrow, 2 when wide); handle `clicked` to navigate.
Button {
    id: card
    required property Theme theme
    property string title: ""
    property string detail: ""
    property string iconName: ""
    /// The page it opens, named at the bottom of the card.
    property string link: ""
    Layout.fillWidth: true
    Layout.fillHeight: true
    Layout.preferredWidth: 0
    Layout.minimumWidth: 0
    padding: 14
    leftPadding: 16
    rightPadding: 14
    Accessible.name: title
    Accessible.description: detail
    background: Rectangle {
        radius: card.theme.cardRadius
        color: card.down ? card.theme.hover : card.theme.card
        border.width: card.visualFocus ? 2 : 1
        border.color: card.visualFocus ? card.theme.focusRing : card.hovered ? card.theme.fieldBorder : card.theme.line
        Behavior on border.color {
            ColorAnimation {
                duration: card.theme.motionEnabled ? 120 : 0
            }
        }
    }
    contentItem: ColumnLayout {
        spacing: 6
        RowLayout {
            Layout.fillWidth: true
            spacing: 10
            Icon {
                visible: card.iconName.length > 0
                name: card.iconName
                color: card.theme.accent
                size: 18
            }
            Text {
                Layout.fillWidth: true
                text: card.title
                color: card.theme.text
                font.pixelSize: card.theme.bodySize
                font.weight: Font.DemiBold
                wrapMode: Text.Wrap
            }
            Icon {
                visible: card.link.length === 0
                name: "chevronRight"
                color: card.hovered ? card.theme.secondary : card.theme.tertiary
                size: 14
            }
        }
        Text {
            visible: text.length > 0
            Layout.fillWidth: true
            Layout.leftMargin: card.iconName.length > 0 ? 28 : 0
            text: card.detail
            color: card.theme.secondary
            font.pixelSize: card.theme.detailSize
            wrapMode: Text.Wrap
        }
        Item {
            Layout.fillHeight: true
        }
        Row {
            visible: card.link.length > 0
            Layout.leftMargin: card.iconName.length > 0 ? 28 : 0
            spacing: 2
            Text {
                text: card.link
                color: card.hovered ? card.theme.accentHover : card.theme.accent
                font.pixelSize: card.theme.detailSize
            }
            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: "chevronRight"
                color: card.hovered ? card.theme.accentHover : card.theme.accent
                size: 12
            }
        }
    }
}
