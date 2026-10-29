import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// A small segmented switch between two or three views of the same content (a period, a list mode). It is a TabBar,
// so currentIndex / setCurrentIndex work; for a stored setting use SettingRow's segments instead.
TabBar {
    id: control
    required property Theme theme
    property var model: []
    implicitHeight: 30
    implicitWidth: Math.max(72 * model.length, contentChildren.length ? implicitContentWidth + leftPadding + rightPadding : 0)
    padding: 3
    spacing: 2
    background: Rectangle {
        radius: control.theme.controlRadius
        color: control.theme.segment
    }
    Repeater {
        model: control.model
        delegate: TabButton {
            id: segment
            required property string modelData
            implicitHeight: 24
            Accessible.name: (control.Accessible.name ? control.Accessible.name + "：" : "") + modelData
            contentItem: Text {
                text: segment.modelData
                color: segment.checked ? control.theme.text : control.theme.secondary
                font.pixelSize: control.theme.detailSize
                font.weight: segment.checked ? Font.DemiBold : Font.Normal
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                elide: Text.ElideRight
            }
            background: Rectangle {
                radius: control.theme.controlRadius - 2
                color: segment.checked ? control.theme.segmentSelected : segment.hovered ? Qt.rgba(control.theme.segmentSelected.r, control.theme.segmentSelected.g, control.theme.segmentSelected.b, 0.4) : "transparent"
                border.width: segment.visualFocus ? 2 : segment.checked && !control.theme.dark ? 1 : 0
                border.color: segment.visualFocus ? control.theme.focusRing : control.theme.line
                Behavior on color {
                    ColorAnimation {
                        duration: control.theme.motionEnabled ? 120 : 0
                    }
                }
            }
        }
    }
}
