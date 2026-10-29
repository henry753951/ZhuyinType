import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// One row of a list page (記住的字詞, 自訂詞, App), as on macOS (ListItemRow): the item, its reading or identifier in
// secondary text on the same line, and quiet details on the right — a UsageMeter (`level` > 0) or `meta`. 編輯 and 刪除
// fade in over the details while the row is hovered, so hovering never moves anything. Rows sit edge to edge on one
// card; `first` leaves out the hairline above. Click selects (`clicked(modifiers)`); right-click asks for the menu.
Item {
    id: row
    required property Theme theme
    property string title: ""
    property string detail: ""
    property string meta: ""
    property int level: 0
    property bool marked: false
    property bool selected: false
    property bool first: false
    property bool canEdit: false
    readonly property bool revealed: hover.hovered || row.activeFocus
    signal clicked(int modifiers)
    signal editClicked
    signal removeClicked
    signal menuRequested
    implicitHeight: Math.max(row.theme.rowMinHeight, content.implicitHeight) + (row.theme.rowPadding - 2) * 2
    activeFocusOnTab: true
    Accessible.role: Accessible.ListItem
    Accessible.name: title + (detail ? "　" + detail : "") + (meta ? "　" + meta : "")
    Accessible.selected: selected
    Keys.onSpacePressed: row.clicked(Qt.NoModifier)
    Rectangle {
        visible: !row.first
        x: row.theme.cardInset
        width: row.width - row.theme.cardInset * 2
        height: 1
        color: row.theme.line
    }
    Rectangle {
        x: 4
        y: 2
        width: row.width - 8
        height: row.height - 4
        radius: row.theme.cardRadius - 4
        color: row.selected ? row.theme.accentSoft : "transparent"
        border.width: row.activeFocus ? 2 : 0
        border.color: row.theme.focusRing
    }
    HoverHandler {
        id: hover
    }
    TapHandler {
        acceptedButtons: Qt.LeftButton
        onTapped: (point, button) => row.clicked(point.modifiers)
    }
    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: row.menuRequested()
    }
    RowLayout {
        id: content
        anchors {
            left: parent.left
            right: parent.right
            leftMargin: row.theme.cardInset
            rightMargin: row.theme.cardInset
            verticalCenter: parent.verticalCenter
        }
        spacing: 10
        Text {
            text: row.title
            textFormat: Text.PlainText
            color: row.marked ? row.theme.secondary : row.theme.text
            font.pixelSize: row.theme.bodySize
            elide: Text.ElideRight
            Layout.maximumWidth: content.width * 0.5
        }
        Text {
            Layout.fillWidth: true
            text: row.detail
            textFormat: Text.PlainText
            color: row.theme.secondary
            font.pixelSize: row.theme.detailSize
            elide: Text.ElideRight
        }
        // The details and the hover actions share one place.
        Item {
            Layout.preferredWidth: Math.max(details.implicitWidth, actions.implicitWidth)
            Layout.preferredHeight: Math.max(details.implicitHeight, actions.implicitHeight)
            Item {
                id: details
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                implicitWidth: row.level > 0 ? meter.width : metaText.implicitWidth
                implicitHeight: 20
                opacity: row.revealed ? 0 : 1
                Behavior on opacity {
                    NumberAnimation {
                        duration: row.theme.motionEnabled ? 120 : 0
                    }
                }
                UsageMeter {
                    id: meter
                    visible: row.level > 0
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    theme: row.theme
                    level: row.level
                }
                Text {
                    id: metaText
                    visible: row.level <= 0
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    text: row.meta
                    color: row.theme.tertiary
                    font.pixelSize: row.theme.detailSize
                    font.features: ({ "tnum": 1 })
                }
            }
            RowLayout {
                id: actions
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: 4
                opacity: row.revealed ? 1 : 0
                enabled: row.revealed
                Behavior on opacity {
                    NumberAnimation {
                        duration: row.theme.motionEnabled ? 120 : 0
                    }
                }
                AppButton {
                    visible: row.canEdit
                    theme: row.theme
                    kind: "plain"
                    compact: true
                    text: "編輯"
                    focusPolicy: Qt.NoFocus
                    onClicked: row.editClicked()
                }
                AppButton {
                    theme: row.theme
                    kind: "plain"
                    compact: true
                    text: "刪除"
                    focusPolicy: Qt.NoFocus
                    onClicked: row.removeClicked()
                }
            }
        }
    }
}
