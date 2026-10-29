import QtQuick
import QtQuick.Layouts

// A section of a page, as on macOS: a heading, a card and an optional footer.
// - Rows (SettingsRow and the rows built on it) go straight into the card; set `separated` on all but the first.
// - Other content (a demo, a field, a list) sets `padded: true` to get the card inset on all four sides.
// - `bare: true` drops the card, for tiles that bring their own.
ColumnLayout {
    id: section
    required property Theme theme
    property string title: ""
    property string footer: ""
    property bool padded: false
    property bool bare: false
    default property alias content: body.data
    Layout.fillWidth: true
    spacing: 0
    Text {
        visible: section.title.length > 0
        text: section.title
        color: section.theme.secondary
        font {
            pixelSize: section.theme.sectionTitleSize
            weight: Font.DemiBold
        }
        Layout.fillWidth: true
        Layout.leftMargin: section.theme.cardInset
        Layout.bottomMargin: section.theme.headerGap
        wrapMode: Text.Wrap
    }
    Rectangle {
        Layout.fillWidth: true
        implicitHeight: body.implicitHeight + (section.padded ? section.theme.tilePadding * 2 : 0)
        color: section.bare ? "transparent" : section.theme.card
        radius: section.theme.cardRadius
        ColumnLayout {
            id: body
            anchors {
                left: parent.left
                right: parent.right
                top: parent.top
                leftMargin: section.bare ? 0 : section.theme.cardInset
                rightMargin: section.bare ? 0 : section.theme.cardInset
                topMargin: section.padded ? section.theme.tilePadding : 0
            }
            spacing: section.padded ? 12 : 0
        }
    }
    Text {
        visible: section.footer.length > 0
        text: section.footer
        color: section.theme.secondary
        font.pixelSize: section.theme.detailSize
        Layout.fillWidth: true
        Layout.leftMargin: section.theme.cardInset
        Layout.rightMargin: section.theme.cardInset
        Layout.topMargin: section.theme.headerGap
        wrapMode: Text.Wrap
    }
}
