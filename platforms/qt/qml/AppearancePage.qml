pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// 視窗與提示 (macOS CandidatesPane): one preview at the top drawn with the real CandidatePanel, then the settings.
// Left alone, the preview plays its states one after another, each as a short flow; a change below switches it to the
// state that shows it, and if the preview is scrolled out of view, that state shows beside the pointer for a moment.
// It holds still for 2 s after the user picks a tab, changes a setting or presses in it, and under Reduce Motion; the
// pointer resting over it does not pause it (UnifiedPreview.swift).
ColumnLayout {
    id: page
    required property var settings
    required property var candidates
    required property var correctionCandidates
    required property Theme theme
    property var shortcuts: null
    property string pageId: "appearance"
    signal navigateRequested(string pageId, string title, string row)
    property string scene: "list"
    property int frame: 1
    property bool ready: false
    property var lastValues: ({})
    property var lastKeys: ({})
    readonly property var values: settings.values
    readonly property bool motion: theme.motionEnabled && !theme.reduceMotion

    // The current keys (CurrentKeys on macOS): a command's first key, or "" when its feature is off or it has none.
    function key(command, fallback, feature) {
        if (shortcuts && shortcuts.values !== undefined)
            return shortcuts.activeLabel(command);
        return values[feature] === false ? "" : fallback;
    }
    readonly property string correctionsKey: key("correctionsList", "⇧↑", "feature.correctionsList")
    readonly property string phraseKey: key("selectLeft", "⇧←", "feature.selectPhrase")
    readonly property var nextKeys: shortcuts && shortcuts.values !== undefined
        ? (values["feature.pageKeys"] === false ? [] : (shortcuts.values.nextPage || []).slice(0, 3).map(binding => binding.label))
        : (values["feature.pageKeys"] === false ? [] : ["Tab", "PgDn", "Alt+↓"])
    readonly property string modeKey: values.shiftSwitch !== false
        ? (values.shiftToggle === "left" ? "左 Shift" : values.shiftToggle === "right" ? "右 Shift" : "Shift")
        : values["feature.capsLockEnglish"] !== false ? "Caps Lock" : ""
    readonly property string selectionText: {
        const keys = String(values.selectionKeys || "123456789").slice(0, Number(values.candidatesPerPage || 9)).toUpperCase().split("");
        return /^[0-9]+$/.test(keys.join("")) ? keys[0] + "–" + keys[keys.length - 1] : keys.join(" ");
    }
    // The states the settings allow, with the note beside the scene (StateNote.text) and the key pressed in it.
    readonly property var states: [
        {
            id: "list",
            title: "候選字清單",
            note: "按 ↓" + (values.spaceAction === "candidates" ? "或空白" : "") + " 開啟；" + selectionText + " 選字，Return 選取反白的字。",
            key: "↓"
        },
        {
            id: "corrections",
            title: "修正清單",
            note: (correctionsKey ? "按 " + correctionsKey + " " : "") + "列出按錯修正與模糊音的字" + (values.showsAnnotations !== false ? "，修正的讀音以灰字標出。" : "。"),
            key: correctionsKey
        },
        {
            id: "composing",
            title: "組字中",
            note: "打完一個注音就轉成中文，整句一起轉換；要改的字可以之後再選。",
            key: "ㄒ"
        },
        {
            id: "paging",
            title: "翻頁",
            note: (nextKeys.length ? "按 " + nextKeys.join("、") + " 翻到下一頁" : "清單中按空白翻頁") + (values.panelAnimation !== false ? "，舊的一頁移出、新的一頁移入。" : "。"),
            key: nextKeys.length ? nextKeys[0] : "空白"
        },
        {
            id: "phrase",
            title: "加入自訂詞",
            note: "選取 1 到 6 個字，再按 Return 加入自訂詞。",
            key: phraseKey
        },
        {
            id: "mode",
            title: "中英切換",
            note: modeKey ? "按 " + modeKey + " 切換中英" + (values.showModeToast !== false ? "，游標旁顯示「中」或「英」。" : "。") : "沒有設定切換中英的按鍵。",
            key: modeKey
        }
    ].filter(state => state.id !== "corrections" || !!correctionsKey)
        .filter(state => state.id !== "paging" || (values["feature.pageKeys"] !== false && candidates.pageCount > 1))
        .filter(state => state.id !== "phrase" || !!phraseKey)
        .filter(state => state.id !== "mode" || !!modeKey)
    readonly property var current: states.find(state => state.id === scene) || states[0]
    readonly property bool listScene: scene === "list" || scene === "corrections" || scene === "paging"
    readonly property bool above: values.candidatePlacement === "above"
    // A compact page of four candidates; every candidate of a page for a while after 每頁的候選字 changes.
    readonly property int pageLimit: fullPage.running ? values.candidatesPerPage : Math.min(4, values.candidatesPerPage)
    spacing: theme.sectionSpacing

    function resizeLists() {
        candidates.pageSize = pageLimit;
        correctionCandidates.pageSize = pageLimit;
    }
    function showScene(id, hold) {
        if (!states.some(state => state.id === id))
            id = "list";
        scene = id;
        frame = 1;
        if (hold)
            holdPlayback.restart();
        if (theme.motionEnabled)
            sceneEnter.restart();
    }
    function advance() {
        if (frame === 0) {
            frame = 1;
            if (scene === "paging")
                candidates.turnPage(candidates.page + 1 < candidates.pageCount ? 1 : -1);
        } else if (holdPlayback.running) {
            // Just picked or changed: the same state plays again.
            frame = 0;
        } else {
            const index = states.findIndex(state => state.id === scene);
            showScene(states[(index + 1) % states.length].id, false);
            frame = 0;
        }
    }
    // The state that shows a change (PreviewFlows.state(showing:)), or "" when nothing the preview shows changed.
    function stateFor(changed) {
        const groups = [
            ["corrections", ["showsAnnotations", "showsListSwitches", "feature.correctionsList", "keys.correctionsList"]],
            ["paging", ["panelAnimation", "pageAtListEdge", "feature.pageKeys", "keys.nextPage", "keys.previousPage"]],
            ["mode", ["showModeToast", "shiftSwitch", "shiftToggle", "feature.capsLockEnglish"]],
            ["list", ["candidateLayout", "candidateFontSize", "candidateDensity", "candidatesPerPage", "selectionKeys", "showsZhuyinHint",
                    "feature.zhuyinRowClick", "candidatePlacement", "panelAppearance", "feature.singleCharacterList", "keys.singleCharacterList"]]
        ];
        for (const [state, keys] of groups)
            if (changed.some(key => keys.includes(key)))
                return state;
        return "";
    }
    function keySnapshot() {
        const result = {};
        for (const command of ["correctionsList", "nextPage", "previousPage", "singleCharacterList"])
            result["keys." + command] = key(command, "", "");
        return result;
    }
    function noteChange() {
        if (!ready)
            return;
        const now = Object.assign({}, values, keySnapshot());
        const changed = Object.keys(now).filter(key => lastValues[key] !== now[key]);
        lastValues = now;
        if (changed.includes("candidatesPerPage"))
            fullPage.restart();
        resizeLists();
        const state = stateFor(changed);
        if (!state)
            return;
        showScene(state, true);
        flash();
    }
    onValuesChanged: noteChange()
    Connections {
        target: page.shortcuts
        ignoreUnknownSignals: true
        function onChanged() {
            page.noteChange();
        }
    }
    Component.onCompleted: {
        lastValues = Object.assign({}, values, keySnapshot());
        resizeLists();
        ready = true;
    }
    Timer {
        id: fullPage
        interval: 2000
        onRunningChanged: if (page.ready)
            page.resizeLists()
    }
    Timer {
        id: holdPlayback
        interval: 2000
    }
    // A press or a drag in the preview pauses it; it goes on about 2 s after.
    Timer {
        id: pressed
        interval: 2000
    }
    Timer {
        id: playback
        objectName: "appearancePlayback"
        interval: 1400
        repeat: true
        running: page.visible && page.motion && !pressed.running
        onTriggered: page.advance()
    }
    NumberAnimation {
        id: sceneEnter
        target: stage
        property: "opacity"
        from: 0
        to: 1
        duration: 220
        easing.type: Easing.OutCubic
    }
    // The pointer, for the floating preview after a change while the preview is out of view.
    HoverHandler {
        id: pointer
    }
    function flash() {
        const window = page.Window.window;
        if (!window || !page.visible)
            return;
        const bottom = stage.mapToItem(null, 0, stage.height).y;
        if (bottom > 60)
            return;
        const at = pointer.hovered ? page.mapToItem(null, pointer.point.position.x, pointer.point.position.y) : Qt.point(window.width / 2, window.height / 2);
        flashPopup.place(at);
    }

    SectionCard {
        theme: page.theme
        padded: true
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 10
            PillTabs {
                Layout.fillWidth: true
                Layout.preferredHeight: childrenRect.height
                theme: page.theme
                model: page.states
                current: page.scene
                objectPrefix: "sceneTab_"
                onSelected: id => page.showScene(id, true)
            }
            Item {
                id: stage
                objectName: "appearanceStage"
                Layout.fillWidth: true
                implicitHeight: stageContent.implicitHeight
                TapHandler {
                    acceptedButtons: Qt.AllButtons
                    onPressedChanged: pressed.restart()
                }
                GridLayout {
                    id: stageContent
                    anchors {
                        left: parent.left
                        right: parent.right
                        top: parent.top
                    }
                    // The note beside the scene when there is room for it.
                    columns: width >= 540 && page.values.candidateLayout !== "horizontal" ? 2 : 1
                    columnSpacing: 18
                    rowSpacing: 12
                    Item {
                        id: sceneDocument
                        Layout.preferredWidth: stageContent.columns === 2 ? Math.max(300, preview.implicitWidth + 16) : stageContent.width
                        Layout.fillWidth: stageContent.columns === 1
                        Layout.minimumWidth: 0
                        implicitHeight: page.listScene ? preview.implicitHeight + 34 : 96
                        // The line being typed, with the key just pressed beside it.
                        Row {
                            id: line
                            y: page.above ? sceneDocument.height - height : 0
                            spacing: 8
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: page.scene === "composing" ? (page.frame ? "王小明" : "王ㄒ") : page.scene === "mode" ? "王小明" : "約了王曉明"
                                textFormat: Text.PlainText
                                font.pixelSize: page.theme.headingSize
                                font.underline: page.scene === "composing" || page.scene === "phrase"
                                color: page.theme.text
                            }
                            KeyCap {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: page.current.key.length > 0 && (page.frame > 0 || page.scene === "composing")
                                theme: page.theme
                                text: page.current.key
                            }
                        }
                        CandidatePanel {
                            id: preview
                            objectName: "appearanceCandidates"
                            visible: page.listScene && (page.frame > 0 || page.scene === "paging")
                            x: 0
                            y: page.above ? 0 : line.height + 6
                            width: implicitWidth
                            height: implicitHeight
                            maximumWidth: sceneDocument.width
                            candidates: page.scene === "corrections" ? page.correctionCandidates : page.candidates
                            preferences: page.values
                            shortcuts: page.shortcuts
                            theme: page.theme
                            onReadingClicked: pressed.restart()
                        }
                        // 加入自訂詞: the engine's notice, in the panel as a compact pill.
                        CandidatePanel {
                            visible: page.scene === "phrase"
                            y: page.above ? 0 : line.height + 6
                            width: implicitWidth
                            height: implicitHeight
                            maximumWidth: sceneDocument.width
                            candidates: QtObject {
                                readonly property var rows: []
                                readonly property string reading: ""
                                readonly property int page: 0
                                readonly property int pageCount: 0
                                readonly property int pageSize: 9
                                readonly property int selected: -1
                                readonly property bool single: false
                                readonly property bool corrections: false
                            }
                            hint: page.frame ? "已加入自訂詞：王小明" : "Enter 加入自訂詞：王小明"
                            preferences: page.values
                            theme: page.theme
                        }
                        // 中／英: the toast at its own size.
                        Rectangle {
                            visible: page.scene === "mode" && page.values.showModeToast !== false && page.frame > 0
                            y: page.above ? 0 : line.height + 6
                            width: 52
                            height: 48
                            radius: 14
                            color: page.theme.surface
                            border.color: page.theme.line
                            Text {
                                anchors.centerIn: parent
                                text: "英"
                                color: page.theme.text
                                font.pixelSize: Math.round(page.theme.titleSize * 1.15)
                                font.weight: Font.Medium
                            }
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        Layout.maximumWidth: 260
                        Layout.alignment: Qt.AlignTop
                        Layout.topMargin: stageContent.columns === 2 ? 10 : 0
                        spacing: 6
                        Text {
                            text: page.current.title
                            color: page.theme.text
                            font.pixelSize: page.theme.headingSize
                            font.weight: Font.DemiBold
                        }
                        Text {
                            Layout.fillWidth: true
                            text: page.current.note
                            color: page.theme.secondary
                            font.pixelSize: page.theme.detailSize
                            lineHeight: 1.2
                            wrapMode: Text.Wrap
                        }
                    }
                }
            }
        }
    }
    ColumnLayout {
        Layout.fillWidth: true
        spacing: page.theme.headerGap
        SettingsPage {
            Layout.fillWidth: true
            pageId: "appearance"
            shortcuts: page.shortcuts
            settings: page.settings
            theme: page.theme
            onNavigateRequested: (pageId, title, row) => page.navigateRequested(pageId, title, row)
        }
        // The 提示 section's footer: the page's one link to the overview of all keys (KeysOverviewLink).
        Text {
            id: overviewLink
            objectName: "keysOverviewLink"
            Layout.leftMargin: page.theme.cardInset
            text: "在快捷鍵總覽中查看全部按鍵"
            color: linkHover.hovered ? page.theme.accentHover : page.theme.accent
            font.pixelSize: page.theme.detailSize
            Accessible.role: Accessible.Link
            Accessible.name: text
            HoverHandler {
                id: linkHover
                cursorShape: Qt.PointingHandCursor
            }
            TapHandler {
                onTapped: page.navigateRequested("keys", "快捷鍵", "")
            }
        }
    }
    // The floating preview after a change: beside the pointer, above it when there is room; over the window, taking no
    // layout space (it is moved to the window's content item when shown).
    Rectangle {
        id: flashPopup
        objectName: "appearanceFlash"
        visible: opacity > 0
        opacity: 0
        z: 1000
        width: flashContent.implicitWidth + 20
        height: flashContent.implicitHeight + 20
        radius: 12
        color: page.theme.card
        border.color: page.theme.line
        function place(at) {
            const window = page.Window.window;
            if (!window)
                return;
            parent = window.contentItem;
            x = Math.max(12, Math.min(parent.width - width - 12, at.x - width / 2));
            const aboveY = at.y - height - 24;
            y = aboveY > 8 ? aboveY : Math.min(parent.height - height - 8, at.y + 24);
            opacity = 1;
            closer.restart();
        }
        Behavior on opacity {
            NumberAnimation { duration: page.theme.motionEnabled ? 200 : 0 }
        }
        Item {
            id: flashContent
            x: 10
            y: 10
            implicitWidth: flashPanel.visible ? flashPanel.implicitWidth : 52
            implicitHeight: flashPanel.visible ? flashPanel.implicitHeight : 48
            CandidatePanel {
                id: flashPanel
                visible: page.listScene
                width: implicitWidth
                height: implicitHeight
                maximumWidth: 280
                candidates: page.scene === "corrections" ? page.correctionCandidates : page.candidates
                preferences: page.values
                shortcuts: page.shortcuts
                theme: page.theme
            }
            Text {
                visible: !page.listScene
                anchors.centerIn: parent
                text: page.scene === "mode" ? "英" : page.current.key
                color: page.theme.text
                font.pixelSize: Math.round(page.theme.titleSize * 1.15)
                font.weight: Font.Medium
            }
        }
        Timer {
            id: closer
            interval: 2000
            onTriggered: flashPopup.opacity = 0
        }
    }
}
