pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The one field for a command's keys, wherever they are shown or set (macOS ShortcutField).  Each key is one chip with
// its modifiers inside (「⇧Tab」「Alt+↓」); alternatives are just more chips.  At rest nothing else shows.  With the
// pointer on the row (`rowHovered`, or on the field): a dashed ＋ chip to add a key, and 還原預設 beside the field when
// the keys differ from the defaults; on a chip, its ✕.  Click a chip to record a new combination (Esc cancels): the
// chip becomes an accent outline with the modifiers held so far and a caret.  An accepted key settles with a small
// bounce; a refused one shakes and a red line says why.  A key another command also uses is red, with a line naming
// it.  Keyboard: chips take focus, Backspace or Delete removes one, Return records; the context menu has 改按鍵…,
// 新增按鍵, 刪除 and 還原預設.  `maxVisible` > 0 shows that many chips, then 「+N」 (the 快捷鍵 overview).
Item {
    id: field
    required property var shortcuts
    required property Theme theme
    required property string command
    property string title: definition ? definition.label : command
    /// The command's feature is off: drawn dimmed, not editable.
    property bool disabled: false
    /// The pointer is on the row the field is in.
    property bool rowHovered: false
    property int maxVisible: 0
    signal navigateRequested(string pageId, string title, string row)

    readonly property var definition: shortcuts && shortcuts.commands ? shortcuts.commands.find(item => item.id === command) || null : null
    readonly property var bindings: shortcuts && shortcuts.values && definition ? (shortcuts.values[command] || []) : []
    readonly property var conflicts: shortcuts && shortcuts.values && definition && shortcuts.conflictsOf ? shortcuts.conflictsOf(command) : ({})
    readonly property bool customized: !!definition && !!shortcuts && shortcuts.values !== undefined && shortcuts.isCustomized(command)
    readonly property int maximum: shortcuts ? shortcuts.maximumBindings : 5
    property int recordingIndex: -2
    readonly property bool recording: recordingIndex !== -2
    property string problem: ""
    property int heldModifiers: 0
    property bool saved: false
    property int settledIndex: -1
    property bool expanded: false
    readonly property bool editing: (rowHovered || ownHover.hovered || focusInside) && !disabled && !recording && !!definition
    readonly property bool focusInside: activeFocus || chips.activeFocusInside
    readonly property bool motion: theme.motionEnabled && !theme.reduceMotion
    readonly property int visibleCount: maxVisible > 0 && !expanded && !recording ? Math.min(bindings.length, maxVisible) : bindings.length
    // The flow's items in reading order: ＋, the chips, +N, then the recorder when it adds a key.
    readonly property var items: {
        const result = [];
        if (bindings.length > 0 && bindings.length < maximum && !disabled)
            result.push({ kind: "add", index: -1 });
        if (bindings.length === 0 && !recording)
            result.push({ kind: "empty", index: -1 });
        for (let i = 0; i < visibleCount; ++i)
            result.push({ kind: recordingIndex === i ? "recorder" : "chip", index: i });
        if (visibleCount < bindings.length)
            result.push({ kind: "more", index: bindings.length - visibleCount });
        if (recording && recordingIndex >= bindings.length)
            result.push({ kind: "recorder", index: recordingIndex });
        return result;
    }
    implicitHeight: contents.implicitHeight
    // All chips on one line (a Flow's own implicit width depends on its width).
    implicitWidth: {
        let total = 0, count = 0;
        for (let i = 0; i < chips.children.length; ++i) {
            const child = chips.children[i];
            if (child.visible && child.implicitWidth > 0) {
                total += child.implicitWidth;
                ++count;
            }
        }
        return total + Math.max(0, count - 1) * chips.spacing;
    }
    opacity: disabled ? 0.45 : 1
    Accessible.role: Accessible.Grouping
    Accessible.name: title + "：" + (bindings.length ? bindings.map(binding => binding.spoken || binding.label).join(" 或 ") : "沒有按鍵")

    onEnabledChanged: if (!enabled) cancel()
    onVisibleChanged: if (!visible) cancel()
    onDisabledChanged: if (disabled) cancel()
    onCommandChanged: cancel()
    onEditingChanged: if (!editing && !recording) expanded = false

    function start(index) {
        if (!enabled || disabled || !definition || !shortcuts)
            return;
        problem = "";
        saved = false;
        heldModifiers = 0;
        recordingIndex = index < 0 ? bindings.length : index;
        field.forceActiveFocus();
    }
    function cancel() {
        recordingIndex = -2;
        heldModifiers = 0;
    }
    function refuse(message) {
        problem = message || "這個鍵不能用";
        heldModifiers = 0;
        if (motion)
            shake.restart();
    }
    function accept(index) {
        problem = "";
        saved = true;
        settledIndex = index;
        settle.restart();
    }
    function removeBinding(index) {
        cancel();
        if (shortcuts.remove(command, index))
            problem = "";
        else
            refuse(shortcuts.error);
    }
    function restoreDefaults() {
        cancel();
        if (shortcuts.reset(command))
            problem = "";
        else
            refuse(shortcuts.error);
    }
    function goTo(other) {
        const entry = shortcuts.commands.find(item => item.id === other);
        if (entry)
            field.navigateRequested(entry.page, entry.label, entry.row);
    }

    Keys.priority: Keys.BeforeItem
    Keys.onShortcutOverride: event => {
        if (recording)
            event.accepted = true;
    }
    Keys.onPressed: event => {
        if (!recording)
            return;
        event.accepted = true;
        if (event.isAutoRepeat)
            return;
        heldModifiers = event.modifiers;
        if (event.key === Qt.Key_Escape && event.modifiers === Qt.NoModifier) {
            cancel();
            return;
        }
        if ([Qt.Key_Shift, Qt.Key_Control, Qt.Key_Alt, Qt.Key_Meta, Qt.Key_AltGr, Qt.Key_Super_L, Qt.Key_Super_R].includes(event.key))
            return;
        const index = recordingIndex >= bindings.length ? -1 : recordingIndex;
        if (shortcuts.record(command, event.key, event.modifiers, index)) {
            const settledAt = index < 0 ? bindings.length - 1 : index;
            cancel();
            accept(settledAt);
        } else {
            refuse(shortcuts.error);
        }
    }
    Keys.onReleased: event => {
        if (recording) {
            heldModifiers = event.modifiers;
            event.accepted = true;
        }
    }
    onActiveFocusChanged: if (!activeFocus && recording) cancel()
    Connections {
        target: field.Window.window
        function onActiveChanged() {
            if (field.Window.window && !field.Window.window.active)
                field.cancel();
        }
    }
    HoverHandler {
        id: ownHover
    }
    Timer {
        id: settle
        interval: 1000
        onTriggered: {
            field.saved = false;
            field.settledIndex = -1;
        }
    }
    SequentialAnimation {
        id: shake
        NumberAnimation { target: displacement; property: "x"; to: -6; duration: 50 }
        NumberAnimation { target: displacement; property: "x"; to: 5; duration: 50 }
        NumberAnimation { target: displacement; property: "x"; to: -3; duration: 50 }
        NumberAnimation { target: displacement; property: "x"; to: 2; duration: 50 }
        NumberAnimation { target: displacement; property: "x"; to: 0; duration: 50 }
    }

    ColumnLayout {
        id: contents
        width: field.width
        spacing: 4
        Item {
            Layout.fillWidth: true
            implicitHeight: chips.implicitHeight
            Flow {
                id: chips
                readonly property bool activeFocusInside: {
                    for (let i = 0; i < children.length; ++i)
                        if (children[i].activeFocus)
                            return true;
                    return false;
                }
                // Right to left so the chips sit against the row's trailing edge; the model is reversed to keep the order.
                width: parent.width
                layoutDirection: Qt.RightToLeft
                spacing: 6
                transform: Translate {
                    id: displacement
                }
                Repeater {
                    model: field.items.slice().reverse()
                    delegate: Loader {
                        id: slot
                        required property var modelData
                        sourceComponent: modelData.kind === "add" ? ghostAdd : modelData.kind === "empty" ? emptyState
                            : modelData.kind === "more" ? moreChip : modelData.kind === "recorder" ? recorderChip : chip
                        onLoaded: item.entry = modelData
                    }
                }
            }
            // 還原預設 on hover, beside the field on its free side; it takes no room.
            Text {
                id: restoreLink
                objectName: "shortcutReset"
                visible: field.customized && !field.disabled && !field.recording
                opacity: field.editing ? 1 : 0
                enabled: field.editing
                // Left of the leftmost chip (the Repeater itself sits at 0 and has no size).
                x: {
                    let left = chips.width;
                    for (let i = 0; i < chips.children.length; ++i) {
                        const child = chips.children[i];
                        if (child.visible && child.width > 0)
                            left = Math.min(left, child.x);
                    }
                    return chips.x + left - width - 10;
                }
                y: Math.max(0, (Math.min(chips.implicitHeight, 22) - height) / 2)
                text: "還原預設"
                color: restoreHover.hovered ? field.theme.accentHover : field.theme.accent
                font.pixelSize: field.theme.detailSize
                Accessible.role: Accessible.Link
                Accessible.name: "還原預設：" + (field.definition ? field.definition.defaultsSpoken : "")
                ToolTip.visible: restoreHover.hovered
                ToolTip.delay: 500
                ToolTip.text: "還原成 " + (field.definition ? field.definition.defaultsSpoken : "")
                Behavior on opacity {
                    NumberAnimation { duration: field.theme.motionEnabled ? 120 : 0 }
                }
                HoverHandler {
                    id: restoreHover
                    cursorShape: Qt.PointingHandCursor
                }
                TapHandler {
                    onTapped: field.restoreDefaults()
                }
            }
        }
        Repeater {
            model: field.recording ? [] : Object.keys(field.conflicts)
            delegate: RowLayout {
                id: conflictLine
                required property string modelData
                readonly property string other: field.conflicts[modelData]
                readonly property var otherDefinition: field.shortcuts.commands.find(item => item.id === other)
                Layout.alignment: Qt.AlignRight
                spacing: 6
                ToolTip.visible: conflictHover.hovered
                ToolTip.delay: 500
                ToolTip.text: "「" + (otherDefinition ? otherDefinition.label : other) + "」也用這個按鍵；兩個功能都按下時只有一個會作用"
                HoverHandler {
                    id: conflictHover
                }
                Icon {
                    name: "warning"
                    size: 13
                    color: field.theme.danger
                }
                Text {
                    text: "也用於「" + (conflictLine.otherDefinition ? conflictLine.otherDefinition.label : conflictLine.other) + "」"
                    color: field.theme.danger
                    font.pixelSize: field.theme.detailSize
                    elide: Text.ElideRight
                    Layout.maximumWidth: field.width * 0.6
                }
                LinkText {
                    Layout.leftMargin: 4
                    text: "改按鍵"
                    onActivated: field.start(Number(conflictLine.modelData))
                }
                LinkText {
                    text: "前往"
                    onActivated: field.goTo(conflictLine.other)
                }
            }
        }
        Text {
            Layout.fillWidth: true
            visible: field.problem.length > 0
            text: field.problem
            color: field.theme.danger
            font.pixelSize: field.theme.detailSize
            horizontalAlignment: Text.AlignRight
            wrapMode: Text.Wrap
            Accessible.role: Accessible.StaticText
        }
    }

    component LinkText: Text {
        id: link
        signal activated
        color: linkHover.hovered ? field.theme.accentHover : field.theme.accent
        font.pixelSize: field.theme.detailSize
        Accessible.role: Accessible.Link
        Accessible.name: text
        HoverHandler {
            id: linkHover
            cursorShape: Qt.PointingHandCursor
        }
        TapHandler {
            onTapped: link.activated()
        }
    }

    Component {
        id: chip
        AbstractButton {
            id: button
            property var entry: ({ index: 0 })
            readonly property int binding: entry.index
            readonly property var value: field.bindings[binding] || ({ label: "", spoken: "" })
            readonly property bool conflict: field.conflicts[String(binding)] !== undefined
            readonly property bool removing: field.editing && hovered
            objectName: "shortcutBinding" + binding
            focusPolicy: Qt.StrongFocus
            implicitHeight: cap.implicitHeight
            implicitWidth: cap.implicitWidth
            enabled: !field.disabled
            scale: field.settledIndex === binding && field.motion ? 1.08 : 1
            Behavior on scale {
                SpringAnimation { spring: 4; damping: 0.35; epsilon: 0.005 }
            }
            Accessible.role: Accessible.Button
            Accessible.name: value.spoken || value.label
            Accessible.description: "按下可改按鍵；Backspace 刪除"
            ToolTip.visible: hovered && !removing
            ToolTip.delay: 600
            ToolTip.text: (value.spoken || value.label) + (conflict ? "：也用於另一個功能；點一下改按鍵" : "：點一下改按鍵")
            contentItem: Item {}
            background: KeyCap {
                id: cap
                theme: field.theme
                text: button.value.label
                tint: button.conflict ? field.theme.danger : "transparent"
                hovered: button.hovered
                pressed: button.down
                focused: button.visualFocus
                fadeEnd: button.removing
            }
            onClicked: field.start(binding)
            Keys.onPressed: event => {
                if ((event.key === Qt.Key_Backspace || event.key === Qt.Key_Delete) && event.modifiers === Qt.NoModifier) {
                    event.accepted = true;
                    if (!event.isAutoRepeat)
                        field.removeBinding(binding);
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    event.accepted = true;
                    field.start(binding);
                }
            }
            // ✕ inside the chip's trailing edge, over the faded end of its text.
            AbstractButton {
                objectName: "shortcutRemove" + button.binding
                anchors.right: parent.right
                anchors.rightMargin: 2
                anchors.verticalCenter: parent.verticalCenter
                width: 14
                height: parent.height
                visible: button.removing
                focusPolicy: Qt.NoFocus
                Accessible.name: "刪除 " + (button.value.spoken || button.value.label)
                ToolTip.visible: hovered
                ToolTip.delay: 500
                ToolTip.text: "刪除 " + (button.value.spoken || button.value.label)
                contentItem: Item {
                    Icon {
                        anchors.centerIn: parent
                        name: "close"
                        size: 10
                        weight: 2.2
                        color: field.theme.secondary
                    }
                }
                background: Item {}
                onClicked: field.removeBinding(button.binding)
            }
            TapHandler {
                acceptedButtons: Qt.RightButton
                onTapped: chipMenu.popup()
            }
            AppMenu {
                id: chipMenu
                theme: field.theme
                Action {
                    text: "改按鍵…"
                    onTriggered: field.start(button.binding)
                }
                Action {
                    text: "新增按鍵"
                    enabled: field.bindings.length < field.maximum
                    onTriggered: field.start(-1)
                }
                Action {
                    text: "刪除"
                    onTriggered: field.removeBinding(button.binding)
                }
                Action {
                    text: "還原預設"
                    enabled: field.customized
                    onTriggered: field.restoreDefaults()
                }
            }
        }
    }
    // Add a key: a faint dashed chip on the free side, shown on hover; it keeps its place, so showing it moves nothing.
    Component {
        id: ghostAdd
        GhostChip {
            property var entry
            objectName: "shortcutAdd"
            text: "＋"
            label: "新增按鍵"
            opacity: field.editing ? 1 : 0
            enabled: !!field.definition && field.bindings.length < field.maximum && !field.disabled
            onClicked: field.start(-1)
        }
    }
    Component {
        id: emptyState
        Item {
            property var entry
            implicitWidth: emptyAdd.implicitWidth
            implicitHeight: emptyAdd.implicitHeight
            Text {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: "—"
                color: field.theme.tertiary
                opacity: field.editing ? 0 : 1
                font.pixelSize: field.theme.detailSize
                Accessible.name: "沒有按鍵"
            }
            GhostChip {
                id: emptyAdd
                objectName: "shortcutAdd"
                anchors.right: parent.right
                text: "＋ 新增按鍵"
                label: "新增按鍵"
                opacity: field.editing ? 1 : 0
                enabled: !!field.definition && !field.disabled
                onClicked: field.start(0)
            }
        }
    }
    // 「+N」: more keys than the overview shows; pointing at it or clicking opens them.
    Component {
        id: moreChip
        AbstractButton {
            id: more
            property var entry: ({ index: 0 })
            objectName: "shortcutMore"
            implicitHeight: 22
            implicitWidth: moreText.implicitWidth + 12
            focusPolicy: Qt.StrongFocus
            Accessible.name: "還有 " + entry.index + " 個按鍵"
            ToolTip.visible: hovered
            ToolTip.delay: 500
            ToolTip.text: "還有 " + entry.index + " 個按鍵"
            onClicked: field.expanded = true
            HoverHandler {
                onHoveredChanged: if (hovered) field.expanded = true
            }
            background: Rectangle {
                radius: 5
                color: field.theme.hover
            }
            contentItem: Text {
                id: moreText
                text: "+" + more.entry.index
                color: field.theme.secondary
                font.pixelSize: field.theme.detailSize
                font.weight: Font.Medium
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }
        }
    }
    // The key being recorded: an accent outline with the modifiers held so far and a caret.
    Component {
        id: recorderChip
        Rectangle {
            property var entry
            objectName: "shortcutRecorder"
            implicitHeight: 22
            implicitWidth: Math.max(30, held.implicitWidth + caret.width + 15)
            radius: 5
            color: field.theme.accentSoft
            border.color: field.theme.accent
            border.width: 1.5
            Accessible.role: Accessible.StaticText
            Accessible.name: "按下新的組合"
            ToolTip.visible: recorderHover.hovered
            ToolTip.text: "按下新的組合；Esc 取消"
            HoverHandler {
                id: recorderHover
            }
            Row {
                anchors.centerIn: parent
                spacing: 1
                Text {
                    id: held
                    anchors.verticalCenter: parent.verticalCenter
                    text: field.shortcuts ? field.shortcuts.modifierLabel(field.heldModifiers) : ""
                    color: field.theme.accent
                    font.pixelSize: field.theme.detailSize
                    font.weight: Font.Medium
                }
                Rectangle {
                    id: caret
                    anchors.verticalCenter: parent.verticalCenter
                    width: 1.5
                    height: 12
                    radius: 0.75
                    color: field.theme.accent
                    SequentialAnimation on opacity {
                        running: field.recording && field.motion
                        loops: Animation.Infinite
                        NumberAnimation { to: 0.25; duration: 550; easing.type: Easing.InOutSine }
                        NumberAnimation { to: 1; duration: 550; easing.type: Easing.InOutSine }
                    }
                }
            }
        }
    }
    component GhostChip: AbstractButton {
        id: ghost
        property string label: ""
        implicitHeight: 22
        implicitWidth: Math.max(22, ghostText.implicitWidth + 12)
        focusPolicy: Qt.NoFocus
        Accessible.name: label
        ToolTip.visible: hovered && text === "＋"
        ToolTip.delay: 500
        ToolTip.text: label
        Behavior on opacity {
            NumberAnimation { duration: field.theme.motionEnabled ? 120 : 0 }
        }
        background: Canvas {
            id: dashes
            readonly property color stroke: Qt.rgba(field.theme.secondary.r, field.theme.secondary.g, field.theme.secondary.b, 0.5)
            onStrokeChanged: requestPaint()
            onWidthChanged: requestPaint()
            onPaint: {
                const context = getContext("2d");
                context.reset();
                context.strokeStyle = stroke;
                context.lineWidth = 1;
                context.setLineDash([3, 2]);
                const r = 5;
                context.beginPath();
                context.roundedRect(0.5, 0.5, width - 1, height - 1, r, r);
                context.stroke();
            }
        }
        contentItem: Text {
            id: ghostText
            text: ghost.text
            color: field.theme.secondary
            font.pixelSize: field.theme.detailSize
            font.weight: Font.Medium
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
    }
}
