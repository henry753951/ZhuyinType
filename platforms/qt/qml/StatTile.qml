import QtQuick
import QtQuick.Layouts

// One headline number in a grid of stats: a small caption, the value in tabular figures, an optional detail.
// Put tiles in a GridLayout; show an EmptyState instead when there is no data at all.
Rectangle {
    id: tile
    required property Theme theme
    property string label: ""
    property string value: "—"
    property string detail: ""
    property alias valueItem: valueText
    Layout.fillWidth: true
    Layout.minimumWidth: 0
    Layout.preferredWidth: 0
    implicitHeight: content.implicitHeight + 28
    radius: theme.cardRadius
    color: theme.card
    border.color: theme.line
    Accessible.role: Accessible.StaticText
    Accessible.name: label + "：" + value
    ColumnLayout {
        id: content
        anchors {
            left: parent.left
            right: parent.right
            top: parent.top
            margins: 14
            leftMargin: 16
            rightMargin: 16
        }
        spacing: 4
        Text {
            Layout.fillWidth: true
            text: tile.label
            color: tile.theme.secondary
            font.pixelSize: tile.theme.detailSize
            elide: Text.ElideRight
        }
        Text {
            id: valueText
            Layout.fillWidth: true
            text: tile.value
            color: tile.value === "—" ? tile.theme.tertiary : tile.theme.text
            font.pixelSize: 22
            font.weight: Font.DemiBold
            font.features: ({ "tnum": 1 })
            elide: Text.ElideRight
        }
        Text {
            visible: text.length > 0
            Layout.fillWidth: true
            text: tile.detail
            color: tile.theme.tertiary
            font.pixelSize: tile.theme.captionSize
            wrapMode: Text.Wrap
        }
    }
}
