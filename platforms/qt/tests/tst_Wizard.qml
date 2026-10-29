import QtQuick
import QtTest
import "../qml" as Pages

// The setup wizard: modes and steps, the sandbox, presets, 套用 and 全部捨棄 (macOS WizardModel).
TestCase {
    id: test
    name: "SetupWizard"
    when: windowShown
    readonly property var definitions: [
        {
            key: "spaceAction",
            title: "空白鍵",
            kind: "choice",
            "default": "commit",
            options: [{
                    value: "commit",
                    label: "送出並輸入空白"
                }, {
                    value: "insert",
                    label: "輸入空白，不送出"
                }, {
                    value: "candidates",
                    label: "開啟候選字視窗"
                }]
        },
        {
            key: "shiftSwitch",
            title: "單按 Shift 切換中英",
            kind: "toggle",
            "default": true
        },
        {
            key: "selectionKeys",
            title: "選字鍵",
            kind: "choice",
            "default": "123456789",
            options: [{
                    value: "123456789",
                    label: "1–9"
                }, {
                    value: "asdfghjkl",
                    label: "A S D F G H J K L"
                }]
        },
        {
            key: "feature.capsLockEnglish",
            title: "Caps Lock 打英文",
            kind: "toggle",
            "default": true
        },
        {
            key: "panelAnimation",
            title: "動畫",
            kind: "toggle",
            "default": false
        }
    ]
    component FakeSettings: QtObject {
        property var values: ({
                spaceAction: "commit",
                shiftSwitch: true,
                selectionKeys: "123456789",
                "feature.capsLockEnglish": true,
                panelAnimation: false,
                candidateLayout: "vertical",
                candidatesPerPage: 9,
                candidateFontSize: 18,
                candidateDensity: "standard",
                candidatePlacement: "below",
                panelAppearance: "light",
                showsAnnotations: true,
                showsZhuyinHint: true,
                showsListSwitches: true,
                showModeToast: true,
                pageAtListEdge: true,
                escape: "composition"
            })
        property var definitions: test.definitions
        property string error: ""
        property string wizardSeenVersion: ""
        function setValue(key, value) {
            const next = Object.assign({}, values);
            next[key] = value;
            values = next;
            return true;
        }
        function assign(map) {
            values = Object.assign({}, values, map);
            return true;
        }
        function markWizardSeen(version) {
            wizardSeenVersion = version;
        }
    }
    component FakeShortcuts: QtObject {
        property var values: ({
                nextPage: [{
                        key: Qt.Key_Tab,
                        modifiers: 0,
                        label: "Tab"
                    }, {
                        key: Qt.Key_PageDown,
                        modifiers: 0,
                        label: "Page Down"
                    }]
            })
        property var commands: [{
                id: "nextPage",
                title: "下一頁",
                feature: ""
            }, {
                id: "deleteWord",
                title: "刪掉一個詞",
                feature: ""
            }]
        function record(command, key, modifiers, index) {
            const list = (values[command] || []).slice();
            const entry = {
                key: key,
                modifiers: modifiers,
                label: key === Qt.Key_Tab ? "Tab" : key === Qt.Key_PageDown ? "Page Down" : "Alt+↓"
            };
            if (index < 0 || index >= list.length)
                list.push(entry);
            else
                list[index] = entry;
            const next = Object.assign({}, values);
            next[command] = list;
            values = next;
            return true;
        }
        function remove(command, index) {
            const list = (values[command] || []).slice();
            list.splice(index, 1);
            const next = Object.assign({}, values);
            next[command] = list;
            values = next;
            return true;
        }
    }
    FakeSettings {
        id: userSettings
    }
    FakeShortcuts {
        id: userShortcuts
    }
    QtObject {
        id: practice
        property var settings: null
        property var shortcuts: null
        property var engineSession: null
        property var inputController: null
        property int starts: 0
        function start(withEngine) {
            starts += 1;
            settings = sandboxComponent.createObject(test, {
                values: Object.assign({}, userSettings.values)
            });
            shortcuts = shortcutsComponent.createObject(test, {
                values: Object.assign({}, userShortcuts.values)
            });
            return true;
        }
        function restart() {
            settings.values = Object.assign({}, userSettings.values);
            return true;
        }
        function stop() {
            settings = null;
            shortcuts = null;
        }
    }
    Component {
        id: sandboxComponent
        FakeSettings {}
    }
    Component {
        id: shortcutsComponent
        FakeShortcuts {}
    }
    Pages.WizardWindow {
        id: wizard
        settings: userSettings
        shortcuts: userShortcuts
        practice: practice
    }
    function init() {
        wizard.visible = false;
        userSettings.values = Object.assign({}, userSettings.values, {
            spaceAction: "commit",
            shiftSwitch: true,
            selectionKeys: "123456789"
        });
        userSettings.wizardSeenVersion = "";
    }
    function test_modesAndSteps() {
        wizard.show("quick");
        verify(wizard.visible);
        compare(wizard.title, "設定嚮導");
        compare(wizard.steps.length, 2 + 6 + 2);
        compare(wizard.steps[0], "welcome");
        compare(wizard.steps[wizard.steps.length - 1], "keys");
        wizard.choose("full");
        compare(wizard.steps.length, 2 + 12 + 2);
        wizard.show("keysOnly");
        compare(wizard.title, "你會用到的按鍵");
        compare(wizard.steps, ["keys"]);
        verify(findChild(wizard.contentItem, "keySummary"));
        wizard.finishClose();
        verify(!wizard.visible);
        verify(!practice.settings, "the sandbox is thrown away");
    }
    function test_newLessonsOnlyAfterAnUpdate() {
        verify(!wizard.hasNewLessons);
        userSettings.wizardSeenVersion = "0.4.1";
        verify(wizard.hasNewLessons);
        wizard.show("newSince");
        compare(wizard.title, "新功能導覽");
        compare(wizard.steps[0], "lesson:typing");
        userSettings.wizardSeenVersion = wizard.currentVersion;
        verify(!wizard.hasNewLessons);
    }
    function test_firstUseOpensOnce() {
        wizard.showOnFirstUse();
        verify(wizard.visible);
        compare(userSettings.wizardSeenVersion, wizard.currentVersion);
        wizard.finishClose();
        wizard.showOnFirstUse();
        verify(!wizard.visible);
    }
    function test_choicesStayInTheSandboxUntilApplied() {
        wizard.show("quick");
        wizard.choosePreset(wizard.presets.find(preset => preset.id === "mac"));
        compare(practice.settings.values.spaceAction, "candidates");
        compare(practice.settings.values.shiftSwitch, false);
        compare(userSettings.values.spaceAction, "commit", "nothing written yet");
        const titles = wizard.changes.map(change => change.title);
        verify(titles.indexOf("空白鍵") >= 0);
        compare(wizard.changes.find(change => change.id === "spaceAction").old, "送出並輸入空白");
        compare(wizard.changes.find(change => change.id === "spaceAction").now, "開啟候選字視窗");
        // A combinable key style: Alt+↓ added to 下一頁.
        const altDown = wizard.groups.paging.styles[2];
        verify(!wizard.active(altDown));
        wizard.setStyle(altDown, true);
        verify(wizard.active(altDown));
        verify(wizard.changes.find(change => change.id === "keys.nextPage"));
        // 改回 one change.
        wizard.revert(wizard.changes.find(change => change.id === "shiftSwitch"));
        compare(practice.settings.values.shiftSwitch, true);
        wizard.go(wizard.steps.indexOf("finish"));
        compare(wizard.step, "finish");
        wizard.apply();
        compare(userSettings.values.spaceAction, "candidates");
        compare(userSettings.values.shiftSwitch, true);
        compare(userShortcuts.values.nextPage.length, 3);
        compare(wizard.step, "keys");
        compare(userSettings.wizardSeenVersion, wizard.currentVersion);
    }
    function test_discardWritesNothing() {
        wizard.show("quick");
        practice.settings.setValue("selectionKeys", "asdfghjkl");
        wizard.discard();
        compare(userSettings.values.selectionKeys, "123456789");
        verify(!wizard.visible);
        compare(userSettings.wizardSeenVersion, wizard.currentVersion);
    }
    function test_lessonNeedsItsTaskOrASkip() {
        wizard.show("quick");
        wizard.next();
        wizard.next();
        compare(wizard.step, "lesson:typing");
        const next = findChild(wizard.contentItem, "wizardNext");
        verify(!next.enabled);
        verify(findChild(wizard.contentItem, "wizardSkipStep").visible);
        verify(wizard.lesson.check({
            text: "今天天氣很好",
            events: [],
            picks: []
        }));
        wizard.completed = {
            typing: true
        };
        verify(next.enabled);
        compare(findChild(wizard.contentItem, "wizardProgress").text, "打字與空白鍵 · 2 / 9");
    }
    function test_keySummaryFollowsSettings() {
        const groups = wizard.keyGroups();
        compare(groups[0].title, "打字");
        compare(groups[0].items[0].action, "送出並輸入空白");
        compare(groups[0].items[2].keys[0], "Shift");
        compare(groups[1].items[1].keys[0], "1–9");
        userSettings.setValue("shiftSwitch", false);
        compare(wizard.keyGroups()[0].items[2].keys[0], "Caps Lock");
    }
}
