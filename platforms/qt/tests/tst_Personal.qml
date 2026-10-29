import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../qml"

TestCase {
    id: test
    name: "Personal"
    width: 700
    height: 900
    visible: true
    when: windowShown
    Theme {
        id: testTheme
        motionEnabled: false
    }
    QtObject {
        id: testSettings
        property var definitions: []
        property var values: ({})
    }
    QtObject {
        id: backend
        property bool ready: true
        property string error: ""
        property bool reject: false
        property var calls: []
        property var userPhrases: []
        property var learnedEntries: []
        property var excludedEntries: []
        function record(method, word, reading) {
            calls = calls.concat([[method, String(word), reading]]);
            error = reject ? "無法儲存個人資料。" : "";
            return !reject;
        }
        function addPhrase(word, reading) {
            if (!record("addPhrase", word, reading))
                return false;
            userPhrases = userPhrases.concat([
                {
                    word: word,
                    reading: reading,
                    count: 0
                }
            ]);
            return true;
        }
        function removePhrase(word, reading) {
            if (!record("removePhrase", word, reading))
                return false;
            userPhrases = userPhrases.filter(row => row.word !== word || row.reading !== reading);
            return true;
        }
        function editPhrase(oldWord, oldReading, word, reading) {
            if (!record("editPhrase", word, reading))
                return false;
            userPhrases = userPhrases.filter(row => row.word !== oldWord || row.reading !== oldReading).concat([
                {
                    word: word,
                    reading: reading,
                    count: 0
                }
            ]);
            return true;
        }
        function readings(word) {
            return word === "你好" ? ["ㄋㄧˇ ㄏㄠˇ", "ㄋㄧˊ ㄏㄠˇ"] : [];
        }
        function forgetAllLearned() {
            return record("forgetAllLearned", "", "");
        }
        function forgetLearned(word, reading) {
            return record("forgetLearned", word, reading);
        }
        function excludeLearned(word, reading) {
            return record("excludeLearned", word, reading);
        }
        function includeLearned(word, reading) {
            return record("includeLearned", word, reading);
        }
        function importPersonal(url) {
            return record("importPersonal", url, "") ? 0 : -1;
        }
        function exportPersonal(url) {
            return record("exportPersonal", url, "");
        }
    }
    QtObject {
        id: platform
        property var apps: []
        property string error: ""
        function addApp(id) {
            apps = apps.concat([id]);
            return true;
        }
        function removeApp(id) {
            apps = apps.filter(value => value !== id);
            return true;
        }
        function appName(id) {
            return id === "gedit" ? "文字編輯器" : id;
        }
        function runningApps() {
            return [
                {
                    id: "gedit",
                    name: "文字編輯器"
                }
            ].filter(app => !apps.includes(app.id));
        }
    }
    QtObject {
        id: log
        property int count: 3
        property bool recordsContext: false
        property string error: ""
        property int clears: 0
        property int shows: 0
        property url exported
        function clearLog() {
            clears++;
            count = 0;
            return true;
        }
        function showLog() {
            shows++;
            return true;
        }
        function exportLog(url) {
            exported = url;
            return true;
        }
    }
    Component {
        id: component
        LibraryPages {
            width: 440
            settings: testSettings
            theme: testTheme
            engineSession: backend
            platformSettings: platform
            correctionLog: log
        }
    }
    function init() {
        backend.ready = true;
        backend.reject = false;
        backend.error = "";
        backend.calls = [];
        backend.userPhrases = [
            {
                word: "你好",
                reading: "ㄋㄧˇ ㄏㄠˇ",
                count: 0
            },
            {
                word: "你好",
                reading: "ㄋㄧˊ ㄏㄠˇ",
                count: 0
            }
        ];
        backend.learnedEntries = [
            {
                word: "學",
                reading: "ㄒㄩㄝˊ",
                count: 3
            }
        ];
        backend.excludedEntries = [
            {
                word: "停",
                reading: "ㄊㄧㄥˊ",
                count: 0
            }
        ];
        platform.apps = [];
        log.count = 3;
        log.clears = 0;
        log.shows = 0;
        log.recordsContext = false;
        testSettings.values = {
            recordsCorrections: true
        };
        failOnWarning(/.*(TypeError|ReferenceError|Binding loop).*/);
    }
    function find(item, property, value) {
        if (item[property] === value)
            return item;
        for (const child of item.children || []) {
            const found = find(child, property, value);
            if (found)
                return found;
        }
        return null;
    }
    function click(item) {
        verify(item);
        waitForRendering(item);
        mouseClick(item, item.width / 2, item.height / 2);
    }
    function view(id) {
        const result = createTemporaryObject(component, test, {
            pageId: id
        });
        verify(result);
        return result;
    }
    function test_searchSelectionAndExactRemoval() {
        const page = view("phrases");
        const list = find(page, "objectName", "personalList");
        const search = find(page, "objectName", "personalSearch");
        compare(list.shown.length, 2);
        compare(find(page, "objectName", "personalCount").text, "共 2 個");
        search.text = "ㄋㄧˇ";
        compare(list.shown.length, 1);
        compare(find(page, "objectName", "personalCount").text, "1 / 2 個");
        const rows = find(page, "objectName", "personalRows");
        tryVerify(() => rows.itemAtIndex(0) !== null);
        click(rows.itemAtIndex(0));
        compare(list.selected.length, 1);
        click(rows.itemAtIndex(0));
        compare(list.selected.length, 0);
        click(rows.itemAtIndex(0));
        // ⌫ removes the selection at once, as on macOS.
        keyClick(Qt.Key_Backspace);
        compare(backend.calls[0][0], "removePhrase");
        compare(backend.calls[0][2], "ㄋㄧˇ ㄏㄠˇ");
        compare(backend.userPhrases.length, 1);
        compare(list.selection.length, 0);
        compare(list.shown.length, 0);
        search.text = "";
        tryVerify(() => rows.itemAtIndex(0) !== null);
        // 刪除 on the row.
        rows.itemAtIndex(0).removeClicked();
        compare(backend.userPhrases.length, 0);
        verify(find(page, "title", "還沒有自訂詞"));
    }
    function test_longListUsesOuterViewport() {
        backend.userPhrases = Array.from({
            length: 5000
        }, (_, index) => ({
                    word: "詞" + String(index).padStart(4, "0"),
                    reading: "ㄘˊ",
                    count: 0
                }));
        const viewport = createTemporaryQmlObject('import QtQuick; Flickable { width: 440; height: 300; clip: true }', test);
        verify(viewport);
        const page = view("phrases");
        page.parent = viewport.contentItem;
        page.scrollViewport = viewport;
        page.height = page.implicitHeight;
        viewport.contentHeight = page.height;
        const rows = find(page, "objectName", "personalRows");
        compare(rows.count, 5000);
        tryVerify(() => !rows.interactive);
        verify(rows.height <= viewport.height);
        viewport.contentY = viewport.contentHeight - viewport.height;
        tryVerify(() => rows.contentY > 150000);
        tryVerify(() => rows.itemAtIndex(4999) !== null);
        const delegates = rows.contentItem.children.filter(child => child.title !== undefined && child.visible);
        verify(delegates.length < 40, "Only the visible band and cache should instantiate delegates");
    }
    function test_addEditAndFailureRetainsInput() {
        const page = view("phrases");
        const list = find(page, "objectName", "personalList");
        click(find(page, "objectName", "personalAdd"));
        tryCompare(list.addSheet, "opened", true);
        compare(list.addSheet.title, "加入自訂詞");
        const word = find(list.addSheet.contentItem, "objectName", "personalWord");
        const reading = find(list.addSheet.contentItem, "objectName", "personalReading");
        const add = find(list.addSheet.footer, "text", "加入");
        verify(!add.enabled);
        // The lexicon's readings fill the reading field, as macOS's combo box does.
        word.text = "你好";
        compare(reading.text, "ㄋㄧˇ ㄏㄠˇ");
        verify(find(list.addSheet.contentItem, "objectName", "readingSuggestions").visible);
        word.text = " 新詞 ";
        reading.text = " ㄒㄧㄣ   ㄘˊ ";
        backend.reject = true;
        click(add);
        compare(list.addSheet.visible, true);
        verify(page.operationError.startsWith("無法加入「新詞」"));
        compare(backend.userPhrases.length, 2);
        backend.reject = false;
        click(add);
        tryCompare(list.addSheet, "visible", false);
        compare(backend.userPhrases[2].word, "新詞");
        compare(backend.userPhrases[2].reading, "ㄒㄧㄣ ㄘˊ");
        compare(list.selected.length, 1);
        // 編輯 replaces the phrase.
        list.openAdd(backend.userPhrases[2]);
        tryCompare(list.addSheet, "opened", true);
        compare(list.addSheet.title, "編輯自訂詞");
        compare(word.text, "新詞");
        word.text = "新字";
        click(find(list.addSheet.footer, "text", "儲存"));
        tryCompare(list.addSheet, "visible", false);
        compare(backend.calls[backend.calls.length - 1][0], "editPhrase");
        verify(backend.userPhrases.some(row => row.word === "新字"));
        verify(!backend.userPhrases.some(row => row.word === "新詞"));
    }
    function test_learnedRemovalExclusionAndClear() {
        const page = view("learnedWords");
        const list = find(page, "objectName", "personalList");
        compare(list.shown[0].word, "學");
        compare(list.level(list.shown[0]), 5);
        compare(list.level(list.shown[1]), 0);
        verify(find(page, "objectName", "personalMore").visible);
        verify(!find(page, "objectName", "personalAdd").visible);
        list.exclude(list.entries);
        compare(backend.calls.length, 1);
        compare(backend.calls[0][0], "excludeLearned");
        list.remove(list.entries);
        compare(backend.calls[1][0], "forgetLearned");
        compare(backend.calls[2][0], "includeLearned");
        list.clearConfirmation.open();
        tryCompare(list.clearConfirmation, "opened", true);
        list.clearConfirmation.reject();
        compare(backend.calls.length, 3);
        list.clearConfirmation.open();
        list.clearConfirmation.accept();
        compare(backend.calls[3][0], "forgetAllLearned");
    }
    function test_batchStopsOnFailure() {
        const page = view("phrases");
        const list = find(page, "objectName", "personalList");
        backend.reject = true;
        list.remove(list.entries);
        compare(backend.calls.length, 1);
        compare(backend.userPhrases.length, 2);
        verify(page.operationError.length > 0);
        verify(find(page, "objectName", "personalError").visible);
    }
    function test_importAndExport() {
        const page = view("learnedWords");
        const list = find(page, "objectName", "personalList");
        list.importPicker.selectedFile = Qt.resolvedUrl("tst_Personal.qml").toString();
        list.importPicker.accepted();
        compare(backend.calls[0][0], "importPersonal");
        compare(backend.calls[0][1], Qt.resolvedUrl("tst_Personal.qml").toString());
        compare(page.operationError, "");
        backend.reject = true;
        list.importPicker.accepted();
        tryCompare(list.failure, "opened", true);
        compare(list.failure.title, "無法匯入「tst_Personal.qml」");
        list.failure.close();
        tryCompare(list.failure, "visible", false);
        list.exportPicker.selectedFile = "file:///tmp/export.txt";
        list.exportPicker.accepted();
        compare(backend.calls[2][0], "exportPersonal");
        compare(page.operationError, backend.error);
        tryCompare(list.failure, "opened", true);
        compare(list.failure.title, "無法匯出");
    }
    function test_logs() {
        const page = view("advanced");
        verify(find(page, "title", "3 筆記錄"));
        click(find(page, "objectName", "clearLog"));
        tryCompare(page.dialogs.logClearConfirmation, "opened", true);
        compare(log.clears, 0);
        page.dialogs.logClearConfirmation.reject();
        compare(log.clears, 0);
        click(find(page, "objectName", "clearLog"));
        page.dialogs.logClearConfirmation.accept();
        compare(log.clears, 1);
        verify(find(page, "title", "0 筆記錄"));
        verify(!find(page, "objectName", "clearLog").enabled);
        verify(!find(page, "objectName", "exportLog").enabled);
        click(find(page, "objectName", "showLog"));
        compare(log.shows, 1);
        page.dialogs.logExportPicker.selectedFile = "file:///tmp/corrections.jsonl";
        page.dialogs.logExportPicker.accepted();
        compare(String(log.exported), "file:///tmp/corrections.jsonl");
        compare(page.dialogs.correctionExplanation.facts.length, 8);
        // The context switch is the log's own and follows 記錄改字.
        const context = find(page, "title", "一併記錄游標前的文字");
        verify(context.enabled);
        context.settings.setValue("recordsCorrectionContext", true);
        compare(log.recordsContext, true);
        testSettings.values = {
            recordsCorrections: false
        };
        verify(!context.enabled);
        page.correctionLog = null;
        verify(!find(page, "objectName", "showLog").enabled);
        verify(find(page, "title", "無法讀取改字記錄"));
    }
    function test_appsAndReadiness() {
        const page = view("apps");
        const list = find(page, "objectName", "personalList");
        verify(find(page, "title", "沒有 App"));
        list.addSheet.open();
        tryCompare(list.addSheet, "opened", true);
        find(list.addSheet.contentItem, "objectName", "personalWord").text = "gedit";
        click(find(list.addSheet.footer, "text", "加入"));
        compare(platform.apps.length, 1);
        compare(list.shown[0].word, "文字編輯器");
        compare(list.shown[0].reading, "gedit");
        compare(platform.runningApps().length, 0);
        list.remove(list.entries);
        compare(platform.apps.length, 0);
        page.platformSettings = null;
        verify(!find(page, "objectName", "personalAdd").enabled);
        const phrases = view("phrases");
        backend.ready = false;
        verify(!find(phrases, "objectName", "personalAdd").enabled);
        verify(find(phrases, "title", "正在載入詞庫…"));
    }
}
