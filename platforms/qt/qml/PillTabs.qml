pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

// A wrapping row of rounded tabs for switching what one area shows (demo scenes, filters). Use the segmented control
// in SettingRow for a setting's value instead. `model` holds { id, title }; each tab is named `objectPrefix + id`.
Flow {
    id: tabs
    required property Theme theme
    property var model: []
    property string current: ""
    property string objectPrefix: "tab_"
    signal selected(string id)
    spacing: 6
    Repeater {
        model: tabs.model
        delegate: Button {
            id: tab
            required property var modelData
            objectName: tabs.objectPrefix + modelData.id
            text: modelData.title
            checked: tabs.current === modelData.id
            leftPadding: 12
            rightPadding: 12
            topPadding: 5
            bottomPadding: 5
            Accessible.role: Accessible.PageTab
            Accessible.name: text
            Accessible.selected: checked
            contentItem: Text {
                text: tab.text
                font.pixelSize: tabs.theme.detailSize
                font.weight: tab.checked ? Font.DemiBold : Font.Normal
                color: tab.checked ? tabs.theme.accent : tabs.theme.secondary
            }
            background: Rectangle {
                radius: height / 2
                color: tab.checked ? tabs.theme.accentSoft : tab.hovered ? tabs.theme.hover : "transparent"
                border.width: tab.visualFocus ? 2 : tab.checked ? 0 : 1
                border.color: tab.visualFocus ? tabs.theme.focusRing : tabs.theme.line
                Behavior on color {
                    ColorAnimation {
                        duration: tabs.theme.motionEnabled ? 120 : 0
                    }
                }
            }
            onClicked: tabs.selected(modelData.id)
        }
    }
}
