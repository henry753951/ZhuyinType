pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// 介紹, 分析 and 分析 › 效能 (macOS IntroPane.swift, AnalyticsPane.swift).
ColumnLayout {
    id: page
    required property var settings
    required property Theme theme
    property string pageId: "intro"
    property var appInfo: ({})
    property var services: null
    property var engineSession: null
    property var inputController: null
    property var shortcuts: null
    /// The setup wizard (WizardWindow): show(mode), hasNewLessons.
    property var wizard: null
    /// services.usage: it changes at most every two seconds while typing.
    readonly property var usage: services ? services.usage : ({})
    readonly property bool usageReady: services !== null && usage.valid !== false
    readonly property bool counting: settings.values.recordsUsageStats !== false
    readonly property var values: settings.values
    property int tick: 0
    property var usageSettings: QtObject {
        property string error: ""
        readonly property var values: page.settings.values
        readonly property var definitions: page.settings.definitions.filter(item => item.key === "recordsUsageStats")
        function setValue(key, value) {
            error = "";
            if (key !== "recordsUsageStats")
                return false;
            if (!page.settings.setValue(key, value)) {
                error = page.settings.error || "無法儲存使用統計偏好。";
                return false;
            }
            page.refresh();
            return true;
        }
    }
    property var dialogs: QtObject {
        property Dialog usageConfirmation: AppDialog {
            id: clearDialog
            objectName: "clearUsageDialog"
            theme: page.theme
            title: "要清除使用統計嗎？"
            contentItem: ColumnLayout {
                spacing: 8
                Text {
                    Layout.fillWidth: true
                    text: "所有計數與每天的紀錄都會刪除，無法復原。"
                    wrapMode: Text.Wrap
                    color: page.theme.text
                    font.pixelSize: page.theme.bodySize
                }
                Text {
                    Layout.fillWidth: true
                    visible: text.length > 0
                    text: page.services && page.services.usageError ? page.services.usageError : ""
                    wrapMode: Text.Wrap
                    color: page.theme.danger
                    font.pixelSize: page.theme.detailSize
                }
            }
            footer: AppDialogButtons {
                AppButton {
                    theme: page.theme
                    text: "取消"
                    onClicked: clearDialog.reject()
                }
                AppButton {
                    objectName: "confirmClearUsage"
                    theme: page.theme
                    kind: "danger"
                    text: "清除"
                    onClicked: {
                        if (page.services && page.services.clearUsage()) {
                            clearDialog.accept();
                            page.refresh();
                        }
                    }
                }
            }
        }
    }
    // What 使用統計 counts (macOS UsageInfo, docs/product/DESIGN.md §2.8).
    InfoPopover {
        id: usageInfo
        objectName: "usageInfo"
        theme: page.theme
        title: "使用統計"
        Repeater {
            model: [["analytics", "送出的字數、組字次數、按鍵數"], ["list", "從各種清單選字的次數、各種改字的次數、加入自訂詞的次數"], ["check", "整句有按錯修正的組字數"], ["clock", "每個按鍵的反應時間，只記分布與最慢的一次"], ["document", "每天一筆，保留 " + (page.services ? page.services.keptDays : 60) + " 天"], ["ban", "不記錄打的字、讀音、按了哪些鍵與 App"], ["lock", "密碼欄位中不計數"], ["drive", "存在這台電腦的 stats.json"]]
            delegate: FactRow {
                required property var modelData
                theme: page.theme
                iconName: modelData[0]
                text: modelData[1]
            }
        }
    }
    signal navigateRequested(string pageId, string title, string row)
    spacing: 12

    function refresh() {
        tick += 1;
    }
    Timer {
        // The process numbers and word counts, every two seconds only while 分析 or 效能 is on screen.
        interval: 2000
        repeat: true
        running: page.visible && (page.pageId === "analytics" || page.pageId === "performance")
        onTriggered: page.refresh()
    }

    function formatted(value) {
        return Number(value || 0).toLocaleString(Qt.locale("en_US"), "f", 0);
    }
    function ms(value) {
        if (value === null || value === undefined)
            return "—";
        return value < 10 ? value.toFixed(1) + " ms" : value.toFixed(0) + " ms";
    }
    function bytes(value) {
        const units = ["bytes", "KB", "MB", "GB", "TB"];
        let amount = value;
        let unit = 0;
        while (amount >= 1000 && unit < units.length - 1) {
            amount /= 1024;
            unit += 1;
        }
        return (unit === 0 ? amount.toFixed(0) : amount.toPrecision(3)) + " " + units[unit];
    }
    function duration(milliseconds) {
        const minutes = Math.floor((milliseconds || 0) / 60000);
        return minutes >= 60 ? Math.floor(minutes / 60) + " 小時 " + minutes % 60 + " 分" : minutes + " 分";
    }
    function monthDay(iso) {
        const date = new Date(iso + "T00:00:00");
        return (date.getMonth() + 1) + "月" + date.getDate() + "日";
    }
    function optionLabel(key) {
        const definition = settings.definitions.find(item => item.key === key);
        const option = definition ? (definition.options || []).find(item => item.value === values[key]) : null;
        return option ? option.label : String(values[key]);
    }
    // The key that switches 中/英 (macOS CurrentKeys.modeKey).
    function modeKey() {
        if (values.shiftSwitch)
            return values.shiftToggle === "left" ? "左 Shift" : values.shiftToggle === "right" ? "右 Shift" : "Shift";
        return values["feature.capsLockEnglish"] !== false ? "Caps Lock" : "";
    }

    Loader {
        Layout.fillWidth: true
        sourceComponent: page.pageId === "intro" ? intro : page.pageId === "analytics" ? analytics : page.pageId === "performance" ? performance : null
    }

    Component {
        id: intro
        ColumnLayout {
            spacing: page.theme.sectionSpacing
            // The name, then the typing story, in one card.
            SectionCard {
                theme: page.theme
                padded: true
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 14
                    Image {
                        source: "qrc:/branding/AppIcon.icns"
                        Layout.preferredWidth: 52
                        Layout.preferredHeight: 52
                        sourceSize.width: 104
                        sourceSize.height: 104
                        Accessible.ignored: true
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 3
                        Text {
                            Layout.fillWidth: true
                            text: "ZhuyinType"
                            color: page.theme.text
                            font.pixelSize: page.theme.titleSize
                            font.weight: Font.DemiBold
                        }
                        Text {
                            Layout.fillWidth: true
                            text: "開源、在本機運作的注音輸入法"
                            color: page.theme.secondary
                            font.pixelSize: page.theme.bodySize
                            wrapMode: Text.Wrap
                        }
                    }
                }
                IntroStage {
                    Layout.fillWidth: true
                    theme: page.theme
                    values: page.values
                    shortcutValues: page.shortcuts ? page.shortcuts.values : null
                }
            }
            SectionCard {
                theme: page.theme
                title: "目前的設定"
                NavigationRow {
                    theme: page.theme
                    title: "空白鍵"
                    value: page.optionLabel("spaceAction")
                    onActivated: page.navigateRequested("typing", "打字", "space")
                }
                NavigationRow {
                    theme: page.theme
                    separated: true
                    title: "標點按法"
                    value: page.values.punctuationKeys === "mac" ? "macOS" : "Windows"
                    onActivated: page.navigateRequested("typing", "打字", "punctuationKeys")
                }
                NavigationRow {
                    theme: page.theme
                    separated: true
                    title: "按錯鍵修正"
                    value: page.optionLabel("typoTolerance")
                    onActivated: page.navigateRequested("choosing", "選字", "typo")
                }
                NavigationRow {
                    theme: page.theme
                    separated: true
                    title: "中英切換"
                    value: page.modeKey() ? "按 " + page.modeKey() : "不使用"
                    onActivated: page.navigateRequested("typing", "打字", "shiftTap")
                }
            }
            SectionCard {
                theme: page.theme
                title: "設定嚮導"
                NavigationRow {
                    objectName: "openSetupWizard"
                    theme: page.theme
                    title: "開始設定嚮導"
                    detail: "選擇習慣的打法，並在嚮導中試打"
                    enabled: page.wizard !== null
                    onActivated: page.wizard.show("quick")
                }
                NavigationRow {
                    objectName: "openWhatsNew"
                    theme: page.theme
                    separated: true
                    visible: page.wizard !== null && page.wizard.hasNewLessons
                    title: "新功能導覽"
                    detail: "只介紹這一版新增的功能"
                    onActivated: page.wizard.show("newSince")
                }
                NavigationRow {
                    objectName: "openKeySummary"
                    theme: page.theme
                    separated: true
                    title: "你會用到的按鍵"
                    detail: "依目前的設定列出按鍵"
                    enabled: page.wizard !== null
                    onActivated: page.wizard.show("keysOnly")
                }
            }
            SectionCard {
                theme: page.theme
                title: "功能"
                bare: true
                GridLayout {
                    Layout.fillWidth: true
                    columns: width < 480 ? 1 : 2
                    columnSpacing: page.theme.tileSpacing
                    rowSpacing: page.theme.tileSpacing
                    Repeater {
                        model: [
                            {
                                title: "整句輸入",
                                text: "一句話連續打完，依讀音與前後文選出最通順的寫法。",
                                icon: "typing",
                                route: "appearance",
                                row: "",
                                destination: "視窗與提示"
                            },
                            {
                                title: "按錯鍵也能打對",
                                text: "按到相鄰的鍵、多按、漏按或順序打反時試著修正，照打的字優先。",
                                icon: "check",
                                route: "choosing",
                                row: "typo",
                                destination: "選字"
                            },
                            {
                                title: "依前文選字",
                                text: "參考游標前最多 128 個字，用語跟著偏向口語或書面。",
                                icon: "context",
                                route: "context",
                                row: "",
                                destination: "前文"
                            },
                            {
                                title: "記住你的用法",
                                text: "選過的字詞下次直接採用；資料只存在這台電腦。",
                                icon: "words",
                                route: "words",
                                row: "",
                                destination: "字詞"
                            },
                            {
                                title: "標點按法",
                                text: page.values.punctuationKeys === "mac" ? "Shift 加符號鍵打出中文標點，和 macOS 的注音相同。" : "Shift 加符號鍵打出鍵上的符號" + (page.values["feature.controlPunctuation"] !== false ? "，Ctrl 加符號鍵打中文標點" : "") + "，和 Windows 的注音相同。",
                                icon: "punctuation",
                                route: "typing",
                                row: "punctuationKeys",
                                destination: "打字"
                            },
                            {
                                title: "改字記錄",
                                text: "可以記錄改字的過程，用來改進選字；只存在這台電腦。",
                                icon: "advanced",
                                route: "advanced",
                                row: "",
                                destination: "進階"
                            },
                            {
                                title: "使用分析",
                                text: "打了多少字、改了幾次、輸入法反應多快。",
                                icon: "analytics",
                                route: "analytics",
                                row: "",
                                destination: "分析"
                            }
                        ]
                        delegate: FeatureCard {
                            required property var modelData
                            theme: page.theme
                            title: modelData.title
                            detail: modelData.text
                            iconName: modelData.icon
                            link: modelData.destination
                            Accessible.name: "開啟" + modelData.destination + "：" + modelData.title
                            onClicked: page.navigateRequested(modelData.route, modelData.destination, modelData.row)
                        }
                    }
                }
            }
        }
    }

    Component {
        id: analytics
        ColumnLayout {
            id: analyticsPage
            readonly property var u: page.usage
            readonly property bool month: (page.services ? page.services.period : "month") !== "all"
            readonly property real rate: (u.characters || 0) > 0 ? (u.corrections || 0) * 100 / u.characters : -1
            readonly property var rhythm: u.rhythm || ({})
            readonly property var words: {
                page.tick;
                return page.engineSession && page.engineSession.ready !== false ? {
                    phrases: (page.engineSession.userPhrases || []).length,
                    learned: (page.engineSession.learnedEntries || []).length
                } : null;
            }
            spacing: page.theme.sectionSpacing
            SectionCard {
                theme: page.theme
                bare: true
                SegmentedControl {
                    id: periodTabs
                    objectName: "analyticsPeriod"
                    theme: page.theme
                    model: ["過去 30 天", "全部"]
                    Layout.preferredWidth: Math.min(220, parent.width)
                    Layout.bottomMargin: 12
                    currentIndex: analyticsPage.month ? 0 : 1
                    onCurrentIndexChanged: {
                        if (page.services) {
                            page.services.setPeriod(currentIndex === 0 ? "month" : "all");
                            page.refresh();
                        }
                    }
                    Accessible.name: "期間"
                }
                // The recap: how much was typed, and a few chips.
                Rectangle {
                    objectName: "recapHero"
                    Layout.fillWidth: true
                    implicitHeight: hero.implicitHeight + page.theme.tilePadding * 2
                    radius: page.theme.cardRadius
                    gradient: Gradient {
                        orientation: Gradient.Horizontal
                        GradientStop {
                            position: 0
                            color: Qt.rgba(page.theme.accent.r, page.theme.accent.g, page.theme.accent.b, 0.16)
                        }
                        GradientStop {
                            position: 1
                            color: Qt.rgba(page.theme.accent.r, page.theme.accent.g, page.theme.accent.b, 0.04)
                        }
                    }
                    Accessible.role: Accessible.StaticText
                    Accessible.name: heroTitle.text + " " + heroNumber.text + " 個字"
                    ColumnLayout {
                        id: hero
                        anchors {
                            left: parent.left
                            right: parent.right
                            top: parent.top
                            margins: page.theme.tilePadding
                        }
                        spacing: 12
                        Text {
                            id: heroTitle
                            text: analyticsPage.month ? "過去 30 天，你打了" : "開始記錄以來，你打了"
                            color: page.theme.secondary
                            font.pixelSize: page.theme.bodySize
                        }
                        Row {
                            spacing: 6
                            Text {
                                id: heroNumber
                                objectName: "usageCharacters"
                                text: page.usageReady ? page.formatted(analyticsPage.u.characters) : "—"
                                color: page.theme.text
                                font.pixelSize: page.theme.titleSize * 2.2
                                font.weight: Font.DemiBold
                                font.features: ({ "tnum": 1 })
                            }
                            Text {
                                anchors.baseline: heroNumber.baseline
                                text: "個字"
                                color: page.theme.secondary
                                font.pixelSize: page.theme.headingSize + 2
                            }
                        }
                        Flow {
                            Layout.fillWidth: true
                            spacing: 8
                            Chip {
                                theme: page.theme
                                visible: (analyticsPage.u.streak || 0) > 1
                                text: "連續 " + analyticsPage.u.streak + " 天"
                            }
                            Chip {
                                theme: page.theme
                                visible: !!analyticsPage.u.bestDay
                                text: analyticsPage.u.bestDay ? "最多的一天 " + page.monthDay(analyticsPage.u.bestDay.date) + "・" + page.formatted(analyticsPage.u.bestDay.characters) + " 字" : ""
                            }
                            Chip {
                                theme: page.theme
                                visible: analyticsPage.u.firstTimeRight !== null && analyticsPage.u.firstTimeRight !== undefined
                                text: "一次就對 " + Math.round((analyticsPage.u.firstTimeRight || 0) * 100) + "%"
                            }
                            Chip {
                                theme: page.theme
                                visible: analyticsPage.rate >= 0
                                text: "每 100 字改 " + analyticsPage.rate.toFixed(1) + " 次"
                            }
                            Chip {
                                objectName: "pausedChip"
                                theme: page.theme
                                visible: !page.counting
                                text: "已暫停記錄"
                            }
                        }
                    }
                }
                Text {
                    Layout.fillWidth: true
                    Layout.topMargin: 8
                    visible: text.length > 0
                    text: !page.services ? "尚未連接統計服務。" : page.services.usageError
                    color: page.theme.danger
                    font.pixelSize: page.theme.detailSize
                    wrapMode: Text.Wrap
                }
            }
            SectionCard {
                theme: page.theme
                title: "每天送出的字"
                padded: true
                visible: (analyticsPage.u.characters || 0) > 0
                BarChart {
                    objectName: "dailyChart"
                    Layout.fillWidth: true
                    Layout.preferredHeight: 150
                    theme: page.theme
                    bars: (analyticsPage.u.days || []).map((day, index, all) => ({
                                value: day.characters,
                                label: (all.length - 1 - index) % 7 === 0 ? (new Date(day.date + "T00:00:00").getMonth() + 1) + "/" + new Date(day.date + "T00:00:00").getDate() : "",
                                tip: page.monthDay(day.date) + "・" + page.formatted(day.characters) + " 字"
                            }))
                    Accessible.name: "過去 30 天每天送出的字數"
                }
            }
            // 打字節奏: how fast while typing, how much was thinking, the longest runs, keys per character, when.
            SectionCard {
                theme: page.theme
                title: "打字節奏"
                bare: true
                GridLayout {
                    Layout.fillWidth: true
                    columns: 3
                    columnSpacing: page.theme.tileSpacing
                    rowSpacing: page.theme.tileSpacing
                    StatTile {
                        theme: page.theme
                        label: "打字速度（字/分鐘）"
                        value: page.formatted(Math.round(analyticsPage.u.activeSpeed || 0))
                    }
                    StatTile {
                        theme: page.theme
                        label: "一次就對（%）"
                        value: page.formatted(Math.round((analyticsPage.u.firstTimeRight || 0) * 100))
                    }
                    StatTile {
                        theme: page.theme
                        label: "最長連續（字）"
                        value: page.formatted(analyticsPage.rhythm.longestRun)
                    }
                }
            }
            SectionCard {
                theme: page.theme
                footer: "打字速度只算連續打字的時間；一次就對是沒有選字、Backspace 或 Esc 就送出的組字。"
                SettingsRow {
                    theme: page.theme
                    title: "打字時間・思考停頓"
                    subtitle: "停頓超過 " + (analyticsPage.rhythm.pauseThreshold || 2).toFixed(0) + " 秒算思考，超過 1 分鐘不計"
                    value: page.duration(analyticsPage.u.typingMilliseconds) + "・" + page.duration(analyticsPage.u.thinkingMilliseconds)
                    MeterBar {
                        theme: page.theme
                        value: analyticsPage.u.typingMilliseconds || 0
                        second: analyticsPage.u.thinkingMilliseconds || 0
                    }
                }
                ValueRow {
                    theme: page.theme
                    title: "一次不改字最長"
                    value: page.formatted(analyticsPage.rhythm.longestCleanRun) + " 字"
                }
                ValueRow {
                    theme: page.theme
                    title: "每個字按幾個鍵"
                    value: (analyticsPage.u.characters || 0) > 0 ? (analyticsPage.u.keys / analyticsPage.u.characters).toFixed(1) : "—"
                }
                ValueRow {
                    theme: page.theme
                    title: "思考停頓的中位數"
                    value: analyticsPage.rhythm.pauseMedian !== null && analyticsPage.rhythm.pauseMedian !== undefined ? analyticsPage.rhythm.pauseMedian.toFixed(1) + " 秒" : "—"
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 2
                    Layout.bottomMargin: page.theme.rowPadding + 2
                    spacing: 8
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: 1
                        color: page.theme.line
                    }
                    Text {
                        Layout.topMargin: page.theme.rowPadding - 8
                        text: "什麼時候打字"
                        color: page.theme.text
                        font.pixelSize: page.theme.bodySize
                    }
                    WeekHeatMap {
                        theme: page.theme
                        hours: analyticsPage.rhythm.hours || []
                    }
                }
            }
            SectionCard {
                theme: page.theme
                title: "打字"
                bare: true
                GridLayout {
                    Layout.fillWidth: true
                    columns: 3
                    columnSpacing: page.theme.tileSpacing
                    rowSpacing: page.theme.tileSpacing
                    Repeater {
                        model: [["送出的字", "characters"], ["組字", "compositions"], ["按鍵", "keys"], ["從清單選字", "picks"], ["改字", "corrections"], ["加入自訂詞", "phrasesAdded"]]
                        delegate: StatTile {
                            required property var modelData
                            theme: page.theme
                            label: modelData[0]
                            value: page.formatted(analyticsPage.u[modelData[1]])
                        }
                    }
                }
            }
            SectionCard {
                theme: page.theme
                title: "從清單選字"
                Repeater {
                    model: [["一般清單", "picksNormal"], ["修正清單", "picksCorrections"], ["單字清單", "picksSingle"], ["標點", "picksPunctuation"]]
                    delegate: SettingsRow {
                        id: pickRow
                        required property var modelData
                        required property int index
                        theme: page.theme
                        separated: index > 0
                        title: modelData[0]
                        value: page.formatted(analyticsPage.u[modelData[1]])
                        MeterBar {
                            theme: page.theme
                            value: analyticsPage.u[pickRow.modelData[1]] || 0
                            maximum: Math.max(analyticsPage.u.picksNormal || 0, analyticsPage.u.picksCorrections || 0, analyticsPage.u.picksSingle || 0, analyticsPage.u.picksPunctuation || 0)
                        }
                    }
                }
            }
            SectionCard {
                theme: page.theme
                title: "改字"
                Repeater {
                    model: [["Backspace 刪掉重打", "backspaces"], ["Esc 清除", "escapes"], ["送出後刪掉重打", "deleteAfterCommit"], ["打出注音本身", "literalZhuyin"], ["恢復成原本的字", "undoLiteral"]]
                    delegate: SettingsRow {
                        id: fixRow
                        required property var modelData
                        required property int index
                        theme: page.theme
                        separated: index > 0
                        title: modelData[0]
                        value: page.formatted(analyticsPage.u[modelData[1]])
                        MeterBar {
                            theme: page.theme
                            value: analyticsPage.u[fixRow.modelData[1]] || 0
                            maximum: Math.max(analyticsPage.u.backspaces || 0, analyticsPage.u.escapes || 0, analyticsPage.u.deleteAfterCommit || 0, analyticsPage.u.literalZhuyin || 0, analyticsPage.u.undoLiteral || 0)
                        }
                    }
                }
                ValueRow {
                    theme: page.theme
                    title: "自動修正"
                    detail: "整句中有按錯修正的字：送出的組字 / 出現過的組字"
                    value: page.formatted(analyticsPage.u.correctedCommitted) + " / " + page.formatted(analyticsPage.u.correctedShown)
                }
            }
            SectionCard {
                theme: page.theme
                title: "你的字詞"
                ValueRow {
                    theme: page.theme
                    divider: false
                    title: "自訂詞"
                    value: analyticsPage.words ? page.formatted(analyticsPage.words.phrases) + " 個" : "—"
                }
                ValueRow {
                    theme: page.theme
                    title: "記住的字詞"
                    value: analyticsPage.words ? page.formatted(analyticsPage.words.learned) + " 個" : "—"
                }
            }
            SectionCard {
                theme: page.theme
                title: "效能"
                NavigationRow {
                    objectName: "performanceRow"
                    theme: page.theme
                    title: "效能"
                    detail: "按鍵反應、CPU、記憶體、耗電與載入時間"
                    value: (analyticsPage.u.latency || {}).total > 0 ? "p95 " + page.ms(analyticsPage.u.latency.p95) : "還沒有資料"
                    onActivated: page.navigateRequested("performance", "效能", "")
                }
            }
            SectionCard {
                theme: page.theme
                title: "統計"
                footer: "只記錄次數與時間，不記錄打的字、讀音與 App；只存在這台電腦。統計只在記憶體中累加，每 10 分鐘最多寫入一次，不影響打字。"
                Repeater {
                    model: page.usageSettings.definitions
                    delegate: SettingRow {
                        id: countingRow
                        required property var modelData
                        objectName: "recordsUsageStatsRow"
                        theme: page.theme
                        definition: modelData
                        settings: page.usageSettings
                        anchor: "recordsUsageStats"
                        info: true
                        subtitle: page.counting ? "從 " + Qt.formatDate(new Date(analyticsPage.u.since + "T00:00:00"), "yyyy年M月d日") + " 開始記錄" : "關閉時不計數"
                        onInfoClicked: usageInfo.show(countingRow)
                    }
                }
                SettingsRow {
                    theme: page.theme
                    separated: true
                    title: "清除統計"
                    subtitle: "刪除所有計數與每天的紀錄"
                    AppButton {
                        objectName: "clearUsage"
                        theme: page.theme
                        compact: true
                        text: "清除…"
                        enabled: page.services !== null
                        onClicked: clearDialog.open()
                    }
                }
                Text {
                    Layout.fillWidth: true
                    Layout.bottomMargin: 10
                    visible: text.length > 0
                    text: page.usageSettings.error
                    color: page.theme.danger
                    font.pixelSize: page.theme.detailSize
                    wrapMode: Text.Wrap
                }
            }
        }
    }

    // 分析 › 效能: per-key latency from the counters, the rest from the system for this process.
    Component {
        id: performance
        ColumnLayout {
            id: performancePage
            readonly property var latency: page.usage.latency || ({
                    total: 0
                })
            readonly property var metrics: {
                page.tick;
                return page.services ? page.services.processMetrics() : ({});
            }
            readonly property var loads: page.engineSession && page.engineSession.loadTimes ? page.engineSession.loadTimes : ({})
            spacing: page.theme.sectionSpacing
            SectionCard {
                theme: page.theme
                SettingsRow {
                    objectName: "keyLatency"
                    theme: page.theme
                    title: "按鍵反應"
                    subtitle: performancePage.latency.total > 0 ? "從按鍵到畫面更新；" + page.formatted(performancePage.latency.total) + " 個按鍵" : ""
                    value: performancePage.latency.total > 0 ? ["p50 " + page.ms(performancePage.latency.p50), "p95 " + page.ms(performancePage.latency.p95), "最慢 " + page.ms(performancePage.latency.maximum)].join("・") : "還沒有資料"
                }
                BarChart {
                    objectName: "latencyChart"
                    visible: performancePage.latency.total > 0
                    Layout.fillWidth: true
                    Layout.preferredHeight: 124
                    Layout.bottomMargin: 8
                    theme: page.theme
                    annotateMaximum: false
                    axisNote: "毫秒以內"
                    bars: (performancePage.latency.counts || []).map((value, index) => ({
                                value: value,
                                index: index,
                                label: index < (performancePage.latency.bounds || []).length ? (performancePage.latency.bounds[index] < 1000 ? String(performancePage.latency.bounds[index]) : "1s") : "更慢",
                                tip: page.formatted(value) + " 個按鍵"
                            })).filter(bar => bar.value > 0 || bar.index < 12).map((bar, index) => Object.assign({}, bar, {
                                label: index % 2 === 0 ? bar.label : ""
                            }))
                    Accessible.name: "按鍵反應時間的分布"
                }
                ValueRow {
                    theme: page.theme
                    title: "CPU 時間"
                    detail: performancePage.metrics.launched ? "自 " + Qt.formatTime(new Date(performancePage.metrics.launched), "HH:mm") + " 啟動以來" : ""
                    value: performancePage.metrics.cpuSeconds !== undefined ? performancePage.metrics.cpuSeconds.toFixed(1) + " 秒" : "無法取得"
                }
                ValueRow {
                    theme: page.theme
                    title: "記憶體"
                    value: [performancePage.metrics.memoryBytes !== undefined ? "目前 " + page.bytes(performancePage.metrics.memoryBytes) : "", performancePage.metrics.peakMemoryBytes !== undefined ? "最高 " + page.bytes(performancePage.metrics.peakMemoryBytes) : ""].filter(part => part).join("・") || "無法取得"
                }
                ValueRow {
                    theme: page.theme
                    title: "耗電"
                    detail: "作業系統的估計，自啟動以來"
                    value: {
                        const joules = performancePage.metrics.energyJoules;
                        if (joules === undefined)
                            return "系統沒有提供";
                        const watts = performancePage.metrics.running > 0 ? joules / performancePage.metrics.running : 0;
                        return joules.toFixed(1) + " 焦耳・平均 " + (watts >= 1 ? watts.toFixed(2) + " 瓦" : (watts * 1000).toFixed(0) + " 毫瓦");
                    }
                }
                ValueRow {
                    objectName: "loadTimes"
                    theme: page.theme
                    title: "載入時間"
                    detail: "詞庫與語言模型、整句重排、逐字模型"
                    value: ["lexicon", "reranker", "character"].map(key => performancePage.loads[key] !== undefined && performancePage.loads[key] !== null && performancePage.loads[key] >= 0 ? page.ms(performancePage.loads[key]) : "—").join("・")
                }
            }
        }
    }
}
