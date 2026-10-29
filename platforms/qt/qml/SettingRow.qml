import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// One setting: a SettingsRow whose control follows the setting's kind (switch, picker, segments, stepper, slider).
// `info` puts the ⓘ in the accessory column; the page opens the card on `infoClicked`.
//
// The subtitle follows the value, as SettingDescriptor.detail does on macOS: `details` in settings.json maps a value
// to its line, and the lines that name keys are built here from the current keys (`shortcuts`), never spelled out, so
// they stay right after a rebind. A feature that another setting turns off is dimmed and says which.
SettingsRow {
    id: row
    required property var definition
    required property var settings
    /// The Shortcuts object, for the keys the subtitle names; without it the lines that name keys use the defaults.
    property var shortcuts: null
    property bool info: false
    readonly property var values: settings.values
    readonly property string availability: unavailableNote()
    title: definition.title
    subtitle: availability || detailText()
    dimmed: availability.length > 0
    anchor: definition.anchor || ""
    accessory: info ? "info" : "none"
    stacked: width < 440 && definition.kind !== "toggle"

    /// The command's first key as it reads in a sentence ("Shift ＋ ↑"), whether or not its feature is on.
    function assignedKey(command) {
        const bound = shortcuts && shortcuts.values ? shortcuts.values[command] : undefined;
        if (bound && bound.length > 0)
            return bound[0].label;
        const known = shortcuts && shortcuts.commands ? shortcuts.commands.find(item => item.id === command) : undefined;
        return known && known.defaults.length > 0 ? known.defaults[0].label : "";
    }
    /// The command's first key, or "" when its feature is off or it has no key (CurrentKeys.text on macOS).
    function activeKey(command) {
        const known = shortcuts && shortcuts.commands ? shortcuts.commands.find(item => item.id === command) : undefined;
        if (known && known.feature && values[known.feature] === false)
            return "";
        const bound = shortcuts && shortcuts.values ? shortcuts.values[command] : undefined;
        if (bound !== undefined)
            return bound.length > 0 ? bound[0].label : "";
        return assignedKey(command);
    }
    /// The keys that pick on a page: "1–9", or "A S D F G" for the home row.
    function selectionText() {
        const keys = String(values.selectionKeys || "123456789").slice(0, Number(values.candidatesPerPage || 9)).toUpperCase().split("");
        return /^[0-9]+$/.test(keys.join("")) ? keys[0] + "–" + keys[keys.length - 1] : keys.join(" ");
    }
    /// The key that shows the 中／英 badge (CurrentKeys.modeKey).
    function modeKey() {
        if (values.shiftSwitch !== false)
            return values.shiftToggle === "left" ? "左 Shift" : values.shiftToggle === "right" ? "右 Shift" : "Shift";
        return values["feature.capsLockEnglish"] !== false ? "Caps Lock" : "";
    }
    function unavailableNote() {
        const key = definition.key;
        if (key === "feature.zhuyinRowClick" && values.showsZhuyinHint === false)
            return "「顯示注音」已關閉，候選字視窗沒有注音列";
        if (key === "feature.correctionsList" && values.typoTolerance === "off" && !["zZh", "cCh", "sSh", "enEng", "anAng"].some(pair => values["fuzzy." + pair]))
            return "「按錯鍵修正」關閉且沒有開啟模糊音：這個鍵開啟一般清單";
        if (key === "showsListSwitches" && !activeKey("singleCharacterList") && !activeKey("correctionsList"))
            return "兩種清單都已關閉";
        return "";
    }
    function detailText() {
        const key = definition.key;
        const value = values[key];
        if (key.startsWith("feature.") && value === false) {
            if (key === "feature.zhuyinRowClick")
                return "關閉時，點注音列不做事";
            if (definition.commands && definition.commands.length > 0)
                return "關閉時，" + assignedKey(definition.commands[0]) + " 交給 App";
        }
        switch (key) {
        case "showModeToast":
            return modeKey() ? "按 " + modeKey() + " 切換時，在游標旁顯示「中」或「英」" : "切換時在游標旁顯示「中」或「英」";
        case "candidateLayout": {
            if (value !== "horizontal")
                return "↑ ↓ 移動反白，← → 改前一個或後一個字";
            const previous = activeKey("previousCharacter");
            return "← → 移動反白，↑ ↓ 翻頁" + (previous ? "，" + previous + " 改前一個字" : "");
        }
        case "candidatesPerPage":
            return "用 " + selectionText() + " 選字；Return 選取反白的字";
        case "showsAnnotations": {
            const corrections = activeKey("correctionsList");
            return corrections ? "修正過的字旁邊以灰字標出注音，例如 " + corrections + " 列出的字" : "修正過的字旁邊以灰字標出注音";
        }
        case "showsListSwitches": {
            const switches = [[activeKey("singleCharacterList"), "單字"], [activeKey("correctionsList"), "修正"]].filter(item => item[0]).map(item => item[0] + " " + item[1]);
            return switches.length === 0 ? "兩種清單都已關閉" : "清單下方的 " + switches.join("、") + "；關閉時只在使用中顯示";
        }
        case "feature.zhuyinRowClick": {
            const zhuyin = activeKey("typeZhuyin");
            return zhuyin ? "和按 " + zhuyin + " 相同；緊接著再按一次或按 Esc 恢復" : "打出注音本身";
        }
        case "panelAnimation":
            return theme.reduceMotion ? "淡入淡出；已開啟「減少動態效果」，不滑動" : definition.detail;
        }
        const lines = definition.details;
        if (lines) {
            const line = lines[String(value)];
            if (line !== undefined)
                return line;
        }
        return definition.detail || "";
    }
    Loader {
        objectName: "settingControl"
        // Nothing to show: the switches of two lists that are both off (macOS disables it too).
        enabled: !(row.definition.key === "showsListSwitches" && row.dimmed)
        Layout.alignment: Qt.AlignVCenter
        Layout.preferredWidth: row.definition.control === "slider" ? 210 : row.definition.control === "segments" ? Math.max(120, row.definition.options.length * 64) : -1
        sourceComponent: row.definition.control === "slider" ? slider : row.definition.control === "segments" ? segments : row.definition.kind === "toggle" ? toggle : row.definition.kind === "number" ? number : choice
    }
    Component {
        id: toggle
        Switch {
            id: toggleControl
            padding: 0
            implicitHeight: 20
            implicitWidth: 34
            checked: row.settings.values[row.definition.key] === true
            Accessible.name: row.definition.title
            onClicked: row.settings.setValue(row.definition.key, checked)
            indicator: Rectangle {
                width: 34
                height: 20
                radius: 10
                color: toggleControl.checked ? row.theme.accent : row.theme.track
                Behavior on color {
                    ColorAnimation {
                        duration: row.theme.motionEnabled ? 140 : 0
                    }
                }
                border {
                    color: row.theme.focusRing
                    width: toggleControl.visualFocus ? 2 : 0
                }
                Rectangle {
                    x: toggleControl.checked ? parent.width - width - 2 : 2
                    Behavior on x {
                        NumberAnimation {
                            duration: row.theme.motionEnabled ? 160 : 0
                            easing.type: Easing.OutCubic
                        }
                    }
                    y: 2
                    width: 16
                    height: 16
                    radius: 8
                    color: "white"
                    scale: toggleControl.pressed && !row.theme.reduceMotion ? 0.92 : 1
                    Behavior on scale {
                        NumberAnimation {
                            duration: row.theme.motionEnabled ? 100 : 0
                        }
                    }
                }
            }
        }
    }
    Component {
        id: slider
        RowLayout {
            spacing: 10
            Text {
                text: row.settings.values[row.definition.key] + (row.definition.unit || "")
                color: row.theme.secondary
                font.pixelSize: row.theme.detailSize
                font.features: ({ "tnum": 1 })
                Layout.preferredWidth: 36
            }
            Text {
                text: "字"
                color: row.theme.secondary
                font.pixelSize: row.theme.captionSize
            }
            Slider {
                id: fontSlider
                objectName: "fontSlider"
                Layout.fillWidth: true
                from: row.definition.min
                to: row.definition.max
                stepSize: 1
                snapMode: Slider.SnapAlways
                value: row.settings.values[row.definition.key]
                Accessible.name: row.definition.title
                onMoved: row.settings.setValue(row.definition.key, Math.round(value))
                background: Rectangle {
                    x: fontSlider.leftPadding
                    y: fontSlider.topPadding + (fontSlider.availableHeight - height) / 2
                    width: fontSlider.availableWidth
                    height: 4
                    radius: 2
                    color: row.theme.track
                    Rectangle {
                        width: parent.width * fontSlider.visualPosition
                        height: parent.height
                        radius: 2
                        color: row.theme.accent
                    }
                }
                handle: Rectangle {
                    x: fontSlider.leftPadding + fontSlider.visualPosition * (fontSlider.availableWidth - width)
                    y: fontSlider.topPadding + (fontSlider.availableHeight - height) / 2
                    width: 20
                    height: 20
                    radius: 10
                    color: "white"
                    border.width: fontSlider.visualFocus ? 3 : 1
                    border.color: fontSlider.visualFocus ? row.theme.focusRing : row.theme.fieldBorder
                }
            }
            Text {
                text: "字"
                color: row.theme.secondary
                font.pixelSize: row.theme.displaySize
            }
        }
    }
    Component {
        id: segments
        Rectangle {
            implicitHeight: row.theme.rowControlHeight
            radius: row.theme.controlRadius - 1
            color: row.theme.segment
            RowLayout {
                anchors {
                    fill: parent
                    margins: 2
                }
                spacing: 2
                Repeater {
                    model: row.definition.options
                    delegate: Button {
                        id: segment
                        required property var modelData
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        checked: row.settings.values[row.definition.key] === modelData.value
                        Accessible.name: row.definition.title + "：" + modelData.label
                        contentItem: Text {
                            text: segment.modelData.label
                            color: segment.checked ? row.theme.text : row.theme.secondary
                            font.pixelSize: row.theme.detailSize
                            font.weight: segment.checked ? Font.DemiBold : Font.Normal
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                        }
                        background: Rectangle {
                            radius: row.theme.controlRadius - 2
                            color: segment.checked ? row.theme.segmentSelected : segment.hovered ? Qt.rgba(row.theme.segmentSelected.r, row.theme.segmentSelected.g, row.theme.segmentSelected.b, 0.4) : "transparent"
                            border.width: segment.visualFocus ? 2 : segment.checked && !row.theme.dark ? 1 : 0
                            border.color: segment.visualFocus ? row.theme.focusRing : row.theme.line
                            Behavior on color {
                                ColorAnimation {
                                    duration: row.theme.motionEnabled ? 120 : 0
                                }
                            }
                        }
                        onClicked: row.settings.setValue(row.definition.key, modelData.value)
                    }
                }
            }
        }
    }
    Component {
        id: number
        RowLayout {
            spacing: 10
            Text {
                text: row.settings.values[row.definition.key] + (row.definition.unit || "")
                color: row.theme.text
                font.pixelSize: row.theme.bodySize
                font.features: ({ "tnum": 1 })
                horizontalAlignment: Text.AlignRight
                Layout.fillWidth: true
            }
            SpinBox {
                id: stepper
                implicitWidth: 56
                implicitHeight: row.theme.rowControlHeight
                padding: 0
                from: row.definition.min
                to: row.definition.max
                value: row.settings.values[row.definition.key]
                Accessible.name: row.definition.title
                onValueModified: row.settings.setValue(row.definition.key, value)
                contentItem: Rectangle {
                    x: 28
                    y: 5
                    width: 1
                    height: 14
                    color: row.theme.fieldBorder
                }
                background: Rectangle {
                    radius: row.theme.controlRadius
                    color: row.theme.control
                    border.width: stepper.visualFocus ? 2 : 0
                    border.color: row.theme.focusRing
                }
                down.indicator: Rectangle {
                    x: 0
                    width: 28
                    height: stepper.height
                    radius: row.theme.controlRadius
                    color: stepper.down.pressed ? row.theme.controlPressed : stepper.down.hovered ? row.theme.controlHover : "transparent"
                    opacity: stepper.value > stepper.from ? 1 : 0.4
                    Icon {
                        anchors.centerIn: parent
                        name: "minus"
                        size: 14
                        color: row.theme.text
                    }
                }
                up.indicator: Rectangle {
                    x: 28
                    width: 28
                    height: stepper.height
                    radius: row.theme.controlRadius
                    color: stepper.up.pressed ? row.theme.controlPressed : stepper.up.hovered ? row.theme.controlHover : "transparent"
                    opacity: stepper.value < stepper.to ? 1 : 0.4
                    Icon {
                        anchors.centerIn: parent
                        name: "plus"
                        size: 14
                        color: row.theme.text
                    }
                }
            }
        }
    }
    Component {
        id: choice
        AppComboBox {
            theme: row.theme
            small: true
            model: row.definition.options
            textRole: "label"
            valueRole: "value"
            currentIndex: row.definition.options.findIndex(option => option.value === row.settings.values[row.definition.key])
            Accessible.name: row.definition.title
            onActivated: row.settings.setValue(row.definition.key, currentValue)
        }
    }
}
