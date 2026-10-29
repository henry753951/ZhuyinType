pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// 快捷鍵, as macOS KeysPane: every key grouped by feature, in the words of what it does.  Rebindable commands have the
// key field right here (two chips, then 「+N」); a feature that is off is dimmed with 「已關閉」 and its row opens the
// feature's page; keys that cannot change wait under 「固定的按鍵」 at the bottom.  Whether a feature is on comes from
// Settings only, so this page and the feature pages always agree.
ColumnLayout {
    id: page
    required property var settings
    required property Theme theme
    property string pageId: "keys"
    property var shortcuts: null
    property var appInfo: ({})
    property bool reduceMotion: theme.reduceMotion
    readonly property bool motionEnabled: !reduceMotion && theme.motionEnabled
    signal navigateRequested(string pageId, string title, string row)
    readonly property bool mac: Qt.platform.os === "osx"
    // Platform words (DESIGN_SYSTEM.md): ⌃ is Ctrl, ⌥ is Alt elsewhere; ⇧ stays a glyph on keycaps.
    readonly property string controlCap: mac ? "⌃" : "Ctrl+"
    readonly property string optionCap: mac ? "⌥" : "Alt+"
    readonly property string forwardDeleteCap: mac ? "⌦" : "Del"
    readonly property var values: settings.values || ({})
    readonly property string query: search.text.trim().toLowerCase()
    property bool showsFixed: false
    readonly property var pageTitles: ({
            typing: "打字",
            choosing: "選字",
            appearance: "視窗與提示",
            punctuation: "標點與符號"
        })
    readonly property var featureIds: ["typeZhuyin", "zhuyinRowClick", "singleCharacterList", "correctionsList", "selectPhrase",
        "pageKeys", "deleteWord", "controlPunctuation", "optionSymbols", "capsLockEnglish", "deleteAfterCommit"]
    // Where each feature with keys is set (macOS KeyFeature.place).
    readonly property var featurePlaces: ({
            typeZhuyin: ["choosing", "typeZhuyin"],
            zhuyinRowClick: ["appearance", "zhuyinRow"],
            singleCharacterList: ["appearance", "single"],
            correctionsList: ["choosing", "corrections"],
            selectPhrase: ["choosing", "selectPhrase"],
            pageKeys: ["appearance", "pageKeys"],
            deleteWord: ["typing", "deleteWord"],
            controlPunctuation: ["punctuation", "controlPunctuation"],
            optionSymbols: ["punctuation", "optionSymbols"],
            capsLockEnglish: ["typing", "capsLock"],
            deleteAfterCommit: ["choosing", "deleteAfterCommit"]
        })

    function on(feature) {
        return !feature || values["feature." + feature] !== false;
    }
    function command(id) {
        return shortcuts && shortcuts.commands ? shortcuts.commands.find(item => item.id === id) : null;
    }
    // What another page's setting does to a feature (macOS Preferences.availability), or null.
    function availability(feature) {
        if (feature === "zhuyinRowClick" && values.showsZhuyinHint === false)
            return { note: "「顯示注音」已關閉，候選字視窗沒有注音列", page: "appearance", row: "showsZhuyin", title: "視窗與提示" };
        if (feature === "correctionsList" && values.typoTolerance === "off" && !["zZh", "cCh", "sSh", "enEng", "anAng"].some(key => values["fuzzy." + key]))
            return { note: "「按錯鍵修正」關閉且沒有開啟模糊音：這個鍵開啟一般清單", page: "choosing", row: "typo", title: "選字" };
        return null;
    }
    readonly property string selectionText: {
        const perPage = values.candidatesPerPage || 9;
        const set = values.selectionKeys || "123456789";
        return set === "123456789" ? "1–" + perPage : set.slice(0, perPage).toUpperCase().split("").join(" ");
    }
    readonly property string spaceSubtitle: ({
            candidates: "打完注音後開啟候選字視窗",
            insert: "打完注音後在組字中輸入空白，不送出",
            commit: "打完注音後送出組字並輸入空白"
        })[values.spaceAction || "commit"] + "；打注音時是一聲"
    readonly property bool vertical: values.candidateLayout !== "horizontal"
    readonly property string shiftTapKey: values.shiftToggle === "left" ? "左 Shift" : values.shiftToggle === "right" ? "右 Shift" : "Shift"

    // The rows, in macOS KeysPane order.  keys: fixed keycaps (one chip per group); command: a rebindable command;
    // feature: the switch that turns it off; home: [page, row] the › opens.
    readonly property var sections: [
        {
            title: "組字",
            rows: [
                { id: "zhuyin", title: "拼注音", subtitle: "固定：注音鍵的位置依標準注音鍵盤", keys: ["注音鍵"] },
                { id: "tones", title: "聲調：二聲、三聲、四聲、輕聲", subtitle: "固定", keys: ["ˊ 6ˇ 3ˋ 4˙ 7"] },
                { id: "space", title: "空白", subtitle: spaceSubtitle, keys: ["空白"], home: ["typing", "space"] },
                { id: "commit", title: "送出", subtitle: "固定", keys: ["Return"] },
                { id: "newline", title: "送出並換行", command: "commitWithNewline" },
                { id: "backspace", title: "刪掉注音符號，或游標前的字", subtitle: "固定", keys: ["⌫"] },
                { id: "deleteWord", title: "刪掉一個詞", subtitle: "有未完成的注音時清除注音", command: "deleteWord", feature: "deleteWord" },
                { id: "escape", title: values.escape === "reading" ? "清除注音" : "清除注音，再按一次清除組字", keys: ["Esc"], home: ["typing", "escape"] },
                { id: "cursor", title: "在組字中移動游標", subtitle: "固定", keys: ["←→", "HomeEnd"] }
            ]
        },
        {
            title: "候選字清單",
            rows: [
                { id: "open", title: "開啟候選字視窗", subtitle: "固定", keys: values.spaceAction === "candidates" ? ["↓", "空白"] : ["↓"] },
                { id: "pick", title: "選字", subtitle: "目前每頁 " + (values.candidatesPerPage || 9) + " 個；Return 選取反白的字", keys: [selectionText], home: ["appearance", "selection"] },
                { id: "move", title: "移動反白", subtitle: vertical ? "直式清單" : "橫式清單；↑ ↓ 翻頁", keys: [vertical ? "↑↓" : "←→"], home: ["appearance", "layout"] },
                { id: "nextPage", title: "下一頁", subtitle: "空白也翻到下一頁", command: "nextPage", feature: "pageKeys" },
                { id: "previousPage", title: "上一頁", command: "previousPage", feature: "pageKeys" },
                { id: "previousCharacter", title: "改前一個字", subtitle: vertical ? "清單開著時；直式清單也可以按 ←" : "清單開著時", command: "previousCharacter" },
                { id: "nextCharacter", title: "改後一個字", subtitle: vertical ? "清單開著時；直式清單也可以按 →" : "清單開著時", command: "nextCharacter" },
                { id: "single", title: "只列一個字", subtitle: "再按一次還原", command: "singleCharacterList", feature: "singleCharacterList" },
                { id: "corrections", title: "列出修正的字", subtitle: "按錯修正與模糊音的字排在前面；再按一次還原", command: "correctionsList", feature: "correctionsList" },
                { id: "zhuyinRow", title: "點注音列打出注音", subtitle: "候選字視窗上方的注音", keys: ["點一下"], feature: "zhuyinRowClick" },
                { id: "close", title: "關閉候選字視窗", subtitle: "固定", keys: ["Esc"] }
            ]
        },
        {
            title: "改字與選取",
            rows: [
                { id: "typeZhuyin", title: "打出注音本身", subtitle: "緊接著再按一次或按 Esc 恢復", command: "typeZhuyin", feature: "typeZhuyin" },
                { id: "selectLeft", title: "往左選取", subtitle: "再按 Return 加入自訂詞", command: "selectLeft", feature: "selectPhrase" },
                { id: "selectRight", title: "往右選取", subtitle: "再按 Return 加入自訂詞", command: "selectRight", feature: "selectPhrase" },
                { id: "deleteAfterCommit", title: "送出後刪掉重打", subtitle: "送出後 30 秒內刪掉剛送出的字再打，算作改字並接著前文選字", keys: ["⌫"], feature: "deleteAfterCommit" }
            ]
        },
        {
            title: "標點與符號",
            rows: [
                { id: "brackets", title: "「 」 、", subtitle: "固定", keys: ["[]'"] },
                { id: "shiftSymbols", title: values.punctuationKeys === "mac" ? "中文標點：，。？！：『』——" : "鍵上的符號：< > _ ? !", keys: ["⇧符號鍵"], home: ["typing", "punctuationKeys"] },
                { id: "controlPunctuation", title: "中文標點：，。、？！：；『』", subtitle: "兩種標點按法都能用", keys: [controlCap + "符號鍵"], feature: "controlPunctuation" },
                { id: "optionSymbols", title: "鍵上的符號：. / ; < >", subtitle: "兩種標點按法都能用", keys: [optionCap + "數字或符號鍵"], feature: "optionSymbols" },
                { id: "punctuationList", title: "列出同類的標點", subtitle: "在標點上按", keys: ["↓"] }
            ]
        },
        {
            title: "中英切換",
            rows: [
                { id: "shiftTap", title: "單按 Shift 切換中英", subtitle: "在可以打字的地方", keys: [shiftTapKey], shiftTap: true, home: ["typing", "shiftTap"] },
                { id: "capsLock", title: "Caps Lock 打英文", subtitle: "開著時打英文，正在組的字先送出", keys: ["Caps Lock"], feature: "capsLockEnglish" }
            ]
        },
        {
            title: "其他",
            rows: [
                { id: "shiftSpace", title: "在組字中放一個空白", subtitle: "固定", keys: ["⇧空白"] },
                { id: "forwardDelete", title: "刪掉游標後的字", subtitle: "固定", keys: [forwardDeleteCap] }
            ]
        }
    ]
    // A key that cannot change and has no switch.
    function isFixed(row) {
        return !row.command && !row.feature && !row.shiftTap;
    }
    function home(row) {
        if (row.home)
            return row.home;
        if (row.command) {
            const entry = command(row.command);
            return entry ? [entry.page, entry.row] : null;
        }
        return row.feature ? featurePlaces[row.feature] : null;
    }
    function keysOf(row) {
        if (!row.command)
            return row.keys;
        return shortcuts ? (shortcuts.values[row.command] || []).map(binding => binding.label) : [];
    }
    function matches(row) {
        if (!query)
            return true;
        // macOS: the title, the subtitle and the keys (a command's first key).
        const keys = row.command ? keysOf(row).slice(0, 1) : row.shiftTap ? ["shift"] : row.keys;
        return [row.title, row.subtitle || "", keys.join(" ")].join(" ").toLowerCase().includes(query);
    }
    readonly property var matchingSections: sections.map(section => ({
                title: section.title,
                rows: section.rows.filter(row => !isFixed(row) && matches(row))
            })).filter(section => section.rows.length > 0)
    readonly property var matchingFixed: sections.reduce((all, section) => all.concat(section.rows.filter(row => isFixed(row) && matches(row))), [])
    readonly property bool changed: (!!shortcuts && shortcuts.customized) || featureIds.some(id => values["feature." + id] === false)
        || (values.shiftToggle || "both") !== "both"
    readonly property var conflicts: shortcuts && shortcuts.conflicts ? shortcuts.conflicts : []

    function open(place, title) {
        if (place)
            navigateRequested(place[0], title || pageTitles[place[0]] || "", place[1] || "");
    }
    function resetAll() {
        if (shortcuts)
            shortcuts.reset();
        const next = { shiftToggle: "both" };
        for (const id of featureIds)
            next["feature." + id] = true;
        settings.assign(next);
    }

    spacing: theme.sectionSpacing
    // The macOS window searches from its toolbar; here the field heads the page.
    AppTextField {
        id: search
        Layout.fillWidth: true
        theme: page.theme
        search: true
        placeholderText: "搜尋"
        Accessible.name: "搜尋按鍵"
        Keys.onEscapePressed: clear()
    }
    SectionCard {
        theme: page.theme
        title: "按鍵衝突"
        visible: page.conflicts.length > 0
        Repeater {
            model: page.conflicts
            delegate: SettingsRow {
                id: conflictRow
                required property var modelData
                required property int index
                objectName: "keyConflict"
                theme: page.theme
                separated: index > 0
                title: "「" + modelData.commandLabel + "」與「" + modelData.otherLabel + "」"
                subtitle: "用同一個按鍵；同一個按鍵只會執行其中一個功能"
                accessory: "chevron"
                clickable: true
                onActivated: {
                    const entry = page.command(conflictRow.modelData.command);
                    page.open([entry.page, entry.row]);
                }
                KeyCap {
                    theme: page.theme
                    text: conflictRow.modelData.label
                    tint: page.theme.danger
                }
            }
        }
    }
    SectionCard {
        theme: page.theme
        visible: page.matchingSections.length === 0 && page.matchingFixed.length === 0
        Text {
            objectName: "keysEmpty"
            Layout.fillWidth: true
            Layout.minimumHeight: 60
            text: "沒有符合「" + search.text.trim() + "」的項目"
            color: page.theme.secondary
            font.pixelSize: page.theme.bodySize
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
    }
    Repeater {
        model: page.matchingSections
        delegate: SectionCard {
            id: section
            required property var modelData
            theme: page.theme
            title: modelData.title
            Repeater {
                model: section.modelData.rows
                delegate: KeyRow {
                    required property var modelData
                    required property int index
                    entry: modelData
                    separated: index > 0
                }
            }
        }
    }
    ColumnLayout {
        Layout.fillWidth: true
        visible: page.matchingFixed.length > 0
        spacing: 0
        SectionCard {
            theme: page.theme
            SettingsRow {
                id: fixedToggle
                objectName: "fixedKeysToggle"
                readonly property bool open: page.showsFixed || page.query.length > 0
                theme: page.theme
                title: "固定的按鍵"
                subtitle: "打注音、聲調、送出與移動游標的按鍵，無法更改"
                clickable: true
                Accessible.name: open ? "收起固定的按鍵" : "顯示固定的按鍵"
                onActivated: page.showsFixed = !page.showsFixed
                Icon {
                    name: "chevronRight"
                    size: 14
                    weight: 2
                    color: page.theme.tertiary
                    rotation: fixedToggle.open ? 90 : 0
                    Behavior on rotation {
                        NumberAnimation {
                            duration: page.motionEnabled ? 200 : 0
                            easing.type: Easing.InOutQuad
                        }
                    }
                }
            }
            Repeater {
                model: fixedToggle.open ? page.matchingFixed : []
                delegate: KeyRow {
                    required property var modelData
                    entry: modelData
                    separated: true
                }
            }
        }
        // The section footer: where keys are set, and 全部還原….
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: page.theme.cardInset
            Layout.rightMargin: page.theme.cardInset
            Layout.topMargin: page.theme.headerGap
            spacing: 12
            Text {
                Layout.fillWidth: true
                text: "點一下任何一列，到功能的頁面設定按鍵。"
                color: page.theme.secondary
                font.pixelSize: page.theme.detailSize
                wrapMode: Text.Wrap
            }
            Text {
                id: resetLink
                objectName: "resetAllKeys"
                enabled: page.changed
                text: "全部還原…"
                color: !enabled ? page.theme.tertiary : resetHover.hovered ? page.theme.accentHover : page.theme.accent
                font.pixelSize: page.theme.detailSize
                Accessible.role: Accessible.Link
                Accessible.name: text
                HoverHandler {
                    id: resetHover
                    cursorShape: resetLink.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                }
                TapHandler {
                    onTapped: resetDialog.open()
                }
            }
        }
    }
    Text {
        Layout.fillWidth: true
        visible: !!page.shortcuts && !!page.shortcuts.error && page.shortcuts.error.indexOf("無法") === 0
        text: page.shortcuts ? page.shortcuts.error : ""
        color: page.theme.danger
        wrapMode: Text.Wrap
        font.pixelSize: page.theme.detailSize
    }
    property QtObject dialogs: QtObject {
        property Dialog reset: AppDialog {
            id: resetDialog
            objectName: "resetAllDialog"
            theme: page.theme
            title: "要還原所有按鍵與功能嗎？"
            contentItem: Text {
                text: "所有按鍵回到預設值，所有功能開啟，單按左右 Shift 切換中英。"
                wrapMode: Text.Wrap
                color: page.theme.secondary
                font.pixelSize: page.theme.bodySize
            }
            footer: AppDialogButtons {
                AppButton {
                    theme: page.theme
                    text: "取消"
                    DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                }
                AppButton {
                    theme: page.theme
                    kind: "danger"
                    text: "全部還原"
                    DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                }
            }
            onAccepted: page.resetAll()
        }
    }

    // One row (macOS KeyItemRow).  A command that is on: its key field, with a › to the feature's page.  Otherwise the
    // keys (or 「已關閉」, dimmed) and, when the row has a page, the whole row opens it.
    component KeyRow: SettingsRow {
        id: keyRow
        required property var entry
        readonly property bool isOn: entry.shiftTap ? page.values.shiftSwitch !== false : page.on(entry.feature)
        readonly property var available: entry.feature ? page.availability(entry.feature) : null
        readonly property var place: page.home(entry)
        readonly property bool editable: !!entry.command && isOn && !!page.shortcuts
        objectName: "keyRow_" + entry.id
        theme: page.theme
        anchor: "row-" + entry.id
        title: entry.title
        subtitle: available ? available.note : (entry.subtitle || "")
        dimmed: !isOn
        marked: editable && page.shortcuts.isCustomized(entry.command) && !!page.shortcuts.values
        clickable: !editable && !!place
        accessory: !editable && !!place ? "chevron" : "none"
        stacked: editable ? field.implicitWidth > width * 0.55 : false
        Accessible.description: place ? "開啟「" + (page.pageTitles[place[0]] || "") + "」" : "固定的按鍵"
        onActivated: page.open(keyRow.place)
        ShortcutRecorder {
            id: field
            visible: keyRow.editable
            Layout.preferredWidth: Math.min(implicitWidth, keyRow.width * 0.6)
            shortcuts: page.shortcuts
            theme: page.theme
            command: keyRow.entry.command || ""
            maxVisible: 2
            rowHovered: keyRow.hovered
            disabled: !!keyRow.available
            onNavigateRequested: (pageId, title, row) => page.navigateRequested(pageId, title, row)
        }
        IconButton {
            visible: keyRow.editable && !!keyRow.place
            theme: page.theme
            iconName: "chevronRight"
            iconSize: 14
            implicitWidth: 16
            implicitHeight: 22
            Accessible.name: "前往「" + (keyRow.place ? page.pageTitles[keyRow.place[0]] : "") + "」"
            onClicked: page.open(keyRow.place)
        }
        Text {
            visible: !keyRow.editable && !keyRow.isOn
            text: "已關閉"
            color: page.theme.secondary
            font.pixelSize: page.theme.bodySize
        }
        Row {
            visible: !keyRow.editable && keyRow.isOn
            spacing: 6
            opacity: keyRow.available ? 0.45 : 1
            Accessible.name: page.keysOf(keyRow.entry).join(" 或 ")
            Repeater {
                model: keyRow.editable ? [] : page.keysOf(keyRow.entry)
                delegate: KeyCap {
                    required property string modelData
                    theme: page.theme
                    text: modelData
                }
            }
        }
    }
}
