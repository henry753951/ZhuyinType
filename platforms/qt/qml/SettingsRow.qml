import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The one row of every settings card, with the macOS version's rules (RowMetrics in SettingsRows.swift), so rows
// cannot drift apart:
// - leading: the title and an optional subtitle that wraps; a sub-row (`indent`) shifts only this block, by 18;
// - trailing, in this order: a value, the control (this row's children), then the accessory column (ⓘ or ›), 16 wide
//   at the right edge, so ⓘ, › and the end of a switch line up on every row of a page;
// - the trailing block is centred on the whole leading block; 10 above and below, at least 24 tall;
// - `separated` draws the hairline above the row (every row of a card but the first);
// - hover never changes the layout. `clickable` rows tint on hover, 4 inside the card's edge.
Item {
    id: row
    required property Theme theme
    property string title: ""
    property string subtitle: ""
    property string value: ""
    property bool indent: false
    property bool dimmed: false
    property bool separated: false
    /// "none", "chevron" or "info".
    property string accessory: "none"
    property bool clickable: false
    /// The row's id for links: a link to it scrolls here and plays the arrival tint once (`arrive()`).
    property string anchor: ""
    property color subtitleColor: dimmed ? theme.tertiary : theme.secondary
    property color valueColor: theme.secondary
    /// Puts the trailing block under the leading one (narrow windows, long key lists).
    property bool stacked: false
    default property alias control: controlSlot.data
    readonly property bool hovered: hover.hovered
    // For the render's geometry check (PreviewCapture): every row's two blocks.
    readonly property bool isSettingsRow: true
    readonly property Item leadingItem: leading
    readonly property Item trailingItem: trailing
    signal activated
    signal infoClicked
    /// A link under the subtitle to where something is set (macOS PlaceLink), e.g. 「在「選字」設定」.
    property string linkTitle: ""
    signal linkActivated
    /// The user changed this from its default (macOS `marked`): a small accent dot in the card's inset, before the title.
    property bool marked: false
    Layout.fillWidth: true
    implicitWidth: content.implicitWidth
    implicitHeight: content.implicitHeight + theme.rowPadding * 2
    activeFocusOnTab: clickable
    Accessible.role: clickable ? Accessible.Button : Accessible.Grouping
    Accessible.name: title + (subtitle ? "，" + subtitle : "") + (value ? "，" + value : "")
    Keys.onReturnPressed: if (clickable) row.activated()
    Keys.onSpacePressed: if (clickable) row.activated()

    function arrive() {
        if (row.theme.motionEnabled)
            arrival.restart();
    }
    // The arrival cue, as on macOS: the row's cell tints with the accent once and fades. Behind the row; no layout.
    Rectangle {
        id: tint
        x: -row.theme.cardInset + 4
        y: 4
        width: row.width + (row.theme.cardInset - 4) * 2
        height: row.height - 8
        radius: row.theme.cardRadius - 4
        color: row.theme.accent
        opacity: 0
        SequentialAnimation {
            id: arrival
            PauseAnimation {
                duration: 180
            }
            NumberAnimation {
                target: tint
                property: "opacity"
                to: 0.17
                duration: 220
                easing.type: Easing.OutCubic
            }
            PauseAnimation {
                duration: 500
            }
            NumberAnimation {
                target: tint
                property: "opacity"
                to: 0
                duration: 900
                easing.type: Easing.InOutQuad
            }
        }
    }
    Rectangle {
        visible: row.separated
        width: parent.width
        height: 1
        color: row.theme.line
    }
    // The hover and focus tint reaches 12 past the text on each side: 4 inside the card's 16 inset.
    Rectangle {
        visible: row.clickable
        x: -row.theme.cardInset + 4
        y: row.separated ? 3 : 4
        width: row.width + (row.theme.cardInset - 4) * 2
        height: row.height - (row.separated ? 6 : 8) + 2
        radius: row.theme.cardRadius - 4
        color: tap.pressed ? row.theme.controlPressed : row.theme.hover
        opacity: row.hovered || tap.pressed ? 1 : 0
        border.width: row.activeFocus ? 2 : 0
        border.color: row.theme.focusRing
        Behavior on opacity {
            NumberAnimation {
                duration: row.theme.motionEnabled ? 110 : 0
            }
        }
    }
    HoverHandler {
        id: hover
        // Every row tracks the pointer (a key field shows its controls while the pointer is anywhere on the row);
        // only clickable rows tint and show the hand.
        cursorShape: row.clickable ? Qt.PointingHandCursor : Qt.ArrowCursor
    }
    TapHandler {
        id: tap
        enabled: row.clickable
        onTapped: row.activated()
    }
    GridLayout {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        columns: row.stacked ? 1 : 2
        columnSpacing: row.theme.rowGap
        rowSpacing: 8
        ColumnLayout {
            id: leading
            Layout.fillWidth: true
            Layout.leftMargin: row.indent ? row.theme.rowIndent : 0
            Layout.minimumHeight: row.theme.rowMinHeight
            Layout.alignment: Qt.AlignVCenter
            spacing: 3
            Text {
                id: titleText
                Layout.fillWidth: true
                text: row.title
                color: row.indent || row.dimmed ? row.theme.secondary : row.theme.text
                font.pixelSize: row.theme.bodySize
                wrapMode: Text.Wrap
                Rectangle {
                    visible: row.marked
                    x: -10
                    y: titleText.font.pixelSize * 0.62 - 3
                    width: 6
                    height: 6
                    radius: 3
                    color: row.theme.accent
                    Accessible.name: "已自訂"
                }
            }
            Text {
                Layout.fillWidth: true
                visible: text.length > 0
                text: row.subtitle
                color: row.subtitleColor
                font.pixelSize: row.theme.detailSize
                wrapMode: Text.Wrap
            }
            AbstractButton {
                visible: row.linkTitle.length > 0
                Layout.topMargin: 1
                implicitWidth: linkRow.implicitWidth
                implicitHeight: linkRow.implicitHeight
                Accessible.name: row.linkTitle
                contentItem: Row {
                    id: linkRow
                    spacing: 2
                    Text {
                        text: row.linkTitle
                        color: row.theme.accent
                        font.pixelSize: row.theme.detailSize
                        font.underline: parent.parent.hovered
                    }
                    Icon {
                        anchors.verticalCenter: parent.verticalCenter
                        name: "chevronRight"
                        size: 11
                        weight: 2
                        color: row.theme.accent
                    }
                }
                background: Item {}
                onClicked: row.linkActivated()
            }
        }
        RowLayout {
            id: trailing
            Layout.alignment: row.stacked ? Qt.AlignRight : Qt.AlignVCenter
            spacing: row.theme.trailingSpacing
            Text {
                visible: text.length > 0
                text: row.value
                color: row.valueColor
                font.pixelSize: row.theme.bodySize
                font.features: ({ "tnum": 1 })
                elide: Text.ElideRight
                horizontalAlignment: Text.AlignRight
                Layout.maximumWidth: Math.max(60, row.width * 0.4)
            }
            RowLayout {
                id: controlSlot
                visible: children.length > 0
                spacing: row.theme.trailingSpacing
            }
            Item {
                visible: row.accessory !== "none"
                implicitWidth: 16
                implicitHeight: 20
                Icon {
                    visible: row.accessory === "chevron"
                    anchors.centerIn: parent
                    name: "chevronRight"
                    size: 14
                    weight: 2
                    color: row.theme.tertiary
                }
                IconButton {
                    visible: row.accessory === "info"
                    anchors.centerIn: parent
                    theme: row.theme
                    iconName: "info"
                    iconSize: 16
                    implicitWidth: 24
                    implicitHeight: 24
                    Accessible.name: "說明：" + row.title
                    onClicked: row.infoClicked()
                }
            }
        }
    }
}
