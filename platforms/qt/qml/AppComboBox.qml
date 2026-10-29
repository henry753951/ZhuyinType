pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

// A drop-down choice: a field with a quiet up-down chevron and a rounded, shadowed list.
ComboBox {
    id: combo
    required property Theme theme
    /// The size used inside settings rows.
    property bool small: false
    implicitHeight: small ? theme.rowControlHeight : theme.controlHeight
    // Wide enough for the longest option, so the picker never resizes and never cuts its text.
    readonly property real widestOption: {
        // A JS array set as the model comes back as a sequence, not an Array.
        const items = Array.isArray(model) ? model : model && typeof model.length === "number" ? Array.prototype.slice.call(model) : [];
        return items.reduce((widest, item) => Math.max(widest, optionMetrics.advanceWidth(typeof item === "object" && textRole ? item[textRole] : String(item))), 0);
    }
    implicitWidth: Math.ceil(widestOption) + leftPadding + rightPadding + 2
    FontMetrics {
        id: optionMetrics
        font: combo.font
    }
    leftPadding: small ? 8 : 10
    rightPadding: small ? 24 : 30
    font.pixelSize: small ? theme.detailSize : theme.bodySize
    opacity: enabled ? 1 : 0.5
    contentItem: Text {
        text: combo.displayText
        color: combo.theme.text
        font: combo.font
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
    indicator: Icon {
        x: combo.width - width - (combo.small ? 7 : 9)
        y: (combo.height - height) / 2
        name: "chevronUpDown"
        color: combo.theme.secondary
        size: combo.small ? 13 : 15
    }
    background: Rectangle {
        radius: combo.theme.controlRadius
        color: combo.down ? combo.theme.controlPressed : combo.hovered ? combo.theme.controlHover : combo.theme.control
        border.width: combo.visualFocus ? 2 : 1
        border.color: combo.visualFocus ? combo.theme.focusRing : combo.theme.controlBorder
    }
    delegate: ItemDelegate {
        id: option
        required property var modelData
        required property int index
        width: ListView.view ? ListView.view.width : implicitWidth
        implicitHeight: 30
        leftPadding: 8
        rightPadding: 10
        highlighted: combo.highlightedIndex === index
        contentItem: Row {
            spacing: 6
            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: "check"
                size: 14
                color: option.highlighted ? combo.theme.accentText : combo.theme.accent
                opacity: combo.currentIndex === option.index ? 1 : 0
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: typeof option.modelData === "object" && combo.textRole ? option.modelData[combo.textRole] : option.modelData
                color: option.highlighted ? combo.theme.accentText : combo.theme.text
                font: combo.font
            }
        }
        background: Rectangle {
            radius: 6
            color: option.highlighted ? combo.theme.accent : "transparent"
        }
    }
    popup: Popup {
        y: combo.height + 4
        width: Math.max(combo.width, list.contentItem.childrenRect.width + 12)
        implicitHeight: Math.min(list.contentHeight + 12, 320)
        padding: 6
        contentItem: ListView {
            id: list
            clip: true
            implicitHeight: contentHeight
            model: combo.popup.visible ? combo.delegateModel : null
            currentIndex: combo.highlightedIndex
            boundsBehavior: Flickable.StopAtBounds
            ScrollIndicator.vertical: ScrollIndicator {}
        }
        background: Rectangle {
            radius: 10
            color: combo.theme.surface
            border.color: combo.theme.line
            Rectangle {
                z: -1
                anchors.fill: parent
                anchors.topMargin: 3
                anchors.margins: -1
                radius: 11
                color: combo.theme.shadow
            }
        }
        enter: Transition {
            NumberAnimation {
                property: "opacity"
                from: 0
                to: 1
                duration: combo.theme.motionEnabled ? 110 : 0
            }
        }
    }
}
