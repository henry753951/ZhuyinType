import QtQuick
import QtTest
import "../qml"

TestCase {
    id: test
    name: "SharedControls"
    width: 700
    height: 600
    when: windowShown
    Theme {
        id: testTheme
    }
    QtObject {
        id: testSettings
        property var values: ({
                size: 18,
                enabled: false
            })
        function setValue(key, value) {
            let next = Object.assign({}, values);
            next[key] = value;
            values = next;
            return true;
        }
    }
    Component {
        id: rowComponent
        SettingRow {
            theme: testTheme
            settings: testSettings
        }
    }
    Component {
        id: navigation
        NavigationRow {
            theme: testTheme
        }
    }
    SignalSpy {
        id: navigationSpy
        signalName: "activated"
    }
    QtObject {
        id: candidateState
        property var rows: [
            {
                text: "曉明"
            },
            {
                text: "小明"
            },
            {
                text: "小名"
            },
            {
                text: "明"
            },
            {
                text: "名"
            },
            {
                text: "鳴"
            },
            {
                text: "銘"
            },
            {
                text: "冥"
            },
            {
                text: "茗"
            }
        ]
        property string reading: "ㄒㄧㄠˇ ㄇㄧㄥˊ"
        property int page: 0
        property int pageSize: 9
        property int pageCount: 2
        property int selected: 0
        property bool single: false
        property bool corrections: false
        property int committed: -1
        signal changed
        function pick(index) {
            committed = index;
        }
        function moveSelection(delta) {
            selected = Math.max(0, Math.min(8, selected + delta));
        }
    }
    Component {
        id: candidateComponent
        CandidatePanel {
            theme: testTheme
            candidates: candidateState
        }
    }

    function test_horizontalCandidatesFitAndKeyboard() {
        const panel = createTemporaryObject(candidateComponent, test, {
            width: 390,
            maximumWidth: 390,
            keyboardEnabled: true,
            preferences: {
                candidateLayout: "horizontal",
                candidateFontSize: 18,
                candidateDensity: "standard",
                showsListSwitches: true,
                showsZhuyinHint: true,
                showsAnnotations: true,
                selectionKeys: "123456789"
            }
        });
        verify(panel);
        panel.height = panel.implicitHeight;
        waitForRendering(panel);
        const list = findChild(panel, "candidateList");
        verify(list.contentWidth <= list.width + 1, "All nine candidates must fit without horizontal scrolling");
        const first = findChild(panel, "candidate0");
        const last = findChild(panel, "candidate8");
        verify(first && last);
        verify(first.width > last.width, "Two-character candidates need more room than single characters");
        panel.forceActiveFocus();
        keyClick(Qt.Key_Right);
        compare(candidateState.selected, 1);
        keyClick(Qt.Key_Return);
        compare(candidateState.committed, 1);
        keyClick(Qt.Key_9);
        compare(candidateState.committed, 8);
    }

    function test_narrowSliderKeyboard() {
        testSettings.values = {
            size: 18,
            enabled: false
        };
        const row = createTemporaryObject(rowComponent, test, {
            width: 320,
            definition: {
                title: "字級",
                key: "size",
                kind: "number",
                control: "slider",
                min: 12,
                max: 32
            }
        });
        verify(row);
        waitForRendering(row);
        compare(row.stacked, true);
        const loader = findChild(row, "settingControl");
        const slider = findChild(row, "fontSlider");
        slider.forceActiveFocus();
        keyClick(Qt.Key_Right);
        compare(testSettings.values.size, 19);
        const box = loader.mapToItem(row, 0, 0);
        verify(box.x >= 0 && box.x + loader.width <= row.width + 1);
        row.width = 600;
        tryCompare(row, "stacked", false);
    }
    function test_navigationKeyboardAndLongText() {
        const row = createTemporaryObject(navigation, test, {
            width: 300,
            title: "在候選字視窗顯示組字的應用程式",
            detail: "檢查窄視窗下較長的說明能換行並保留操作",
            value: "尚未連接"
        });
        verify(row);
        navigationSpy.target = row;
        navigationSpy.clear();
        row.forceActiveFocus();
        keyClick(Qt.Key_Space);
        compare(navigationSpy.count, 1);
        verify(row.implicitHeight >= 48);
    }

    QtObject {
        id: testShortcuts
        property var commands: [
            {
                id: "correctionsList",
                title: "列出修正的字",
                feature: "feature.correctionsList",
                defaults: [{ label: "Shift ＋ ↑" }]
            },
            {
                id: "singleCharacterList",
                title: "只列一個字",
                feature: "feature.singleCharacterList",
                defaults: [{ label: "Shift ＋ ↓" }]
            }
        ]
        property var values: ({
                correctionsList: [{ label: "Ctrl ＋ U" }],
                singleCharacterList: [{ label: "Shift ＋ ↓" }]
            })
    }
    // The lines that follow the value, and those that name keys, come from the current settings and keys.
    function test_dynamicDetails() {
        testSettings.values = {
            "feature.correctionsList": true,
            "feature.singleCharacterList": true,
            typoTolerance: "standard",
            candidatesPerPage: 5,
            selectionKeys: "asdfghjkl",
            escape: "reading"
        };
        const feature = createTemporaryObject(rowComponent, test, {
            width: 500,
            shortcuts: testShortcuts,
            definition: { key: "feature.correctionsList", title: "列出修正的字", kind: "toggle", detail: "按錯修正與模糊音的字排在前面；再按一次還原", commands: ["correctionsList"], anchor: "corrections" }
        });
        compare(feature.subtitle, "按錯修正與模糊音的字排在前面；再按一次還原");
        compare(feature.anchor, "corrections");
        testSettings.setValue("feature.correctionsList", false);
        compare(feature.subtitle, "關閉時，Ctrl ＋ U 交給 App");
        testSettings.setValue("feature.correctionsList", true);
        testSettings.setValue("typoTolerance", "off");
        compare(feature.dimmed, true);
        compare(feature.subtitle, "「按錯鍵修正」關閉且沒有開啟模糊音：這個鍵開啟一般清單");
        const perPage = createTemporaryObject(rowComponent, test, {
            width: 500,
            definition: { key: "candidatesPerPage", title: "每頁的候選字", kind: "number", min: 3, max: 9, unit: " 個" }
        });
        compare(perPage.subtitle, "用 A S D F G 選字；Return 選取反白的字");
        const escape = createTemporaryObject(rowComponent, test, {
            width: 500,
            definition: { key: "escape", title: "Esc", kind: "choice", options: [{ value: "composition", label: "清除整段組字" }, { value: "reading", label: "只清除注音" }], details: { composition: "先清除注音，再按一次清除整段組字", reading: "只清除還沒打完的注音" } }
        });
        compare(escape.subtitle, "只清除還沒打完的注音");
        const switches = createTemporaryObject(rowComponent, test, {
            width: 500,
            shortcuts: testShortcuts,
            definition: { key: "showsListSwitches", title: "單字與修正按鈕", kind: "toggle" }
        });
        compare(switches.subtitle, "清單下方的 Shift ＋ ↓ 單字、Ctrl ＋ U 修正；關閉時只在使用中顯示");
        testSettings.setValue("feature.correctionsList", false);
        testSettings.setValue("feature.singleCharacterList", false);
        compare(switches.subtitle, "兩種清單都已關閉");
        compare(findChild(switches, "settingControl").enabled, false);
    }
}
