import QtQuick
import QtQuick.Layouts

// A row that folds its details open below it (DisclosureGroup on macOS): a chevron on the left that turns, a title,
// a secondary line, and the details (children) indented under the title while `expanded`. Rows of a card set
// `separated` on all but the first.
ColumnLayout {
    id: disclosure
    required property Theme theme
    property string title: ""
    property string subtitle: ""
    property bool expanded: false
    property bool separated: false
    default property alias details: body.data
    Layout.fillWidth: true
    spacing: 0
    Rectangle {
        visible: disclosure.separated
        Layout.fillWidth: true
        implicitHeight: 1
        color: disclosure.theme.line
    }
    Item {
        id: header
        Layout.fillWidth: true
        implicitHeight: label.implicitHeight + disclosure.theme.rowPadding * 2
        activeFocusOnTab: true
        Accessible.role: Accessible.Button
        Accessible.name: disclosure.title + "，" + disclosure.subtitle + (disclosure.expanded ? "，已展開" : "，已收合")
        Keys.onReturnPressed: disclosure.expanded = !disclosure.expanded
        Keys.onSpacePressed: disclosure.expanded = !disclosure.expanded
        Rectangle {
            x: -disclosure.theme.cardInset + 4
            y: 3
            width: header.width + (disclosure.theme.cardInset - 4) * 2
            height: header.height - 6
            radius: disclosure.theme.cardRadius - 4
            color: tap.pressed ? disclosure.theme.controlPressed : disclosure.theme.hover
            opacity: hover.hovered || tap.pressed ? 1 : 0
            border.width: header.activeFocus ? 2 : 0
            border.color: disclosure.theme.focusRing
        }
        HoverHandler {
            id: hover
            cursorShape: Qt.PointingHandCursor
        }
        TapHandler {
            id: tap
            onTapped: disclosure.expanded = !disclosure.expanded
        }
        Icon {
            id: chevron
            x: 0
            y: disclosure.theme.rowPadding + 2
            name: "chevronRight"
            size: 12
            weight: 2
            color: disclosure.theme.tertiary
            rotation: disclosure.expanded ? 90 : 0
            Behavior on rotation {
                NumberAnimation {
                    duration: disclosure.theme.motionEnabled ? 160 : 0
                    easing.type: Easing.OutCubic
                }
            }
        }
        ColumnLayout {
            id: label
            anchors {
                left: parent.left
                right: parent.right
                leftMargin: 20
                verticalCenter: parent.verticalCenter
            }
            spacing: 3
            Text {
                Layout.fillWidth: true
                text: disclosure.title
                textFormat: Text.PlainText
                color: disclosure.theme.text
                font.pixelSize: disclosure.theme.bodySize
                wrapMode: Text.Wrap
            }
            Text {
                Layout.fillWidth: true
                visible: text.length > 0
                text: disclosure.subtitle
                textFormat: Text.PlainText
                color: disclosure.theme.secondary
                font.pixelSize: disclosure.theme.detailSize
                wrapMode: Text.Wrap
            }
        }
    }
    ColumnLayout {
        id: body
        visible: disclosure.expanded
        Layout.fillWidth: true
        Layout.leftMargin: 20
        Layout.bottomMargin: disclosure.theme.rowPadding + 2
        spacing: 8
    }
}
