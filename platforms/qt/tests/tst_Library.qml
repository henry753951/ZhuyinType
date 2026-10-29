import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../qml"

TestCase {
    id: test
    name: "LibraryPages"
    width: 900
    height: 900
    visible: true
    when: windowShown

    Theme {
        id: testTheme
    }
    QtObject {
        id: testSettings
        property var definitions: [
            {
                key: "learningEnabled",
                title: "記住選過的字詞",
                page: "words",
                group: "字詞",
                kind: "toggle",
                default: true
            },
            {
                key: "recordsCorrections",
                title: "記錄改字",
                page: "advanced",
                group: "改字記錄",
                kind: "toggle",
                default: false
            },
            {
                key: "recordsCorrectionContext",
                title: "一併記錄游標前的文字",
                page: "advanced",
                group: "改字記錄",
                kind: "toggle",
                default: false
            }
        ]
        property var values: ({})
        property int resets: 0
        function restoreDefaults() {
            resets++;
            return true;
        }
        function setValue(key, value) {
            const next = Object.assign({}, values);
            next[key] = value;
            values = next;
            return true;
        }
    }
    Component {
        id: library
        LibraryPages {
            settings: testSettings
            theme: testTheme
            width: 440
        }
    }
    Component {
        id: shortcuts
        ShortcutOverview {
            settings: testSettings
            theme: testTheme
            width: 440
        }
    }
    SignalSpy {
        id: navigationSpy
        signalName: "navigateRequested"
    }

    function init() {
        testTheme.appearance = "light";
        testTheme.motionEnabled = false;
        testSettings.resets = 0;
        testSettings.values = {
            learningEnabled: true,
            recordsCorrections: false,
            recordsCorrectionContext: false,
            candidateLayout: "vertical",
            selectionKeys: "123456789",
            shiftToggle: "both",
            "feature.pageKeys": true,
            showsZhuyinHint: true
        };
        navigationSpy.clear();
        failOnWarning(/.*(TypeError|ReferenceError|Binding loop).*/);
    }
    function clickCenter(item) {
        verify(item);
        waitForRendering(item);
        mouseClick(item, item.width / 2, item.height / 2);
    }
    function find(item, property, value) {
        if (item[property] === value)
            return item;
        for (const child of item.children || []) {
            const result = find(child, property, value);
            if (result)
                return result;
        }
        return null;
    }
    function test_pages_data() {
        const cases = [];
        for (const id of ["words", "learnedWords", "phrases", "apps", "advanced", "keys"])
            for (const w of [360, 440, 860])
                for (const mode of ["light", "dark"])
                    cases.push({
                        tag: id + "-" + w + "-" + mode,
                        pageId: id,
                        w: w,
                        mode: mode
                    });
        return cases;
    }
    function test_pages(data) {
        testTheme.appearance = data.mode;
        const view = createTemporaryObject(library, test, {
            pageId: data.pageId,
            width: data.w
        });
        verify(view);
        waitForRendering(view);
        verify(view.implicitHeight > 100);
        // The loaded page must stay within the width allocated by the shell.
        const loader = view.children[0];
        verify(loader.width <= view.width + 1);
    }
    function test_searchKeyboardAndClear() {
        const view = createTemporaryObject(shortcuts, test);
        const input = find(view, "placeholderText", "搜尋");
        verify(input);
        input.forceActiveFocus();
        keyClick(Qt.Key_E);
        keyClick(Qt.Key_S);
        keyClick(Qt.Key_C);
        // macOS KeysPane: Esc is fixed (清除注音, 關閉候選字視窗) and named in 打出注音本身's subtitle; the fixed
        // list opens while searching.
        compare(view.matchingSections.length, 1);
        compare(view.matchingFixed.length, 2);
        const clear = find(input, "objectName", "searchClear");
        clickCenter(clear);
        compare(input.text, "");
        compare(view.matchingSections.length, 5);
        input.text = "不存在的動作";
        compare(view.matchingSections.length, 0);
        compare(view.matchingFixed.length, 0);
        verify(find(view, "objectName", "keysEmpty").visible);
        keyClick(Qt.Key_Escape);
        compare(input.text, "");
        input.text = "拼注音";
        compare(view.matchingFixed.length, 1);
    }
    function test_listSearchKeyboard() {
        const view = createTemporaryObject(library, test, {
            pageId: "learnedWords",
            width: 360
        });
        const input = find(view, "placeholderText", "搜尋字詞");
        verify(input);
        input.forceActiveFocus();
        keyClick(Qt.Key_A);
        compare(input.text, "a");
        verify(find(view, "title", "正在載入詞庫…"));
        clickCenter(find(input, "objectName", "searchClear"));
        compare(input.text, "");
    }
    function row(view, id) {
        for (const section of view.sections)
            for (const entry of section.rows)
                if (entry.id === id)
                    return entry;
        return null;
    }
    function test_previewSettings() {
        const view = createTemporaryObject(shortcuts, test);
        testSettings.setValue("candidateLayout", "horizontal");
        testSettings.setValue("selectionKeys", "asdfghjkl");
        testSettings.setValue("shiftToggle", "left");
        testSettings.setValue("feature.pageKeys", false);
        testSettings.setValue("showsZhuyinHint", false);
        compare(row(view, "move").keys[0], "←→");
        compare(row(view, "move").subtitle, "橫式清單；↑ ↓ 翻頁");
        compare(row(view, "pick").keys[0], "A S D F G H J K L");
        compare(row(view, "shiftTap").keys, ["左 Shift"]);
        verify(!view.on(row(view, "nextPage").feature));
        compare(view.availability("zhuyinRowClick").note, "「顯示注音」已關閉，候選字視窗沒有注音列");
        verify(view.changed);
    }
    function test_fixedKeysKeyboardAndMotion() {
        const view = createTemporaryObject(shortcuts, test);
        const toggle = find(view, "objectName", "fixedKeysToggle");
        verify(toggle);
        verify(!find(view, "objectName", "keyRow_commit"));
        toggle.forceActiveFocus();
        keyClick(Qt.Key_Space);
        verify(view.showsFixed);
        tryVerify(() => !!find(view, "objectName", "keyRow_commit"));
        keyClick(Qt.Key_Space);
        verify(!view.showsFixed);
        tryVerify(() => !find(view, "objectName", "keyRow_commit"));
        testTheme.motionEnabled = true;
        compare(view.motionEnabled, true);
        testTheme.motionEnabled = false;
        compare(view.motionEnabled, false);
    }
    function test_informationCardMouseAndEscape() {
        const view = createTemporaryObject(library, test, {
            pageId: "words"
        });
        clickCenter(findInfo(view));
        const card = view.dialogs.learningExplanation;
        tryCompare(card, "opened", true);
        verify(card.height > 100 && card.height <= test.height - 24);
        keyClick(Qt.Key_Escape);
        tryCompare(card, "visible", false);
    }
    function findInfo(item) {
        if (item.iconName === "info" && item.visible)
            return item;
        for (const child of item.children || []) {
            const result = findInfo(child);
            if (result)
                return result;
        }
        return null;
    }
    function test_resetModalCancelAndConfirm() {
        const view = createTemporaryObject(library, test, {
            pageId: "advanced"
        });
        const launch = find(view, "text", "回復…");
        verify(launch);
        clickCenter(launch);
        const dialog = view.dialogs.resetConfirmation;
        tryCompare(dialog, "opened", true);
        compare(testSettings.resets, 0);
        clickCenter(find(dialog.footer, "text", "取消"));
        tryCompare(dialog, "visible", false);
        compare(testSettings.resets, 0);
        clickCenter(launch);
        tryCompare(dialog, "opened", true);
        clickCenter(find(dialog.footer, "text", "回復"));
        compare(testSettings.resets, 1);
        tryCompare(dialog, "visible", false);
    }
    function test_navigationKeyboard() {
        const view = createTemporaryObject(library, test, {
            pageId: "words"
        });
        navigationSpy.target = view;
        const row = find(view, "title", "自訂詞");
        verify(row);
        row.forceActiveFocus();
        keyClick(Qt.Key_Space);
        compare(navigationSpy.count, 1);
        compare(navigationSpy.signalArguments[0][0], "phrases");
    }
}
