import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The card an ⓘ opens (InfoButton's popover on macOS): a title and what the option does, next to the ⓘ, closed by
// clicking elsewhere or Esc. It changes nothing. `show(item)` opens it under `item`'s trailing edge; it stays inside
// the window and scrolls when it is taller.
Popup {
    id: popover
    required property Theme theme
    property string title: ""
    default property alias content: body.data
    width: 380
    height: Math.min(implicitHeight, parent && parent.Window.window ? parent.Window.window.height - 24 : implicitHeight)
    margins: 12
    padding: 18
    modal: false
    focus: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    function show(item) {
        parent = item;
        x = item.width - width;
        y = item.height + 4;
        open();
    }
    enter: Transition {
        NumberAnimation {
            property: "opacity"
            from: 0
            to: 1
            duration: popover.theme.motionEnabled ? 120 : 0
        }
    }
    exit: Transition {
        NumberAnimation {
            property: "opacity"
            to: 0
            duration: popover.theme.motionEnabled ? 90 : 0
        }
    }
    background: Rectangle {
        radius: popover.theme.radius
        color: popover.theme.surface
        border.color: popover.theme.line
    }
    contentItem: ScrollView {
        id: scroller
        clip: true
        contentWidth: availableWidth
        implicitHeight: column.implicitHeight
        ColumnLayout {
            id: column
            width: scroller.availableWidth
            spacing: 14
            Text {
                Layout.fillWidth: true
                text: popover.title
                color: popover.theme.text
                font.pixelSize: popover.theme.headingSize
                font.weight: Font.DemiBold
                wrapMode: Text.Wrap
                Accessible.role: Accessible.Heading
            }
            ColumnLayout {
                id: body
                Layout.fillWidth: true
                spacing: 14
            }
        }
    }
}
