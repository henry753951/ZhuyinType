pragma ComponentBehavior: Bound

import QtCore
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Dialogs

// 字詞 (with 記住的字詞 and 自訂詞), 進階 (with the App list) and 快捷鍵, as on macOS (SettingsPages.swift WordsPane,
// WordListPage, AdvancedPane, AppsPage; ListPage.swift; ListControllers.swift).
ColumnLayout {
    id: page
    required property var settings
    required property Theme theme
    property var engineSession: null
    property var platformSettings: null
    /// KEYS's CorrectionLog { count, location, recordsContext, error; showLog(), exportLog(url), clearLog() }.
    property var correctionLog: null
    property var shortcuts: null
    property var scrollViewport: null
    readonly property bool personalReady: engineSession !== null && engineSession.ready
    readonly property var values: settings.values || ({})
    property string operationError: ""
    signal operationFailed(string message)
    signal confirmationRequested(string operation)
    property string pageId: "words"
    property var appInfo: ({})
    property bool reduceMotion: theme.reduceMotion
    readonly property bool motionEnabled: !reduceMotion && theme.motionEnabled
    signal navigateRequested(string pageId, string title, string row)
    spacing: 16

    /// Calls `method` on `target`; on failure keeps the message in `operationError` and returns false.
    function invoke(target, method, args) {
        operationError = "";
        if (!target || typeof target[method] !== "function") {
            operationError = "操作尚未連接。";
        } else {
            try {
                const result = target[method].apply(target, args);
                if (method === "importPersonal" ? typeof result === "number" && result >= 0 : result === true)
                    return true;
                operationError = target.error || "操作未完成，請再試一次。";
            } catch (error) {
                operationError = String(error);
            }
        }
        operationFailed(operationError);
        return false;
    }
    /// A command's keys in prose ("Shift＋←"), or "" when its feature is off or it has no key (CurrentKeys.text).
    function keyText(command) {
        if (!shortcuts || !shortcuts.values)
            return "";
        const known = (shortcuts.commands || []).find(item => item.id === command);
        if (known && known.feature && values[known.feature] === false)
            return "";
        const bound = shortcuts.values[command] || [];
        return bound.length > 0 ? String(bound[0].label).split(" ＋ ").join("＋") : "";
    }
    readonly property string selectLeft: keyText("selectLeft")
    readonly property string selectRight: keyText("selectRight")
    readonly property string fileManager: Qt.platform.os === "windows" ? "檔案總管" : Qt.platform.os === "osx" ? " Finder " : "檔案管理員"

    Loader {
        Layout.fillWidth: true
        sourceComponent: page.pageId === "keys" ? keysPage : page.pageId === "advanced" ? advancedPage : page.pageId === "words" ? wordsPage : listPage
    }

    // MARK: 字詞

    Component {
        id: wordsPage
        ColumnLayout {
            spacing: page.theme.sectionSpacing
            SectionCard {
                theme: page.theme
                SettingRow {
                    id: learningRow
                    definition: page.settings.definitions.find(item => item.key === "learningEnabled") || ({
                            key: "learningEnabled",
                            title: "記住選過的字詞",
                            kind: "toggle"
                        })
                    settings: page.settings
                    theme: page.theme
                    info: true
                    anchor: "learningEnabled"
                    onInfoClicked: learnedInfo.show(learningRow)
                }
                NavigationRow {
                    theme: page.theme
                    separated: true
                    anchor: "learnedWords"
                    title: "記住的字詞"
                    detail: "檢視、移除、匯出或匯入"
                    value: page.personalReady ? page.engineSession.learnedEntries.length.toLocaleString(Qt.locale(), "f", 0) + " 個" : ""
                    onActivated: page.navigateRequested("learnedWords", "記住的字詞", "")
                }
            }
            SectionCard {
                theme: page.theme
                NavigationRow {
                    theme: page.theme
                    anchor: "phrases"
                    title: "自訂詞"
                    detail: page.selectLeft ? "選字時優先列出；打字時也能選取後加入" : "選字時優先列出"
                    value: page.personalReady ? page.engineSession.userPhrases.length.toLocaleString(Qt.locale(), "f", 0) + " 個" : ""
                    onActivated: page.navigateRequested("phrases", "自訂詞", "")
                }
            }
        }
    }

    // MARK: 記住的字詞, 自訂詞, App

    Component {
        id: listPage
        ColumnLayout {
            id: list
            objectName: "personalList"
            readonly property bool learned: page.pageId === "learnedWords"
            readonly property bool apps: page.pageId === "apps"
            readonly property bool connected: apps ? page.platformSettings !== null : page.personalReady
            readonly property string noun: apps ? "App" : learned ? "字詞" : "自訂詞"
            readonly property var sorts: apps ? ["名稱"] : learned ? ["常用程度", "注音", "字詞"] : ["注音", "字詞"]
            readonly property bool canAdd: !learned
            readonly property bool canEdit: !learned && !apps
            readonly property int rowHeight: 40
            property var selection: []
            readonly property var entries: {
                if (!connected)
                    return [];
                if (apps)
                    return page.platformSettings.apps.map(id => ({
                                word: typeof page.platformSettings.appName === "function" ? page.platformSettings.appName(id) : id,
                                reading: id,
                                id: id,
                                excluded: false
                            }));
                if (!learned)
                    return page.engineSession.userPhrases;
                return page.engineSession.learnedEntries.map(row => Object.assign({}, row, {
                        excluded: false
                    })).concat(page.engineSession.excludedEntries.map(row => Object.assign({}, row, {
                        excluded: true
                    })));
            }
            function key(row) {
                return JSON.stringify([row.word, row.reading, !!row.excluded]);
            }
            // How much a learned word is used, 1–5, on a log scale against the most used (LearnedListController).
            readonly property real topWeight: Math.log1p(entries.reduce((maximum, row) => Math.max(maximum, row.excluded ? 0 : row.count || 0), 1))
            function level(row) {
                if (!learned || row.excluded)
                    return 0;
                return Math.max(1, Math.ceil(Math.log1p(Math.max(0, row.count || 0)) / Math.max(topWeight, 0.0001) * 5));
            }
            readonly property string query: search.text.trim().toLocaleLowerCase()
            readonly property var shown: {
                const rows = entries.filter(row => (row.word + " " + row.reading + (row.excluded ? " 不記住" : "")).toLocaleLowerCase().includes(query));
                const sort = sorts[Math.max(0, sorting.currentIndex)];
                return rows.sort((a, b) => {
                    if (sort === "常用程度") {
                        const ca = a.excluded ? -1 : a.count || 0, cb = b.excluded ? -1 : b.count || 0;
                        if (ca !== cb)
                            return cb - ca;
                    }
                    const field = sort === "注音" ? "reading" : "word";
                    return String(a[field]).localeCompare(String(b[field])) || list.key(a).localeCompare(list.key(b));
                });
            }
            readonly property var selected: entries.filter(row => selection.includes(key(row)))
            onEntriesChanged: selection = selection.filter(id => entries.some(row => key(row) === id))

            function select(row, modifiers) {
                const id = key(row);
                const toggle = (modifiers & Qt.ControlModifier) || (modifiers & Qt.MetaModifier);
                if (toggle)
                    selection = selection.includes(id) ? selection.filter(value => value !== id) : selection.concat([id]);
                else
                    selection = selection.includes(id) && selection.length === 1 ? [] : [id];
                rows.forceActiveFocus();
            }
            /// Removing a learned word forgets it; removing an excluded one lets it be learned again (no confirmation,
            /// as on macOS).
            function remove(targets) {
                for (const row of targets) {
                    if (!entries.some(entry => key(entry) === key(row)))
                        continue;
                    const method = apps ? "removeApp" : !learned ? "removePhrase" : row.excluded ? "includeLearned" : "forgetLearned";
                    if (!page.invoke(apps ? page.platformSettings : page.engineSession, method, apps ? [row.id] : [row.word, row.reading]))
                        break;
                }
            }
            function exclude(targets) {
                for (const row of targets.filter(row => !row.excluded))
                    if (!page.invoke(page.engineSession, "excludeLearned", [row.word, row.reading]))
                        break;
            }
            /// The rows a context menu or ⌫ acts on: the selection when the row is in it, else the row.
            function targets(row) {
                return row && !selection.includes(key(row)) ? [row] : selected;
            }
            function openAdd(row) {
                if (apps) {
                    addMenu.popup(addButton, 0, addButton.height + 4);
                    return;
                }
                page.operationError = "";
                addDialog.editing = row || null;
                wordField.text = row ? row.word : "";
                readingField.text = row ? row.reading : "";
                addDialog.open();
                wordField.forceActiveFocus();
            }
            property var menuTargets: []
            spacing: page.theme.headerGap + 3

            // The toolbar: search, count, order, add or more (ListSection.toolbar).
            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: list.width < 420 ? 0 : page.theme.cardInset
                Layout.rightMargin: list.width < 420 ? 0 : page.theme.cardInset
                spacing: 10
                AppTextField {
                    id: search
                    objectName: "personalSearch"
                    theme: page.theme
                    search: true
                    Layout.fillWidth: true
                    Layout.maximumWidth: 260
                    placeholderText: "搜尋" + list.noun
                    Accessible.name: placeholderText
                    Keys.onEscapePressed: clear()
                }
                Item {
                    Layout.fillWidth: true
                }
                Text {
                    objectName: "personalCount"
                    text: list.query ? list.shown.length + " / " + list.entries.length + " 個" : "共 " + list.entries.length + " 個"
                    color: page.theme.secondary
                    font.pixelSize: page.theme.detailSize
                    font.features: ({ "tnum": 1 })
                }
                AppComboBox {
                    id: sorting
                    objectName: "personalSort"
                    theme: page.theme
                    small: true
                    visible: list.sorts.length > 1
                    model: list.sorts.map(sort => "依" + sort)
                    Accessible.name: "排序"
                }
                IconButton {
                    id: addButton
                    objectName: "personalAdd"
                    visible: list.canAdd
                    enabled: list.connected
                    theme: page.theme
                    iconName: "plus"
                    Accessible.name: "加入" + list.noun
                    onClicked: list.openAdd(null)
                }
                IconButton {
                    id: moreButton
                    objectName: "personalMore"
                    visible: list.learned
                    enabled: list.connected
                    theme: page.theme
                    iconName: "more"
                    Accessible.name: "更多"
                    onClicked: moreMenu.popup(moreButton, moreButton.width - moreMenu.width, moreButton.height + 4)
                }
            }
            Text {
                objectName: "personalError"
                Layout.fillWidth: true
                Layout.leftMargin: page.theme.cardInset
                text: addDialog.opened ? "" : page.operationError || (list.apps ? (page.platformSettings ? page.platformSettings.error : "") : (page.engineSession ? page.engineSession.error : ""))
                visible: text.length > 0
                color: page.theme.danger
                font.pixelSize: page.theme.detailSize
                wrapMode: Text.Wrap
                Accessible.name: "操作錯誤：" + text
            }
            // The rows on one card. Long lists stay smooth: only the rows inside the window's scroll view are made.
            Rectangle {
                id: listCard
                Layout.fillWidth: true
                Layout.preferredHeight: list.shown.length === 0 ? empty.implicitHeight + 72 : list.shown.length * list.rowHeight
                radius: page.theme.cardRadius
                color: page.theme.card
                clip: true
                readonly property real contentStart: page.scrollViewport ? mapToItem(page.scrollViewport.contentItem, 0, 0).y : 0
                // Without an outer scroll view (a test, a preview) the list scrolls by itself, a window tall.
                readonly property real viewportHeight: page.scrollViewport ? page.scrollViewport.height : Math.min(height, 600)
                readonly property real offset: page.scrollViewport ? Math.max(0, Math.min(Math.max(0, list.shown.length * list.rowHeight - viewportHeight), page.scrollViewport.contentY - contentStart)) : 0
                EmptyState {
                    id: empty
                    visible: list.shown.length === 0
                    anchors.centerIn: parent
                    width: parent.width - page.theme.cardInset * 2
                    theme: page.theme
                    iconName: list.query ? "search" : "tray"
                    title: !list.connected ? (list.apps ? "無法讀取 App 清單" : "正在載入詞庫…") : list.query ? "沒有符合「" + search.text.trim() + "」的" + list.noun : list.apps ? "沒有 App" : "還沒有" + (list.learned ? "記住的字詞" : "自訂詞")
                    text: !list.connected ? "" : list.query ? "試試較短的字，或用注音搜尋。" : list.apps ? "每個 App 都直接在文字中顯示組字。少數 App 需要把組字改放在候選字視窗上方；按 ＋ 加入。" : list.learned ? "從候選字清單選字後，ZhuyinType 會記住你的選擇，下次排在前面。" : "按 ＋ 加入，或打字時選取字詞再按 Return。自訂詞在選字時優先列出。"
                }
                ListView {
                    id: rows
                    objectName: "personalRows"
                    visible: list.shown.length > 0
                    y: listCard.offset
                    width: parent.width
                    height: Math.min(list.shown.length * list.rowHeight, listCard.viewportHeight)
                    contentY: listCard.offset
                    clip: true
                    model: list.shown
                    interactive: !page.scrollViewport
                    reuseItems: true
                    // Lay out at the top first: a first layout far down would make every row above it.
                    Component.onCompleted: forceLayout()
                    activeFocusOnTab: true
                    Accessible.role: Accessible.List
                    Accessible.name: list.noun
                    delegate: SelectableRow {
                        id: wordRow
                        required property var modelData
                        required property int index
                        theme: page.theme
                        width: rows.width
                        height: list.rowHeight
                        first: index === 0
                        title: modelData.word
                        detail: modelData.reading
                        meta: modelData.excluded ? "不記住" : ""
                        level: list.level(modelData)
                        marked: !!modelData.excluded
                        canEdit: list.canEdit
                        selected: list.selection.includes(list.key(modelData))
                        onClicked: modifiers => list.select(modelData, modifiers)
                        onEditClicked: list.openAdd(modelData)
                        onRemoveClicked: list.remove([modelData])
                        onMenuRequested: {
                            list.menuTargets = list.targets(modelData);
                            rowMenu.popup();
                        }
                    }
                    Keys.onDeletePressed: if (list.selected.length)
                        list.remove(list.selected)
                    Keys.onPressed: event => {
                        if (event.key === Qt.Key_Backspace && list.selected.length) {
                            list.remove(list.selected);
                            event.accepted = true;
                        }
                    }
                    Keys.onEscapePressed: event => {
                        event.accepted = list.selection.length > 0;
                        list.selection = [];
                    }
                }
            }
            Caption {
                theme: page.theme
                Layout.leftMargin: page.theme.cardInset
                Layout.rightMargin: page.theme.cardInset
                Layout.topMargin: 2
                text: list.apps ? "清單中的 App，組字顯示在候選字視窗上方。" : list.learned ? "選過的字詞下次排在前面；設為「不記住」的字詞之後不再學。" : page.selectLeft && page.selectRight ? "自訂詞在選字時優先列出；打字時用 " + page.selectLeft + " " + page.selectRight + " 選取字詞，再按 Return 也能加入。" : "自訂詞在選字時優先列出。"
            }

            AppMenu {
                id: rowMenu
                theme: page.theme
                Action {
                    text: "編輯…"
                    enabled: list.canEdit && list.menuTargets.length === 1
                    onTriggered: list.openAdd(list.menuTargets[0])
                }
                Action {
                    text: "不再記住"
                    enabled: list.learned && list.menuTargets.some(row => !row.excluded)
                    onTriggered: list.exclude(list.menuTargets)
                }
                MenuSeparator {}
                Action {
                    text: list.menuTargets.length > 1 ? "刪除 " + list.menuTargets.length + " 個" : "刪除"
                    onTriggered: list.remove(list.menuTargets)
                }
            }
            AppMenu {
                id: moreMenu
                theme: page.theme
                Action {
                    text: "不再記住所選字詞"
                    enabled: list.selected.some(row => !row.excluded)
                    onTriggered: list.exclude(list.selected)
                }
                Action {
                    text: "匯出…"
                    onTriggered: exportFile.open()
                }
                Action {
                    text: "匯入…"
                    onTriggered: importFile.open()
                }
                MenuSeparator {}
                Action {
                    text: "全部清除…"
                    onTriggered: {
                        page.confirmationRequested("clearLearned");
                        clearDialog.open();
                    }
                }
            }
            // ＋ on the App list: the apps with a window open, then 其他… to type a name (AppListController).
            AppMenu {
                id: addMenu
                theme: page.theme
                property var running: []
                onAboutToShow: running = page.platformSettings && typeof page.platformSettings.runningApps === "function" ? page.platformSettings.runningApps() : []
                Instantiator {
                    model: addMenu.running
                    delegate: Action {
                        required property var modelData
                        text: modelData.name
                        onTriggered: page.invoke(page.platformSettings, "addApp", [modelData.id])
                    }
                    onObjectAdded: (index, object) => addMenu.insertAction(index, object)
                    onObjectRemoved: (index, object) => addMenu.removeAction(object)
                }
                MenuSeparator {
                    visible: addMenu.running.length > 0
                    height: visible ? implicitHeight : 0
                }
                Action {
                    text: "選擇其他 App…"
                    onTriggered: {
                        page.operationError = "";
                        wordField.text = "";
                        addDialog.editing = null;
                        addDialog.open();
                        wordField.forceActiveFocus();
                    }
                }
            }

            property Dialog addSheet: AppDialog {
                id: addDialog
                property var editing: null
                theme: page.theme
                title: list.apps ? "加入 App" : editing ? "編輯自訂詞" : "加入自訂詞"
                readonly property var suggestions: !list.apps && page.engineSession && typeof page.engineSession.readings === "function" && wordField.text.trim() ? page.engineSession.readings(wordField.text.trim()) : []
                property string suggested: ""
                onSuggestionsChanged: {
                    // As macOS fills the reading box: the first reading, unless the reading typed is one of them.
                    if (suggestions.length > 0 && !suggestions.includes(readingField.text) && (readingField.text === "" || readingField.text === suggested)) {
                        readingField.text = suggestions[0];
                        suggested = suggestions[0];
                    }
                }
                contentItem: ColumnLayout {
                    spacing: 10
                    Text {
                        Layout.fillWidth: true
                        text: list.apps ? (Qt.platform.os === "windows" ? "輸入程式的檔名，例如 notepad.exe。" : "輸入程式的名稱，例如 firefox。") : "自訂詞在選字時優先列出。"
                        color: page.theme.secondary
                        font.pixelSize: page.theme.detailSize
                        wrapMode: Text.Wrap
                    }
                    GridLayout {
                        Layout.fillWidth: true
                        columns: 2
                        columnSpacing: 8
                        rowSpacing: 8
                        Text {
                            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
                            text: list.apps ? "App：" : "詞："
                            color: page.theme.text
                            font.pixelSize: page.theme.bodySize
                        }
                        AppTextField {
                            id: wordField
                            objectName: "personalWord"
                            theme: page.theme
                            Layout.fillWidth: true
                            placeholderText: list.apps ? (Qt.platform.os === "windows" ? "notepad.exe" : "firefox") : "例如：王小明"
                            Accessible.name: list.apps ? "App" : "詞"
                            onAccepted: addButtonInSheet.clicked()
                        }
                        Text {
                            visible: !list.apps
                            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
                            text: "讀音："
                            color: page.theme.text
                            font.pixelSize: page.theme.bodySize
                        }
                        AppTextField {
                            id: readingField
                            objectName: "personalReading"
                            theme: page.theme
                            Layout.fillWidth: true
                            visible: !list.apps
                            placeholderText: "ㄨㄤˊ ㄒㄧㄠˇ ㄇㄧㄥˊ"
                            Accessible.name: "讀音"
                            onAccepted: addButtonInSheet.clicked()
                        }
                        Item {
                            visible: !list.apps && addDialog.suggestions.length > 1
                            implicitWidth: 1
                        }
                        Flow {
                            objectName: "readingSuggestions"
                            visible: !list.apps && addDialog.suggestions.length > 1
                            Layout.fillWidth: true
                            spacing: 4
                            Repeater {
                                model: addDialog.suggestions
                                delegate: AppButton {
                                    required property string modelData
                                    theme: page.theme
                                    kind: "plain"
                                    compact: true
                                    text: modelData
                                    onClicked: {
                                        readingField.text = modelData;
                                        addDialog.suggested = modelData;
                                    }
                                }
                            }
                        }
                    }
                    Text {
                        Layout.fillWidth: true
                        text: page.operationError
                        visible: text.length > 0
                        color: page.theme.danger
                        font.pixelSize: page.theme.detailSize
                        wrapMode: Text.Wrap
                    }
                }
                footer: AppDialogButtons {
                    AppButton {
                        theme: page.theme
                        text: "取消"
                        onClicked: addDialog.reject()
                    }
                    AppButton {
                        id: addButtonInSheet
                        theme: page.theme
                        kind: "primary"
                        text: addDialog.editing ? "儲存" : "加入"
                        enabled: list.connected && wordField.text.trim().length > 0 && (list.apps || readingField.text.trim().length > 0)
                        onClicked: {
                            if (!enabled)
                                return;
                            const word = wordField.text.trim();
                            const reading = readingField.text.trim().split(/\s+/).join(" ");
                            const editing = addDialog.editing;
                            let done;
                            if (list.apps)
                                done = page.invoke(page.platformSettings, "addApp", [word]);
                            else if (editing)
                                done = page.invoke(page.engineSession, "editPhrase", [editing.word, editing.reading, word, reading]);
                            else
                                done = page.invoke(page.engineSession, "addPhrase", [word, reading]);
                            if (done) {
                                list.selection = [list.key(list.apps ? {
                                        word: typeof page.platformSettings.appName === "function" ? page.platformSettings.appName(word) : word,
                                        reading: word
                                    } : {
                                        word: word,
                                        reading: reading
                                    })];
                                addDialog.close();
                            } else if (!list.apps) {
                                page.operationError = "無法加入「" + word + "」：詞需為 1 到 6 個字，讀音需為每個字一個注音，以空白分隔。";
                            }
                        }
                    }
                }
            }
            property Dialog clearConfirmation: AppDialog {
                id: clearDialog
                theme: page.theme
                title: "要清除所有記住的字詞嗎？"
                contentItem: Text {
                    text: "清除後無法復原。不記住的字詞會保留。"
                    color: page.theme.secondary
                    font.pixelSize: page.theme.bodySize
                    wrapMode: Text.Wrap
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
                        text: "清除"
                        DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                    }
                }
                onAccepted: page.invoke(page.engineSession, "forgetAllLearned", [])
            }
            property Dialog failure: AppDialog {
                id: failureDialog
                property string detail: ""
                theme: page.theme
                contentItem: Text {
                    text: failureDialog.detail
                    color: page.theme.secondary
                    font.pixelSize: page.theme.bodySize
                    wrapMode: Text.Wrap
                }
                footer: AppDialogButtons {
                    AppButton {
                        theme: page.theme
                        kind: "primary"
                        text: "好"
                        DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                    }
                }
            }
            property FileDialog importPicker: FileDialog {
                id: importFile
                title: "匯入的資料會加到這台電腦現有的資料中。"
                acceptLabel: "匯入"
                fileMode: FileDialog.OpenFile
                nameFilters: ["文字檔 (*.txt)", "所有檔案 (*)"]
                onAccepted: {
                    if (!page.invoke(page.engineSession, "importPersonal", [selectedFile])) {
                        failureDialog.title = "無法匯入「" + decodeURIComponent(String(selectedFile).split("/").pop()) + "」";
                        failureDialog.detail = "這個檔案不是 ZhuyinType 匯出的個人資料。";
                        failureDialog.open();
                    }
                }
            }
            property FileDialog exportPicker: FileDialog {
                id: exportFile
                title: "自訂詞、記住的字詞與不記住的字詞會存成一個文字檔。"
                fileMode: FileDialog.SaveFile
                currentFolder: StandardPaths.writableLocation(StandardPaths.DocumentsLocation)
                selectedFile: currentFolder + "/ZhuyinType 個人資料.txt"
                defaultSuffix: "txt"
                nameFilters: ["文字檔 (*.txt)"]
                onAccepted: {
                    if (!page.invoke(page.engineSession, "exportPersonal", [selectedFile])) {
                        failureDialog.title = "無法匯出";
                        failureDialog.detail = page.operationError;
                        failureDialog.open();
                    }
                }
            }
        }
    }

    // MARK: 進階

    Component {
        id: advancedPage
        ColumnLayout {
            id: advanced
            spacing: page.theme.sectionSpacing
            readonly property var log: page.correctionLog
            readonly property int count: log ? log.count : 0
            SectionCard {
                theme: page.theme
                title: "改字記錄"
                footer: "記錄只存在這台電腦；密碼欄位中不記錄。"
                SettingRow {
                    id: recordsRow
                    definition: page.settings.definitions.find(item => item.key === "recordsCorrections") || ({
                            key: "recordsCorrections",
                            title: "記錄改字",
                            kind: "toggle"
                        })
                    settings: page.settings
                    theme: page.theme
                    info: true
                    anchor: "recordsCorrections"
                    onInfoClicked: correctionInfo.show(recordsRow)
                }
                // The context switch is the log's own (CorrectionLog.recordsContext), as on macOS.
                SettingRow {
                    separated: true
                    indent: true
                    dimmed: page.values.recordsCorrections !== true
                    enabled: page.values.recordsCorrections === true && advanced.log !== null
                    anchor: "recordsCorrectionContext"
                    definition: ({
                            key: "recordsCorrectionContext",
                            title: "一併記錄游標前的文字",
                            detail: "重播記錄時使用同樣的前文",
                            kind: "toggle"
                        })
                    settings: QtObject {
                        readonly property var values: ({
                                recordsCorrectionContext: !!page.correctionLog && page.correctionLog.recordsContext === true
                            })
                        function setValue(key, value) {
                            if (!page.correctionLog)
                                return false;
                            page.correctionLog.recordsContext = value;
                            return true;
                        }
                    }
                    theme: page.theme
                }
                SettingsRow {
                    theme: page.theme
                    separated: true
                    title: advanced.log ? advanced.count.toLocaleString(Qt.locale(), "f", 0) + " 筆記錄" : "無法讀取改字記錄"
                    AppButton {
                        objectName: "showLog"
                        theme: page.theme
                        compact: true
                        text: "在" + page.fileManager + "中顯示"
                        enabled: page.correctionLog !== null
                        onClicked: page.invoke(page.correctionLog, "showLog", [])
                    }
                    AppButton {
                        objectName: "exportLog"
                        theme: page.theme
                        compact: true
                        text: "匯出…"
                        enabled: page.correctionLog !== null && page.correctionLog.count > 0
                        onClicked: logExport.open()
                    }
                    AppButton {
                        objectName: "clearLog"
                        theme: page.theme
                        compact: true
                        text: "清除…"
                        enabled: page.correctionLog !== null && page.correctionLog.count > 0
                        onClicked: {
                            page.confirmationRequested("clearLog");
                            logClear.open();
                        }
                    }
                }
            }
            Text {
                Layout.fillWidth: true
                Layout.leftMargin: page.theme.cardInset
                text: page.operationError || (page.correctionLog && page.correctionLog.error ? page.correctionLog.error : "")
                visible: text.length > 0
                wrapMode: Text.Wrap
                color: page.theme.danger
                font.pixelSize: page.theme.detailSize
                Accessible.name: "操作錯誤：" + text
            }
            SectionCard {
                theme: page.theme
                title: "相容性"
                NavigationRow {
                    theme: page.theme
                    anchor: "apps"
                    title: "在候選字視窗顯示組字的 App"
                    detail: "少數 App 需要"
                    value: !page.platformSettings ? "" : page.platformSettings.apps.length === 0 ? "沒有" : page.platformSettings.apps.length + " 個"
                    onActivated: page.navigateRequested("apps", "在候選字視窗顯示組字的 App", "")
                }
            }
            SectionCard {
                theme: page.theme
                SettingsRow {
                    theme: page.theme
                    anchor: "reset"
                    title: "回復預設設定"
                    subtitle: "自訂詞與記住的字詞會保留"
                    AppButton {
                        theme: page.theme
                        compact: true
                        text: "回復…"
                        enabled: typeof page.settings.restoreDefaults === "function"
                        onClicked: resetDialog.open()
                    }
                }
            }
        }
    }
    Component {
        id: keysPage
        ShortcutOverview {
            shortcuts: page.shortcuts
            settings: page.settings
            theme: page.theme
            appInfo: page.appInfo
            reduceMotion: page.reduceMotion
            onNavigateRequested: (pageId, title, row) => page.navigateRequested(pageId, title, row)
        }
    }

    // MARK: The ⓘ cards and dialogs

    property QtObject dialogs: QtObject {
        property FileDialog logExportPicker: FileDialog {
            id: logExport
            title: "匯出改字記錄"
            fileMode: FileDialog.SaveFile
            currentFolder: StandardPaths.writableLocation(StandardPaths.DocumentsLocation)
            selectedFile: currentFolder + "/ZhuyinType 改字記錄.jsonl"
            defaultSuffix: "jsonl"
            nameFilters: ["改字記錄 (*.jsonl)", "所有檔案 (*)"]
            onAccepted: page.invoke(page.correctionLog, "exportLog", [selectedFile])
        }
        property Dialog logClearConfirmation: AppDialog {
            id: logClear
            theme: page.theme
            title: "要清除所有改字記錄嗎？"
            contentItem: Text {
                text: "清除後無法復原。"
                color: page.theme.secondary
                font.pixelSize: page.theme.bodySize
                wrapMode: Text.Wrap
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
                    text: "清除"
                    DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                }
            }
            onAccepted: page.invoke(page.correctionLog, "clearLog", [])
        }
        property Dialog resetConfirmation: AppDialog {
            id: resetDialog
            theme: page.theme
            title: "要回復所有設定的預設值嗎？"
            closePolicy: Popup.CloseOnEscape
            contentItem: Text {
                text: "自訂詞與記住的字詞會保留。"
                color: page.theme.secondary
                font.pixelSize: page.theme.bodySize
                wrapMode: Text.Wrap
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
                    text: "回復"
                    DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                }
            }
            onAccepted: {
                if (typeof page.settings.restoreDefaults === "function")
                    page.settings.restoreDefaults();
            }
        }
        property alias learningExplanation: learnedInfo
        property alias correctionExplanation: correctionInfo
    }
    // What is remembered and for how long (LearnedInfo, docs/product/DESIGN.md §2.5).
    InfoPopover {
        id: learnedInfo
        theme: page.theme
        title: "記住的字詞"
        FactRow {
            theme: page.theme
            iconName: "checkCircle"
            text: "選過的字詞：同樣的讀音直接採用"
        }
        FactRow {
            theme: page.theme
            iconName: "link"
            text: "常接在一起的詞：送出時學習，選字時參考"
        }
        FactRow {
            theme: page.theme
            iconName: "typing"
            text: "按錯習慣：常按錯的鍵更容易修正"
        }
        FactRow {
            theme: page.theme
            iconName: "clock"
            text: "近期與長期分開記，半衰期 8 小時與 90 天"
        }
        FactRow {
            theme: page.theme
            iconName: "lock"
            text: "密碼欄位中暫停學習"
        }
        FactRow {
            theme: page.theme
            iconName: "ban"
            text: "設為「不記住」的字詞之後不再學"
        }
    }
    // What the correction log records, with the current keys (CorrectionLogInfo, DESIGN.md §2.7).
    InfoPopover {
        id: correctionInfo
        theme: page.theme
        title: "記錄改字"
        readonly property var facts: [["checkCircle", "只記錄有改字的組字"], ["typing", "打的鍵與按鍵間隔，以及每一步畫面上的字"], ["list", ["從清單選字"].concat(page.keyText("correctionsList") ? [page.keyText("correctionsList") + " 修正"] : []).concat(page.keyText("singleCharacterList") ? [page.keyText("singleCharacterList") + " 單字"] : []).join("、")], ["deleteLeft", "Backspace 或 Esc 刪掉後重打；送出後 30 秒內刪掉重打"], ["textCursor", (page.keyText("typeZhuyin") ? [page.keyText("typeZhuyin") + " 打出注音"] : []).concat(page.selectLeft && page.selectRight ? [page.selectLeft + " " + page.selectRight + " 加入自訂詞"] : []).concat(["在句中修改"]).join("、")], ["alignLeft", "游標前的文字只在開啟第二個選項時記錄"], ["lock", "密碼欄位中不記錄；記錄中沒有 App 的名稱"], ["drive", "存在這台電腦的 corrections.jsonl，可以匯出"]]
        Repeater {
            model: correctionInfo.facts
            delegate: FactRow {
                required property var modelData
                theme: page.theme
                iconName: modelData[0]
                text: modelData[1]
            }
        }
    }
}
