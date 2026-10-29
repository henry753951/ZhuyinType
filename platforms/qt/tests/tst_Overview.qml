import QtQuick
import QtTest
import "../qml" as Pages

TestCase {
    id: test
    name: "OverviewPages"
    visible: true
    width: 640
    height: 800
    when: windowShown
    Pages.Theme {
        id: testTheme
        motionEnabled: false
    }
    readonly property var defaults: ({
            recordsUsageStats: true,
            spaceAction: "commit",
            punctuationKeys: "windows",
            typoTolerance: "standard",
            shiftSwitch: true,
            shiftToggle: "both",
            "feature.capsLockEnglish": true,
            "feature.controlPunctuation": true,
            "feature.correctionsList": true,
            "feature.selectPhrase": true,
            "feature.singleCharacterList": true,
            showModeToast: true,
            candidateLayout: "vertical",
            candidateDensity: "standard",
            candidateFontSize: 18,
            candidatesPerPage: 9,
            candidatePlacement: "below",
            selectionKeys: "123456789",
            showsAnnotations: true,
            showsZhuyinHint: true,
            showsListSwitches: true,
            pageAtListEdge: true,
            panelAnimation: false
        })
    QtObject {
        id: mockSettings
        property var values: test.defaults
        property var definitions: [
            {
                key: "recordsUsageStats",
                title: "記錄使用統計",
                kind: "toggle",
                page: "analytics",
                group: "統計",
                "default": true
            },
            {
                key: "spaceAction",
                title: "空白鍵",
                kind: "choice",
                options: [{
                        value: "commit",
                        label: "送出並輸入空白"
                    }, {
                        value: "candidates",
                        label: "開啟候選字視窗"
                    }]
            },
            {
                key: "typoTolerance",
                title: "按錯鍵修正",
                kind: "choice",
                options: [{
                        value: "off",
                        label: "關閉"
                    }, {
                        value: "standard",
                        label: "標準"
                    }]
            }
        ]
        property string error: ""
        function setValue(key, value) {
            const next = Object.assign({}, values);
            next[key] = value;
            values = next;
            return true;
        }
    }
    QtObject {
        id: shortcuts
        property var values: ({
                correctionsList: [{
                        key: 0,
                        modifiers: 0,
                        label: "Shift+↑"
                    }],
                selectLeft: [{
                        key: 0,
                        modifiers: 0,
                        label: "Shift+←"
                    }]
            })
    }
    QtObject {
        id: wizard
        property bool hasNewLessons: false
        property string shown: ""
        function show(mode) {
            shown = mode;
        }
    }
    function sample(characters) {
        const hours = [];
        for (let i = 0; i < 168; ++i)
            hours.push(i % 24 === 9 ? 30 : 0);
        return {
            valid: true,
            since: "2026-09-14",
            characters: characters,
            compositions: 12,
            cleanCompositions: 9,
            keys: 300,
            picks: 6,
            picksNormal: 4,
            picksCorrections: 1,
            picksSingle: 1,
            picksPunctuation: 0,
            corrections: 9,
            backspaces: 2,
            escapes: 1,
            deleteAfterCommit: 0,
            literalZhuyin: 0,
            undoLiteral: 0,
            correctedShown: 3,
            correctedCommitted: 2,
            phrasesAdded: 1,
            typingMilliseconds: 120000,
            thinkingMilliseconds: 30000,
            activeSpeed: characters / 2,
            firstTimeRight: characters > 0 ? 0.75 : null,
            streak: 3,
            bestDay: characters > 0 ? {
                date: "2026-09-27",
                characters: characters
            } : null,
            days: Array.from({
                length: 30
            }, (_, i) => ({
                        date: "2026-10-" + String(i + 1).padStart(2, "0"),
                        characters: i === 29 ? characters : 0
                    })),
            latency: {
                total: 300,
                p50: 2.2,
                p95: 9.6,
                maximum: 87,
                counts: [0, 100, 150, 30, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                bounds: [1, 2, 3, 4, 5, 6, 8, 10, 12, 15, 20, 25, 30, 40, 50, 75, 100, 150, 250, 500, 1000]
            },
            rhythm: {
                longestRun: 186,
                longestCleanRun: 74,
                pauseThreshold: 2,
                pauseMedian: 5.7,
                hours: hours
            }
        };
    }
    QtObject {
        id: services
        property string period: "month"
        property int keptDays: 60
        property var usage: test.sample(1234)
        property string usageError: ""
        property int clears: 0
        function setPeriod(value) {
            period = value;
            return true;
        }
        function clearUsage() {
            clears += 1;
            usage = test.sample(0);
            return true;
        }
        function processMetrics() {
            return {
                launched: "2026-10-29T09:30:00",
                running: 100,
                cpuSeconds: 83,
                memoryBytes: 52428800,
                peakMemoryBytes: 104857600
            };
        }
    }
    QtObject {
        id: engine
        property bool ready: true
        property var userPhrases: [{
                word: "王小明",
                reading: "ㄨㄤˊ ㄒㄧㄠˇ ㄇㄧㄥˊ"
            }]
        property var learnedEntries: [{}, {}]
        property var loadTimes: ({
                lexicon: 812.4,
                reranker: 41,
                character: null
            })
    }
    Pages.OverviewPages {
        id: overview
        width: test.width
        settings: mockSettings
        theme: testTheme
        shortcuts: shortcuts
    }
    SignalSpy {
        id: navigation
        target: overview
        signalName: "navigateRequested"
    }
    function init() {
        overview.dialogs.usageConfirmation.close();
        overview.services = null;
        overview.engineSession = null;
        overview.wizard = null;
        overview.pageId = "";
        mockSettings.values = test.defaults;
        services.usage = test.sample(1234);
        services.period = "month";
        services.clears = 0;
        wizard.shown = "";
        wizard.hasNewLessons = false;
        navigation.clear();
        test.width = 640;
    }
    function textOf(item) {
        const found = [];
        function walk(node) {
            if (node.visible === false)
                return;
            if (node.text !== undefined && typeof node.text === "string" && node.text.length > 0)
                found.push(node.text);
            if (node.title !== undefined && typeof node.title === "string")
                found.push(node.title);
            if (node.subtitle !== undefined && typeof node.subtitle === "string")
                found.push(node.subtitle);
            if (node.value !== undefined && typeof node.value === "string")
                found.push(node.value);
            for (const child of node.children)
                walk(child);
        }
        walk(item);
        return found.join("\n");
    }

    function test_routes_data() {
        return ["intro", "analytics", "performance"].map(route => ({
                    tag: route,
                    route: route
                }));
    }
    function test_routes(data) {
        overview.services = services;
        overview.engineSession = engine;
        overview.pageId = data.route;
        waitForRendering(overview);
        verify(overview.implicitHeight > 200);
    }
    function test_introMatchesMacOS() {
        overview.wizard = wizard;
        overview.pageId = "intro";
        waitForRendering(overview);
        const all = textOf(overview);
        verify(all.indexOf("開源、在本機運作的注音輸入法") >= 0);
        verify(all.indexOf("送出並輸入空白") >= 0);
        verify(all.indexOf("按 Shift") >= 0);
        verify(all.indexOf("選擇習慣的打法，並在嚮導中試打") >= 0);
        verify(all.indexOf("依目前的設定列出按鍵") >= 0);
        verify(all.indexOf("新功能導覽") < 0, "only after an update with new lessons");
        verify(all.indexOf("參考游標前最多 128 個字") >= 0);
        verify(all.indexOf("資料只存在這台電腦") >= 0);
        verify(all.indexOf("Ctrl 加符號鍵打中文標點") >= 0);
        verify(!findChild(overview, "clearTrial"), "no live trial on 介紹");
        verify(all.indexOf("固定範例") < 0);
        verify(!findChild(overview, "overviewLiveInput"));
        mockSettings.setValue("punctuationKeys", "mac");
        mockSettings.setValue("shiftSwitch", false);
        waitForRendering(overview);
        const mac = textOf(overview);
        verify(mac.indexOf("和 macOS 的注音相同") >= 0);
        verify(mac.indexOf("按 Caps Lock") >= 0);
        wizard.hasNewLessons = true;
        waitForRendering(overview);
        verify(findChild(overview, "openWhatsNew").visible);
    }
    function test_introLinksToRowsAndWizard() {
        overview.wizard = wizard;
        overview.pageId = "intro";
        waitForRendering(overview);
        findChild(overview, "openSetupWizard").activated();
        compare(wizard.shown, "quick");
        findChild(overview, "openKeySummary").activated();
        compare(wizard.shown, "keysOnly");
    }
    function test_introStory() {
        overview.pageId = "intro";
        waitForRendering(overview);
        const stage = findChild(overview, "introStage");
        verify(stage);
        const captions = stage.steps.map(step => step.caption);
        verify(captions.indexOf("列出按錯修正的字") >= 0);
        verify(captions.indexOf("加入自訂詞，下次直接打得出") >= 0);
        verify(captions.indexOf("按 Shift 切換中英") >= 0);
        verify(captions.indexOf("按 ↓，開啟候選字視窗") >= 0);
        compare(stage.steps[stage.steps.findIndex(step => step.caption === "選字")].keys[0], "2");
        mockSettings.values = Object.assign({}, test.defaults, {
            spaceAction: "candidates",
            "feature.correctionsList": false,
            "feature.selectPhrase": false,
            showModeToast: false,
            selectionKeys: "asdfghjkl"
        });
        waitForRendering(overview);
        const off = stage.steps.map(step => step.caption);
        verify(off.indexOf("列出按錯修正的字") < 0);
        verify(off.indexOf("選取字詞") < 0);
        verify(off.indexOf("按 Shift 切換中英") < 0);
        verify(off.indexOf("按 ↓ 或空白，開啟候選字視窗") >= 0);
        compare(stage.steps[stage.steps.findIndex(step => step.caption === "選字")].keys[0], "S");
    }
    function test_analyticsMatchesMacOS() {
        overview.services = services;
        overview.engineSession = engine;
        overview.pageId = "analytics";
        waitForRendering(overview);
        compare(findChild(overview, "usageCharacters").text, "1,234");
        const all = textOf(overview);
        for (const expected of ["過去 30 天，你打了", "連續 3 天", "最多的一天 9月27日・1,234 字", "一次就對 75%", "每 100 字改 0.7 次", "每天送出的字", "打字節奏", "打字速度（字/分鐘）", "思考停頓的中位數", "5.7 秒", "什麼時候打字", "從清單選字", "一般清單", "改字", "Backspace 刪掉重打", "自動修正", "2 / 3", "你的字詞", "1 個", "2 個", "p95 9.6 ms", "統計", "從 2026年9月14日 開始記錄", "清除統計", "每 10 分鐘最多寫入一次"])
            verify(all.indexOf(expected) >= 0, expected);
        verify(!findChild(overview, "pausedChip").visible);
        mockSettings.setValue("recordsUsageStats", false);
        waitForRendering(overview);
        verify(findChild(overview, "pausedChip").visible);
        verify(textOf(overview).indexOf("關閉時不計數") >= 0);
    }
    function test_periodSelection() {
        overview.services = services;
        overview.pageId = "analytics";
        waitForRendering(overview);
        const tabs = findChild(overview, "analyticsPeriod");
        tabs.currentIndex = 1;
        compare(services.period, "all");
        verify(textOf(overview).indexOf("開始記錄以來，你打了") >= 0);
    }
    function test_emptyStatsKeepTheRecap() {
        services.usage = test.sample(0);
        overview.services = services;
        overview.pageId = "analytics";
        waitForRendering(overview);
        compare(findChild(overview, "usageCharacters").text, "0");
        verify(!findChild(overview, "dailyChart").visible);
    }
    function test_clearConfirmation() {
        overview.services = services;
        overview.pageId = "analytics";
        waitForRendering(overview);
        mouseClick(findChild(overview, "clearUsage"));
        tryVerify(() => overview.dialogs.usageConfirmation.opened);
        compare(services.clears, 0);
        mouseClick(findChild(overview.dialogs.usageConfirmation.footer, "confirmClearUsage"));
        compare(services.clears, 1);
        compare(findChild(overview, "usageCharacters").text, "0");
    }
    function test_performance() {
        overview.services = services;
        overview.engineSession = engine;
        overview.pageId = "performance";
        waitForRendering(overview);
        compare(findChild(overview, "keyLatency").value, "p50 2.2 ms・p95 9.6 ms・最慢 87 ms");
        verify(findChild(overview, "latencyChart").visible);
        compare(findChild(overview, "loadTimes").value, "812 ms・41 ms・—");
        const all = textOf(overview);
        verify(all.indexOf("83.0 秒") >= 0);
        verify(all.indexOf("目前 50.0 MB・最高 100 MB") >= 0);
        verify(all.indexOf("系統沒有提供") >= 0);
        verify(all.indexOf("尚未接入") < 0);
        services.usage = Object.assign(test.sample(0), {
            latency: {
                total: 0,
                counts: []
            }
        });
        overview.refresh();
        waitForRendering(overview);
        compare(findChild(overview, "keyLatency").value, "還沒有資料");
    }
}
