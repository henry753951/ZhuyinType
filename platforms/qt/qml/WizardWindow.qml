pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The setup wizard (macOS UI/Wizard: WizardWindowController, WizardModel, WizardView; lessons and presets from
// Settings/Registry/Habits.swift). Its own window: 歡迎 → 習慣 → the lessons, each with its choices and a practice field →
// 設定好了 (原本 → 改成) → 你會用到的按鍵. Every choice changes only the sandbox (`practice`); 套用 writes the changes to the
// user's settings and keys, 全部捨棄 or 全部跳過 writes nothing. It opens from 介紹 and once on the first opening of the
// settings; never by itself while the user is typing.
Window {
    id: wizard
    objectName: "setupWizard"
    required property var settings
    required property var shortcuts
    /// The sandbox (Practice in C++): settings, shortcuts, engineSession, inputController; start(), restart(), stop().
    required property var practice
    /// The user's engine, for keeping the words added while practising.
    property var engineSession: null
    signal navigateRequested(string pageId)

    // MARK: Data (Habits.swift)

    readonly property string currentVersion: lessons.reduce((top, lesson) => newer(lesson.since, top) ? lesson.since : top, "0")
    readonly property bool hasNewLessons: settings.wizardSeenVersion.length > 0 && newer(currentVersion, settings.wizardSeenVersion)
    function newer(a, b) {
        const x = String(a).split(".").map(Number), y = String(b).split(".").map(Number);
        for (let i = 0; i < Math.max(x.length, y.length); ++i) {
            if ((x[i] || 0) !== (y[i] || 0))
                return (x[i] || 0) > (y[i] || 0);
        }
        return false;
    }
    function value(key, v, title, detail) {
        return {
            kind: "value",
            key: key,
            value: v,
            title: title,
            detail: detail || ""
        };
    }
    function binding(command, key, modifiers, title) {
        return {
            kind: "key",
            command: command,
            key: key,
            modifiers: modifiers,
            title: title,
            detail: ""
        };
    }
    readonly property var groups: ({
            space: {
                title: "空白鍵",
                exclusive: true,
                styles: [value("spaceAction", "commit", "送出並輸入空白", "打完注音後，再按空白送出組字並輸入空白"), value("spaceAction", "insert", "輸入空白，不送出", "打完注音後，再按空白在組字中輸入空白，不送出"), value("spaceAction", "candidates", "開啟候選字視窗", "打完注音後，再按空白開啟候選字視窗")]
            },
            punctuation: {
                title: "標點按法",
                exclusive: true,
                styles: [value("punctuationKeys", "windows", "Windows", "Ctrl 加符號鍵打中文標點，Shift 加符號鍵打鍵上的符號"), value("punctuationKeys", "mac", "macOS", "Shift 加符號鍵打中文標點")]
            },
            modeSwitch: {
                title: "中英切換",
                exclusive: false,
                styles: [value("shiftSwitch", true, "單按 Shift", "在可以打字的地方單按一下"), value("feature.capsLockEnglish", true, "Caps Lock 打英文", "開著時打英文")]
            },
            layout: {
                title: "排列",
                exclusive: true,
                styles: [value("candidateLayout", "vertical", "直式", "↑ ↓ 移動反白"), value("candidateLayout", "horizontal", "橫式", "← → 移動反白，↑ ↓ 翻頁")]
            },
            paging: {
                title: "翻頁",
                exclusive: false,
                styles: [binding("nextPage", Qt.Key_Tab, 0, "Tab"), binding("nextPage", Qt.Key_PageDown, 0, "Page Down"), binding("nextPage", Qt.Key_Down, Qt.AltModifier, "Alt+↓"), value("pageAtListEdge", true, "在頁底按 ↓ 翻到下一頁")]
            },
            selection: {
                title: "選字鍵",
                exclusive: true,
                styles: [value("selectionKeys", "123456789", "1–9"), value("selectionKeys", "asdfghjkl", "A S D F G H J K L")]
            },
            anchor: {
                title: "游標在句中時改",
                exclusive: true,
                styles: [value("candidateAnchor", "after", "游標右邊的字"), value("candidateAnchor", "before", "游標左邊的字")]
            },
            typo: {
                title: "按錯鍵修正",
                exclusive: true,
                styles: [value("typoTolerance", "off", "關閉", "照按下的鍵解讀注音"), value("typoTolerance", "standard", "標準", "修正按到相鄰的鍵、多按、漏按、順序與聲調"), value("typoTolerance", "aggressive", "積極", "比「標準」更容易採用修正")]
            },
            context: {
                title: "前文",
                exclusive: true,
                styles: [value("usesPrecedingText", true, "參考游標前的文字"), value("usesPrecedingText", false, "不參考")]
            },
            correctionLog: {
                title: "記錄改字",
                exclusive: true,
                styles: [value("recordsCorrections", false, "不記錄"), value("recordsCorrections", true, "記錄改字")]
            },
            fuzzy: {
                title: "模糊音",
                exclusive: false,
                styles: [value("fuzzy.zZh", true, "ㄗ 與 ㄓ"), value("fuzzy.cCh", true, "ㄘ 與 ㄔ"), value("fuzzy.sSh", true, "ㄙ 與 ㄕ"), value("fuzzy.enEng", true, "ㄣ 與 ㄥ"), value("fuzzy.anAng", true, "ㄢ 與 ㄤ")]
            },
            density: {
                title: "間距",
                exclusive: true,
                styles: [value("candidateDensity", "standard", "標準"), value("candidateDensity", "compact", "緊湊")]
            },
            placement: {
                title: "位置",
                exclusive: true,
                styles: [value("candidatePlacement", "below", "游標下方"), value("candidatePlacement", "above", "游標上方")]
            }
        })
    // The lessons in order; 快速設定 takes the basic ones. `check` is the task's goal on the practice field's outcome.
    readonly property var lessons: [
        {
            id: "typing",
            title: "打字與空白鍵",
            detail: "打完字以後，空白鍵做什麼由你決定。",
            level: "basic",
            since: "0.4.2",
            groups: ["space"],
            prompt: "打出「今天天氣很好」",
            hint: "ㄐㄧㄣ ㄊㄧㄢ ㄊㄧㄢ ㄑㄧˋ ㄏㄣˇ ㄏㄠˇ",
            check: o => o.text.indexOf("今天天氣很好") >= 0
        },
        {
            id: "punctuation",
            title: "標點",
            detail: "按住修飾鍵，再按符號鍵打出標點。",
            level: "basic",
            since: "0.4.2",
            groups: ["punctuation"],
            prompt: "打出「好，謝謝。」",
            check: o => o.text.indexOf("，") >= 0 && o.text.indexOf("。") >= 0
        },
        {
            id: "modeSwitch",
            title: "中英切換",
            detail: "切換時，正在組的字會先送出。",
            level: "basic",
            since: "0.4.2",
            groups: ["modeSwitch"],
            prompt: "切換到英文打「OK」，再切回中文打「好」",
            check: o => o.text.indexOf("OK好") >= 0 && o.events.filter(e => e === "mode").length >= 2
        },
        {
            id: "selecting",
            title: "選字",
            detail: "候選字的排列，和用來選字的鍵。",
            level: "basic",
            since: "0.4.2",
            groups: ["layout", "selection"],
            prompt: "打 ㄇㄧㄥˊ，用選字鍵選「名」",
            check: o => o.picks.indexOf("名") >= 0
        },
        {
            id: "paging",
            title: "翻頁",
            detail: "這些翻頁方式同時有效，不用的可以關掉。",
            level: "basic",
            since: "0.4.2",
            groups: ["paging"],
            prompt: "打 ㄇㄧㄥˊ，翻到下一頁，選「暝」",
            check: o => o.picks.indexOf("暝") >= 0 && o.events.indexOf("page") >= 0
        },
        {
            id: "editing",
            title: "改字",
            detail: "按 ↓ 列出其他的字，選一個換掉。",
            level: "basic",
            since: "0.4.2",
            groups: ["anchor"],
            prompt: "打「謝謝你」，再把「你」改成「妳」",
            check: o => o.text.indexOf("謝謝妳") >= 0 && o.picks.indexOf("妳") >= 0
        },
        {
            id: "typo",
            title: "按錯鍵與修正清單",
            detail: "按到相鄰的鍵、多按或漏按時，試著修正。",
            level: "full",
            since: "0.4.2",
            groups: ["typo"],
            prompt: "照提示按錯一個鍵，再用修正清單選到正確的字",
            check: o => o.events.indexOf("corrections") >= 0 && o.picks.length > 0
        },
        {
            id: "fuzzy",
            title: "模糊音",
            detail: "容易混淆的兩個音互相通用。",
            level: "optional",
            since: "0.4.2",
            groups: ["fuzzy"],
            prompt: "開啟「ㄙ 與 ㄕ」，用 ㄙㄨ ㄈㄨˊ 打出「舒服」",
            check: o => o.text.indexOf("舒服") >= 0
        },
        {
            id: "context",
            title: "前文",
            detail: "參考游標前的文字選字，不會存下。",
            level: "full",
            since: "0.4.2",
            groups: ["context"],
            prompt: "在「天氣」後面接著打出「很好」",
            initialText: "天氣",
            check: o => o.text.indexOf("天氣很好") >= 0
        },
        {
            id: "phrase",
            title: "加入自訂詞",
            detail: "選取 1 到 6 個字，按 Return 加入。",
            level: "full",
            since: "0.4.2",
            groups: [],
            prompt: "選取「王小明」，按 Return 加入",
            check: o => o.events.indexOf("phrase") >= 0
        },
        {
            id: "look",
            title: "候選字視窗的樣子",
            detail: "預覽就是真正的候選字視窗。",
            level: "full",
            since: "0.4.2",
            groups: ["density", "placement", "selection"],
            prompt: ""
        },
        {
            id: "correctionLog",
            title: "改字記錄",
            detail: "記下改字的過程來改進選字，只存在這台電腦。",
            level: "advanced",
            since: "0.4.2",
            groups: ["correctionLog"],
            prompt: ""
        }
    ]
    readonly property var presets: [
        {
            id: "windows",
            title: "Windows 注音的習慣",
            lines: [{
                    keys: ["Shift"],
                    text: "單按切換中英"
                }, {
                    keys: ["Ctrl", "，"],
                    text: "打中文標點"
                }, {
                    keys: ["1–9"],
                    text: "選字"
                }],
            values: {
                shiftSwitch: true,
                shiftToggle: "both",
                startInEnglish: false,
                "feature.controlPunctuation": true,
                selectionKeys: "123456789"
            }
        },
        {
            id: "mac",
            title: "macOS 注音的習慣",
            lines: [{
                    keys: ["空白"],
                    text: "開啟候選字視窗"
                }, {
                    keys: ["Caps Lock"],
                    text: "切換中英"
                }, {
                    keys: ["Shift"],
                    text: "不切換中英"
                }],
            values: {
                spaceAction: "candidates",
                shiftSwitch: false,
                "feature.capsLockEnglish": true,
                "feature.optionSymbols": true,
                selectionKeys: "123456789"
            }
        },
        {
            id: "default",
            title: "ZhuyinType 預設",
            lines: [{
                    keys: ["空白"],
                    text: "送出並輸入空白"
                }, {
                    keys: ["Ctrl", "，"],
                    text: "打中文標點"
                }, {
                    keys: ["Shift"],
                    text: "單按切換中英"
                }],
            values: Object.assign(settings.definitions.filter(d => d.key.indexOf("feature.") !== 0).reduce((all, d) => {
                    all[d.key] = d["default"];
                    return all;
                }, {}), {
                shiftSwitch: true
            })
        },
        {
            id: "custom",
            title: "自訂",
            lines: [{
                    keys: [],
                    text: "從目前的設定開始"
                }, {
                    keys: [],
                    text: "每一步自己選"
                }],
            values: ({})
        }
    ]

    // MARK: Model (WizardModel.swift)

    property string mode: "quick"
    property int index: 0
    property int direction: 1
    property string preset: "custom"
    property var completed: ({})
    property bool keepPracticePhrases: false
    readonly property var sandbox: practice.settings
    readonly property var sandboxShortcuts: practice.shortcuts
    readonly property var modeLessons: mode === "quick" ? lessons.filter(l => l.level === "basic") : mode === "full" ? lessons : mode === "newSince" ? lessons.filter(l => newer(l.since, settings.wizardSeenVersion || "0")) : []
    readonly property var steps: mode === "keysOnly" ? ["keys"] : mode === "newSince" ? modeLessons.map(l => "lesson:" + l.id).concat(["finish", "keys"]) : ["welcome", "preset"].concat(modeLessons.map(l => "lesson:" + l.id), ["finish", "keys"])
    readonly property string step: steps[Math.min(index, steps.length - 1)]
    readonly property var lesson: step.indexOf("lesson:") === 0 ? lessons.find(l => "lesson:" + l.id === step) : null
    readonly property var counted: steps.filter(s => s !== "welcome")
    readonly property var changes: changesNow()
    readonly property var practicePhrases: practice.engineSession && practice.engineSession.userPhrases ? practice.engineSession.userPhrases : []

    function show(newMode) {
        mode = newMode || "quick";
        index = 0;
        direction = 1;
        preset = "custom";
        completed = {};
        keepPracticePhrases = false;
        practice.start(mode !== "keysOnly");
        title = mode === "keysOnly" ? "你會用到的按鍵" : mode === "newSince" ? "新功能導覽" : "設定嚮導";
        visible = true;
        raise();
        requestActivate();
    }
    /// The first opening of the settings after install also opens the wizard, once.
    function showOnFirstUse() {
        if (settings.wizardSeenVersion.length > 0)
            return;
        settings.markWizardSeen(currentVersion);
        show("quick");
    }
    function finishClose() {
        visible = false;
        practice.stop();
    }
    function go(target) {
        if (target < 0)
            return;
        if (target >= steps.length) {
            finishClose();
            return;
        }
        direction = target > index ? 1 : -1;
        index = target;
        stepMotion.restart();
    }
    function next() {
        go(index + 1);
    }
    function back() {
        go(index - 1);
    }
    /// 全部跳過 / 全部捨棄: nothing is written.
    function discard() {
        settings.markWizardSeen(currentVersion);
        finishClose();
    }
    function choose(newMode) {
        if (mode === newMode)
            return;
        mode = newMode;
        index = Math.min(index, steps.length - 1);
    }
    function choosePreset(entry) {
        preset = entry.id;
        practice.restart();
        sandbox.assign(entry.values);
    }
    function active(style) {
        if (!sandbox)
            return false;
        if (style.kind === "key")
            return keyIndex(style) >= 0;
        return sandbox.values[style.key] === style.value;
    }
    function keyIndex(style) {
        const list = sandboxShortcuts.values[style.command] || [];
        return list.findIndex(b => b.key === style.key && b.modifiers === style.modifiers);
    }
    function setStyle(style, on) {
        if (style.kind === "key") {
            const at = keyIndex(style);
            if (on && at < 0)
                sandboxShortcuts.record(style.command, style.key, style.modifiers, -1);
            else if (!on && at >= 0)
                sandboxShortcuts.remove(style.command, at);
        } else if (on) {
            sandbox.setValue(style.key, style.value);
        } else if (style.value === true) {
            sandbox.setValue(style.key, false);
        }
    }
    function label(definition, v) {
        if (definition.kind === "toggle")
            return v ? "開啟" : "關閉";
        const option = (definition.options || []).find(o => o.value === v);
        return option ? option.label : String(v);
    }
    function keyLabels(list) {
        return (list || []).map(b => b.label).join("、") || "無";
    }
    // What the wizard would change, 原本 → 改成.
    function changesNow() {
        if (!sandbox || !sandboxShortcuts)
            return [];
        const result = [];
        for (const definition of settings.definitions) {
            const was = settings.values[definition.key], now = sandbox.values[definition.key];
            if (was !== now)
                result.push({
                    id: definition.key,
                    title: definition.title,
                    old: label(definition, was),
                    now: label(definition, now),
                    definition: definition
                });
        }
        const wasKeys = shortcuts.values.nextPage, nowKeys = sandboxShortcuts.values.nextPage;
        if (keyLabels(wasKeys) !== keyLabels(nowKeys))
            result.push({
                id: "keys.nextPage",
                title: "下一頁",
                old: keyLabels(wasKeys),
                now: keyLabels(nowKeys),
                definition: null
            });
        return result;
    }
    function revert(change) {
        if (change.id === "keys.nextPage")
            copyKeys(shortcuts, sandboxShortcuts, "nextPage");
        else
            sandbox.setValue(change.id, settings.values[change.id]);
    }
    // One command's keys from `source` to `target`, through the conflict check.
    function copyKeys(source, target, command) {
        const wanted = source.values[command] || [];
        let list = target.values[command] || [];
        for (let i = list.length - 1; i >= 1; --i)
            target.remove(command, i);
        if (wanted.length === 0) {
            target.remove(command, 0);
            return;
        }
        target.record(command, wanted[0].key, wanted[0].modifiers, 0);
        for (let i = 1; i < wanted.length; ++i)
            target.record(command, wanted[i].key, wanted[i].modifiers, -1);
    }
    /// 套用: the changes go to the user's settings; practice phrases only if kept.
    function apply() {
        const updates = {};
        for (const change of changes) {
            if (change.id === "keys.nextPage")
                copyKeys(sandboxShortcuts, shortcuts, "nextPage");
            else
                updates[change.id] = sandbox.values[change.id];
        }
        settings.assign(updates);
        if (keepPracticePhrases && engineSession)
            for (const phrase of practicePhrases)
                engineSession.addPhrase(phrase.word, phrase.reading);
        settings.markWizardSeen(currentVersion);
        go(steps.indexOf("keys"));
    }
    // 你會用到的按鍵: from the user's settings, only what is on.
    function keyGroups() {
        const v = settings.values;
        const commands = shortcuts.commands;
        function items(ids) {
            return ids.map(id => commands.find(c => c.id === id)).filter(c => c && (!c.feature || v[c.feature] !== false)).map(c => ({
                        keys: (shortcuts.values[c.id] || []).slice(0, 2).map(b => b.label),
                        action: c.title
                    })).filter(item => item.keys.length > 0);
        }
        const mode = v.shiftSwitch ? (v.shiftToggle === "left" ? "左 Shift" : v.shiftToggle === "right" ? "右 Shift" : "Shift") : v["feature.capsLockEnglish"] !== false ? "Caps Lock" : "";
        const space = groups.space.styles.find(s => s.value === v.spaceAction);
        const typing = [{
                keys: ["空白"],
                action: space ? space.title : ""
            }, {
                keys: ["Esc"],
                action: v.escape === "reading" ? "清除注音" : "清除注音，再按一次清除組字"
            }].concat(mode ? [{
                    keys: [mode],
                    action: "切換中英"
                }] : [], items(["deleteWord", "commitWithNewline"]));
        const selectionText = v.selectionKeys === "asdfghjkl" ? "A S D F G H J K L" : "1–9";
        return [{
                title: "打字",
                icon: "typing",
                items: typing
            }, {
                title: "候選字清單",
                icon: "list",
                items: [{
                        keys: ["↓"],
                        action: "開啟候選字清單"
                    }, {
                        keys: [selectionText],
                        action: "選字"
                    }].concat(items(["singleCharacterList", "correctionsList", "nextPage", "previousPage"]))
            }, {
                title: "改字與選取",
                icon: "textCursor",
                items: items(["typeZhuyin", "selectLeft", "selectRight", "previousCharacter", "nextCharacter"])
            }].filter(group => group.items.length > 0);
    }

    // MARK: View (WizardView.swift)

    title: "設定嚮導"
    width: 780
    height: 600
    minimumWidth: 640
    minimumHeight: 520
    visible: false
    color: wizardTheme.canvas
    onClosing: close => {
        close.accepted = false;
        discard();
    }
    Theme {
        id: wizardTheme
        appearance: wizard.settings.values.panelAppearance
        motionEnabled: wizard.settings.values.panelAnimation !== false
    }
    readonly property Theme theme: wizardTheme
    readonly property real margin: 32

    // A lesson's title and what it is about.
    component StepTitle: ColumnLayout {
        property string title: ""
        property string detail: ""
        property real size: wizardTheme.titleSize + 6
        spacing: 6
        Text {
            Layout.fillWidth: true
            text: parent.title
            color: wizardTheme.text
            font.pixelSize: parent.size
            font.weight: Font.DemiBold
            wrapMode: Text.Wrap
        }
        Text {
            Layout.fillWidth: true
            visible: text.length > 0
            text: parent.detail
            color: wizardTheme.secondary
            font.pixelSize: wizardTheme.headingSize
            wrapMode: Text.Wrap
        }
    }
    // A group of styles: exclusive ones as a choice (only the chosen one shows its detail), combinable ones as checks.
    component StyleGroup: ColumnLayout {
        id: groupView
        property var group: ({
                styles: []
            })
        property bool across: false
        spacing: 8
        Text {
            text: groupView.group.title || ""
            color: wizardTheme.text
            font.pixelSize: wizardTheme.bodySize
            font.weight: Font.DemiBold
        }
        GridLayout {
            Layout.fillWidth: true
            columns: groupView.across ? Math.max(1, groupView.group.styles.length) : 1
            columnSpacing: 8
            rowSpacing: 6
            Repeater {
                model: groupView.group.styles
                delegate: OptionTile {
                    required property var modelData
                    Layout.fillWidth: true
                    theme: wizardTheme
                    title: modelData.title
                    multi: !groupView.group.exclusive
                    selected: {
                        wizard.sandbox ? wizard.sandbox.values : null;
                        wizard.sandboxShortcuts ? wizard.sandboxShortcuts.values : null;
                        return wizard.active(modelData);
                    }
                    ToolTip.visible: hovered && !groupView.group.exclusive && modelData.detail.length > 0
                    ToolTip.text: modelData.detail
                    ToolTip.delay: 500
                    onClicked: wizard.setStyle(modelData, groupView.group.exclusive ? true : !selected)
                }
            }
        }
        Text {
            Layout.fillWidth: true
            readonly property var chosen: {
                wizard.sandbox ? wizard.sandbox.values : null;
                return groupView.group.exclusive ? groupView.group.styles.find(style => wizard.active(style)) : null;
            }
            visible: !!chosen && chosen.detail.length > 0
            text: chosen ? chosen.detail : ""
            color: wizardTheme.secondary
            font.pixelSize: wizardTheme.detailSize
            wrapMode: Text.Wrap
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        // 上一步, the progress on the centre line, 全部跳過.
        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: 56
            AppButton {
                objectName: "wizardBack"
                anchors.verticalCenter: parent.verticalCenter
                x: wizard.margin - 10
                visible: wizard.step !== "welcome" && wizard.index > 0 && wizard.step !== "keys"
                theme: wizardTheme
                kind: "plain"
                compact: true
                iconName: "chevronLeft"
                text: "上一步"
                onClicked: wizard.back()
            }
            ColumnLayout {
                visible: wizard.step !== "welcome" && wizard.counted.length > 1
                anchors.centerIn: parent
                spacing: 5
                Row {
                    Layout.alignment: Qt.AlignHCenter
                    spacing: 4
                    Repeater {
                        model: wizard.counted.length
                        delegate: Rectangle {
                            required property int index
                            readonly property int at: wizard.counted.indexOf(wizard.step)
                            width: Math.min(14, Math.max(7, 220 / Math.max(1, wizard.counted.length) - 4)) * (index === at ? 1.6 : 1)
                            height: 4
                            radius: 2
                            color: index <= at ? wizardTheme.accent : wizardTheme.track
                            opacity: index < at ? 0.55 : 1
                        }
                    }
                }
                Text {
                    objectName: "wizardProgress"
                    Layout.alignment: Qt.AlignHCenter
                    text: (wizard.step === "preset" ? "習慣" : wizard.step === "finish" ? "確認" : wizard.step === "keys" ? "按鍵" : wizard.lesson ? wizard.lesson.title : "") + " · " + (wizard.counted.indexOf(wizard.step) + 1) + " / " + wizard.counted.length
                    color: wizardTheme.secondary
                    font.pixelSize: wizardTheme.captionSize
                    font.features: ({ "tnum": 1 })
                }
            }
            AppButton {
                objectName: "wizardSkipAll"
                anchors.verticalCenter: parent.verticalCenter
                anchors.right: parent.right
                anchors.rightMargin: wizard.margin - 10
                visible: wizard.step !== "finish" && wizard.step !== "keys"
                theme: wizardTheme
                kind: "plain"
                compact: true
                text: "全部跳過"
                onClicked: wizard.discard()
            }
        }
        Flickable {
            id: body
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: width
            contentHeight: stepContent.implicitHeight + 24
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            Loader {
                id: stepContent
                x: wizard.margin + motion.offset
                y: 8
                width: body.width - wizard.margin * 2
                opacity: motion.opacity
                height: Math.max(implicitHeight, body.height - 32)
                sourceComponent: wizard.step === "welcome" ? welcomeStep : wizard.step === "preset" ? presetStep : wizard.step === "finish" ? finishStep : wizard.step === "keys" ? keysStep : lessonStep
            }
        }
        // The step's buttons, at the bottom right.
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: wizard.margin
            Layout.rightMargin: wizard.margin
            Layout.topMargin: 10
            Layout.bottomMargin: 22
            spacing: 10
            AppButton {
                objectName: "wizardOwnKeys"
                visible: !!wizard.lesson && wizard.lesson.groups.indexOf("paging") >= 0
                theme: wizardTheme
                kind: "plain"
                compact: true
                text: "自行設定…"
                onClicked: wizard.navigateRequested("keys")
            }
            Item {
                Layout.fillWidth: true
            }
            AppButton {
                objectName: "wizardSkipStep"
                readonly property bool done: !!wizard.lesson && (!wizard.lesson.prompt || !!wizard.completed[wizard.lesson.id])
                visible: !!wizard.lesson && !done
                theme: wizardTheme
                text: "略過這一步"
                onClicked: wizard.next()
            }
            AppButton {
                visible: wizard.step === "finish"
                theme: wizardTheme
                text: "全部捨棄"
                onClicked: wizard.discard()
            }
            AppButton {
                objectName: "wizardNext"
                theme: wizardTheme
                kind: "primary"
                enabled: !wizard.lesson || !wizard.lesson.prompt || !!wizard.completed[wizard.lesson.id]
                text: wizard.step === "welcome" ? "開始" : wizard.step === "finish" ? "套用" : wizard.step === "keys" ? "完成" : "下一步"
                onClicked: {
                    if (wizard.step === "finish")
                        wizard.apply();
                    else if (wizard.step === "keys")
                        wizard.finishClose();
                    else
                        wizard.next();
                }
            }
        }
    }
    // The step transition: a push in the direction of travel; a crossfade with reduced motion.
    QtObject {
        id: motion
        property real offset: 0
        property real opacity: 1
    }
    ParallelAnimation {
        id: stepMotion
        NumberAnimation {
            target: motion
            property: "offset"
            from: wizardTheme.reduceMotion || !wizardTheme.motionEnabled ? 0 : wizard.direction > 0 ? 16 : -16
            to: 0
            duration: wizardTheme.motionEnabled ? 320 : 0
            easing.type: Easing.OutCubic
        }
        NumberAnimation {
            target: motion
            property: "opacity"
            from: wizardTheme.motionEnabled ? 0 : 1
            to: 1
            duration: wizardTheme.motionEnabled ? 240 : 0
            easing.type: Easing.OutCubic
        }
    }

    Component {
        id: welcomeStep
        ColumnLayout {
            spacing: 0
            Item {
                Layout.fillHeight: true
            }
            Image {
                Layout.alignment: Qt.AlignHCenter
                source: "qrc:/branding/AppIcon.icns"
                Layout.preferredWidth: 64
                Layout.preferredHeight: 64
                sourceSize.width: 128
                sourceSize.height: 128
                Accessible.ignored: true
            }
            Text {
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: 14
                text: "歡迎使用 ZhuyinType"
                color: wizardTheme.text
                font.pixelSize: wizardTheme.titleSize + 8
                font.weight: Font.DemiBold
            }
            Text {
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: 6
                text: "選好你的打字習慣，再實際試打。"
                color: wizardTheme.secondary
                font.pixelSize: wizardTheme.headingSize + 1
            }
            SectionCard {
                Layout.topMargin: 22
                theme: wizardTheme
                padded: true
                visible: !!wizard.sandbox
                IntroStage {
                    Layout.fillWidth: true
                    theme: wizardTheme
                    values: wizard.sandbox ? wizard.sandbox.values : wizard.settings.values
                    shortcutValues: wizard.sandboxShortcuts ? wizard.sandboxShortcuts.values : null
                }
            }
            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 18
                spacing: 12
                OptionTile {
                    objectName: "wizardQuick"
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    theme: wizardTheme
                    iconName: "play"
                    title: "快速設定"
                    lines: [{
                            keys: [],
                            text: wizard.lessons.filter(l => l.level === "basic").length + " 個步驟，約 2 分鐘"
                        }]
                    selected: wizard.mode === "quick"
                    onClicked: wizard.choose("quick")
                }
                OptionTile {
                    objectName: "wizardFull"
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    theme: wizardTheme
                    iconName: "list"
                    title: "完整導覽"
                    lines: [{
                            keys: [],
                            text: wizard.lessons.length + " 個步驟，每個功能"
                        }]
                    selected: wizard.mode === "full"
                    onClicked: wizard.choose("full")
                }
            }
            Item {
                Layout.fillHeight: true
            }
        }
    }

    Component {
        id: presetStep
        ColumnLayout {
            spacing: 20
            StepTitle {
                Layout.fillWidth: true
                title: "你原本習慣哪一種注音輸入？"
                detail: "選最接近的一種，之後每一步都能再改。"
            }
            GridLayout {
                Layout.fillWidth: true
                columns: 2
                columnSpacing: 14
                rowSpacing: 14
                Repeater {
                    model: wizard.presets
                    delegate: OptionTile {
                        required property var modelData
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        Layout.preferredWidth: 1
                        theme: wizardTheme
                        title: modelData.title
                        lines: modelData.lines
                        selected: wizard.preset === modelData.id
                        onClicked: wizard.choosePreset(modelData)
                    }
                }
            }
            RowLayout {
                spacing: 8
                Text {
                    objectName: "presetChanges"
                    text: wizard.changes.length === 0 ? "和目前的設定相同" : "會改變 " + wizard.changes.length + " 項設定"
                    color: wizardTheme.secondary
                    font.pixelSize: wizardTheme.bodySize
                }
                AppButton {
                    id: showChanges
                    visible: wizard.changes.length > 0
                    theme: wizardTheme
                    kind: "plain"
                    compact: true
                    text: "查看"
                    onClicked: presetChanges.show(showChanges)
                }
            }
            Item {
                Layout.fillHeight: true
            }
            InfoPopover {
                id: presetChanges
                theme: wizardTheme
                title: "會改變的設定"
                Repeater {
                    model: wizard.changes
                    delegate: Text {
                        required property var modelData
                        Layout.fillWidth: true
                        text: modelData.title + "：" + modelData.old + " → " + modelData.now
                        color: wizardTheme.text
                        font.pixelSize: wizardTheme.bodySize
                        wrapMode: Text.Wrap
                    }
                }
            }
        }
    }

    Component {
        id: lessonStep
        GridLayout {
            id: lessonView
            readonly property var lesson: wizard.lesson || ({
                    groups: [],
                    prompt: ""
                })
            // A horizontal list is wide and short: the practice field takes the whole width under the choices.
            readonly property bool wide: wizard.sandbox && wizard.sandbox.values.candidateLayout === "horizontal" && lesson.prompt.length > 0
            columns: wide ? 1 : 2
            columnSpacing: 24
            rowSpacing: 18
            ColumnLayout {
                Layout.alignment: Qt.AlignTop
                Layout.preferredWidth: lessonView.wide ? -1 : 236
                Layout.maximumWidth: lessonView.wide ? Number.POSITIVE_INFINITY : 236
                Layout.fillWidth: lessonView.wide
                spacing: 20
                StepTitle {
                    Layout.fillWidth: true
                    title: lessonView.lesson.title || ""
                    detail: lessonView.lesson.detail || ""
                    size: lessonView.wide ? wizardTheme.titleSize + 6 : wizardTheme.titleSize + 4
                }
                GridLayout {
                    Layout.fillWidth: true
                    columns: lessonView.wide ? Math.max(1, lessonView.lesson.groups.length) : 1
                    columnSpacing: 20
                    rowSpacing: 18
                    Repeater {
                        model: lessonView.lesson.groups
                        delegate: StyleGroup {
                            required property string modelData
                            Layout.fillWidth: true
                            Layout.alignment: Qt.AlignTop
                            group: wizard.groups[modelData]
                            across: lessonView.wide && lessonView.lesson.groups.length === 1
                        }
                    }
                }
            }
            // The one live demo: the practice field, or for a lesson without practice its preview.
            Loader {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignTop
                active: lessonView.lesson.prompt.length > 0 && !!wizard.practice.engineSession
                visible: active
                sourceComponent: Component {
                    LiveInput {
                        objectName: "practiceField"
                        engineSession: wizard.practice.engineSession
                        inputController: wizard.practice.inputController
                        settings: wizard.sandbox
                        shortcuts: wizard.sandboxShortcuts
                        theme: wizardTheme
                        prompt: lessonView.lesson.prompt
                        hint: lessonView.lesson.hint || ""
                        initialText: lessonView.lesson.initialText || ""
                        done: !!wizard.completed[lessonView.lesson.id]
                        onOutcomeChanged: {
                            if (!done && lessonView.lesson.check && lessonView.lesson.check(outcome)) {
                                const next = Object.assign({}, wizard.completed);
                                next[lessonView.lesson.id] = true;
                                wizard.completed = next;
                            }
                        }
                    }
                }
            }
            Loader {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignTop
                active: lessonView.lesson.prompt.length === 0
                visible: active
                sourceComponent: lessonView.lesson.id === "correctionLog" ? correctionLogDemo : lookDemo
            }
        }
    }
    Component {
        id: lookDemo
        SectionCard {
            theme: wizardTheme
            padded: true
            CandidatePanel {
                candidates: QtObject {
                    readonly property int pageSize: Math.max(1, wizard.sandbox ? wizard.sandbox.values.candidatesPerPage : 9)
                    readonly property var rows: ["曉明", "小明", "小名", "明", "名", "鳴", "銘", "冥", "茗"].slice(0, pageSize).map(word => ({
                                text: word,
                                annotation: ""
                            }))
                    readonly property int page: 0
                    readonly property int pageCount: 2
                    readonly property int selected: 0
                    readonly property string reading: "ㄒㄧㄠˇ ㄇㄧㄥˊ"
                    readonly property bool single: false
                    readonly property bool corrections: false
                    signal changed
                    function pick(row) {
                    }
                    function turnPage(delta) {
                    }
                    function moveSelection(delta) {
                    }
                    function toggleSingle() {
                    }
                    function toggleCorrections() {
                    }
                }
                preferences: wizard.sandbox ? wizard.sandbox.values : wizard.settings.values
                theme: wizardTheme
                maximumWidth: parent ? parent.width : 400
                Layout.preferredWidth: implicitWidth
                Layout.preferredHeight: implicitHeight
            }
        }
    }
    Component {
        id: correctionLogDemo
        SectionCard {
            theme: wizardTheme
            padded: true
            Repeater {
                model: [["document", "記下打錯又改正的過程：原本的字、改成的字與讀音"], ["drive", "只存在這台電腦，可以隨時匯出或清除"], ["ban", "不記錄密碼欄位，也不記錄 App"]]
                delegate: FactRow {
                    required property var modelData
                    theme: wizardTheme
                    iconName: modelData[0]
                    text: modelData[1]
                }
            }
        }
    }

    Component {
        id: finishStep
        ColumnLayout {
            spacing: 20
            RowLayout {
                spacing: 16
                Rectangle {
                    Layout.preferredWidth: 46
                    Layout.preferredHeight: 46
                    radius: 23
                    color: Qt.rgba(wizardTheme.success.r, wizardTheme.success.g, wizardTheme.success.b, 0.12)
                    border.width: 2.5
                    border.color: wizardTheme.success
                    Icon {
                        anchors.centerIn: parent
                        name: "check"
                        size: 22
                        weight: 3
                        color: wizardTheme.success
                    }
                }
                StepTitle {
                    Layout.fillWidth: true
                    title: "設定好了"
                    detail: wizard.changes.length === 0 ? "沒有要改變的設定。" : "按「套用」之前，還可以改回任何一項。"
                }
            }
            SectionCard {
                theme: wizardTheme
                visible: wizard.changes.length > 0
                Repeater {
                    model: wizard.changes
                    delegate: SettingsRow {
                        id: changeRow
                        required property var modelData
                        required property int index
                        objectName: "change-" + modelData.id
                        theme: wizardTheme
                        separated: index > 0
                        title: modelData.title
                        subtitle: modelData.old + " → " + modelData.now
                        AppButton {
                            visible: !!changeRow.modelData.definition
                            theme: wizardTheme
                            kind: "plain"
                            compact: true
                            text: "修改…"
                            onClicked: {
                                editDialog.definition = changeRow.modelData.definition;
                                editDialog.open();
                            }
                        }
                        AppButton {
                            theme: wizardTheme
                            kind: "plain"
                            compact: true
                            text: "改回"
                            onClicked: wizard.revert(changeRow.modelData)
                        }
                    }
                }
            }
            OptionTile {
                objectName: "keepPracticePhrases"
                visible: wizard.practicePhrases.length > 0
                Layout.fillWidth: true
                theme: wizardTheme
                multi: true
                title: "保留練習時加入的自訂詞（" + wizard.practicePhrases.map(p => p.word).join("、") + "）"
                selected: wizard.keepPracticePhrases
                onClicked: wizard.keepPracticePhrases = !wizard.keepPracticePhrases
            }
            Item {
                Layout.fillHeight: true
            }
            AppDialog {
                id: editDialog
                property var definition: null
                theme: wizardTheme
                title: definition ? definition.title : ""
                contentItem: ColumnLayout {
                    Loader {
                        Layout.fillWidth: true
                        active: !!editDialog.definition && !!wizard.sandbox
                        sourceComponent: Component {
                            SettingRow {
                                theme: wizardTheme
                                definition: editDialog.definition
                                settings: wizard.sandbox
                            }
                        }
                    }
                }
                footer: AppDialogButtons {
                    AppButton {
                        theme: wizardTheme
                        kind: "primary"
                        text: "完成"
                        onClicked: editDialog.close()
                    }
                }
            }
        }
    }

    Component {
        id: keysStep
        ColumnLayout {
            spacing: 20
            StepTitle {
                Layout.fillWidth: true
                title: "你會用到的按鍵"
                detail: "依你的設定產生；之後可以在「介紹」再看一次。"
            }
            GridLayout {
                objectName: "keySummary"
                Layout.fillWidth: true
                columns: width < 520 ? 1 : 2
                columnSpacing: 14
                rowSpacing: 14
                Repeater {
                    model: {
                        wizard.settings.values;
                        wizard.shortcuts.values;
                        return wizard.keyGroups();
                    }
                    delegate: SectionCard {
                        id: keyCard
                        required property var modelData
                        Layout.fillWidth: true
                        Layout.preferredWidth: 1
                        Layout.alignment: Qt.AlignTop
                        theme: wizardTheme
                        padded: true
                        RowLayout {
                            spacing: 8
                            Icon {
                                name: keyCard.modelData.icon
                                size: 16
                                color: wizardTheme.secondary
                            }
                            Text {
                                text: keyCard.modelData.title
                                color: wizardTheme.text
                                font.pixelSize: wizardTheme.bodySize
                                font.weight: Font.DemiBold
                            }
                        }
                        Repeater {
                            model: keyCard.modelData.items.slice(0, 4)
                            delegate: RowLayout {
                                id: keyItem
                                required property var modelData
                                Layout.fillWidth: true
                                spacing: 10
                                Flow {
                                    Layout.preferredWidth: 124
                                    spacing: 4
                                    Repeater {
                                        model: keyItem.modelData.keys
                                        delegate: KeyCap {
                                            required property string modelData
                                            theme: wizardTheme
                                            text: modelData
                                        }
                                    }
                                }
                                Text {
                                    Layout.fillWidth: true
                                    text: keyItem.modelData.action
                                    color: wizardTheme.text
                                    font.pixelSize: wizardTheme.detailSize
                                    wrapMode: Text.Wrap
                                }
                            }
                        }
                    }
                }
            }
            Item {
                Layout.fillHeight: true
            }
        }
    }
}
