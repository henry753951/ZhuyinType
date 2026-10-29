import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../qml"

TestCase {
    id: test
    name: "ShortcutRecorder"
    width: 700
    height: 700
    visible: true
    when: windowShown
    Theme {
        id: testTheme
        motionEnabled: false
    }
    // Exercise event delivery and backend responses here; ShortcutTest validates real storage and rules.
    QtObject {
        id: backend
        readonly property int maximumBindings: 5
        property bool acceptRecords: true
        property string error: ""
        property var calls: []
        property var values: ({})
        readonly property var initial: [
            {
                key: Qt.Key_QuoteLeft,
                modifiers: 0,
                label: "`"
            }
        ]
        readonly property var commands: [
            {
                id: "typeZhuyin",
                label: "打出注音本身",
                page: "choosing",
                row: "keys-typeZhuyin",
                defaults: initial,
                defaultsSpoken: "`"
            },
            {
                id: "nextPage",
                label: "下一頁",
                page: "appearance",
                row: "keys-nextPage",
                defaults: [],
                defaultsSpoken: "Tab"
            }
        ]
        property var conflictMap: ({})
        readonly property var conflicts: []
        readonly property bool customized: isCustomized("typeZhuyin")
        function conflictsOf(command) {
            return values[command] !== undefined ? conflictMap : {};
        }
        function modifierLabel(modifiers) {
            return modifiers & Qt.ShiftModifier ? "⇧" : "";
        }
        function activeLabel(command) {
            return (values[command] || [{ label: "" }])[0].label;
        }
        function record(command, key, modifiers, index) {
            calls = calls.concat([
                {
                    action: "record",
                    command: command,
                    key: key,
                    modifiers: modifiers,
                    index: index
                }
            ]);
            if (!acceptRecords) {
                error = "已用於「下一頁」。";
                return false;
            }
            const list = values[command].slice();
            const binding = {
                key: key,
                modifiers: modifiers,
                label: "新的按鍵"
            };
            if (index < 0)
                list.push(binding);
            else
                list[index] = binding;
            const next = Object.assign({}, values);
            next[command] = list;
            values = next;
            error = "";
            return true;
        }
        function remove(command, index) {
            calls = calls.concat([
                {
                    action: "remove",
                    command: command,
                    index: index
                }
            ]);
            const list = values[command].slice();
            list.splice(index, 1);
            const next = Object.assign({}, values);
            next[command] = list;
            values = next;
            return true;
        }
        function reset(command) {
            calls = calls.concat([
                {
                    action: "reset",
                    command: command
                }
            ]);
            values = {
                typeZhuyin: initial.slice(),
                nextPage: []
            };
            return true;
        }
        function isCustomized(command) {
            return command === "typeZhuyin" && JSON.stringify(values[command]) !== JSON.stringify(initial);
        }
    }
    Component {
        id: recorderComponent
        ShortcutRecorder {
            width: 320
            theme: testTheme
            shortcuts: backend
            command: "typeZhuyin"
            title: "打出注音本身"
        }
    }
    Component {
        id: overviewComponent
        ShortcutOverview {
            width: 640
            theme: testTheme
            shortcuts: backend
            settings: ({
                    values: {}
                })
        }
    }
    Button {
        id: outside
        focusPolicy: Qt.StrongFocus
        y: 550
        text: "其他控制項"
    }
    function init() {
        backend.calls = [];
        backend.error = "";
        backend.acceptRecords = true;
        backend.values = {
            typeZhuyin: backend.initial.slice(),
            nextPage: []
        };
        backend.conflictMap = {};
        failOnWarning(/.*(TypeError|ReferenceError|Binding loop|Unable to assign|Unable assign).*/);
    }
    function click(item) {
        verify(item);
        waitForRendering(item);
        // The field's controls show while the pointer is on it, as on macOS.
        mouseMove(item, item.width / 2, item.height / 2);
        mouseClick(item, item.width / 2, item.height / 2);
    }
    function makeRecorder() {
        const item = createTemporaryObject(recorderComponent, test);
        verify(item);
        waitForRendering(item);
        return item;
    }
    function test_boundOverviewIncludesFixedRows() {
        // Fixed rows instantiate hidden recorders with an empty command; their definition is undefined.
        const item = createTemporaryObject(overviewComponent, test);
        verify(item);
        waitForRendering(item);
        compare(item.shortcuts, backend);
        // macOS KeysPane: six sections; the fixed keys wait under 固定的按鍵 (sixteen of them).
        compare(item.matchingSections.length, 5);
        compare(item.matchingFixed.length, 16);
        verify(!findChild(item, "keyRow_commit"));
        const toggle = findChild(item, "fixedKeysToggle");
        toggle.forceActiveFocus();
        keyClick(Qt.Key_Space);
        tryVerify(() => !!findChild(item, "keyRow_commit"));
    }
    function test_missingDefinition_data() {
        return [
            {
                tag: "fixed-row",
                command: ""
            },
            {
                tag: "unknown-command",
                command: "unknown"
            }
        ];
    }
    function test_missingDefinition(data) {
        const item = createTemporaryObject(recorderComponent, test, {
            command: data.command
        });
        verify(item);
        compare(item.customized, false);
        compare(item.bindings.length, 0);
        verify(!findChild(item, "shortcutAdd").enabled);
        item.start(-1);
        verify(!item.recording);
    }
    function test_disconnectedBackend() {
        const item = createTemporaryObject(recorderComponent, test, {
            shortcuts: null
        });
        verify(item);
        compare(item.customized, false);
        compare(item.bindings.length, 0);
        verify(!findChild(item, "shortcutAdd").enabled);
    }
    function test_recordAndCancel() {
        const item = makeRecorder();
        click(findChild(item, "shortcutBinding0"));
        verify(item.recording);
        keyClick(Qt.Key_Escape);
        verify(!item.recording);
        compare(backend.calls.length, 0);
        click(findChild(item, "shortcutBinding0"));
        const control = Qt.platform.os === "osx" ? Qt.MetaModifier : Qt.ControlModifier;
        keyClick(Qt.Key_U, control);
        compare(backend.calls.length, 1);
        compare(backend.calls[0].command, "typeZhuyin");
        compare(backend.calls[0].key, Qt.Key_U);
        compare(backend.calls[0].modifiers, control);
        compare(backend.calls[0].index, 0);
        verify(!item.recording);
        verify(item.saved);
        verify(item.customized);
    }
    function test_restoreLinkAndConflictLine() {
        backend.values = {
            typeZhuyin: [{ key: Qt.Key_U, modifiers: Qt.AltModifier, label: "⌥U", spoken: "⌥ U" }],
            nextPage: []
        };
        backend.conflictMap = { "0": "nextPage" };
        const item = makeRecorder();
        const reset = findChild(item, "shortcutReset");
        verify(reset.visible);
        mouseMove(item, item.width - 4, 4);
        tryVerify(() => reset.opacity === 1);
        mouseClick(reset);
        compare(backend.calls[backend.calls.length - 1].action, "reset");
        verify(!item.customized);
    }
    function test_maxVisibleShowsMore() {
        backend.values = {
            typeZhuyin: [backend.initial[0], backend.initial[0], backend.initial[0]],
            nextPage: []
        };
        const item = createTemporaryObject(recorderComponent, test, { maxVisible: 2 });
        waitForRendering(item);
        verify(findChild(item, "shortcutBinding1"));
        verify(!findChild(item, "shortcutBinding2"));
        const more = findChild(item, "shortcutMore");
        verify(more);
        mouseMove(more, 2, 2);
        tryVerify(() => !!findChild(item, "shortcutBinding2"));
    }
    function test_rejectionKeepsRecordingAndCanCancel() {
        const item = makeRecorder();
        backend.acceptRecords = false;
        click(findChild(item, "shortcutBinding0"));
        keyClick(Qt.Key_Tab);
        compare(backend.calls.length, 1);
        compare(backend.calls[0].key, Qt.Key_Tab);
        verify(item.recording);
        compare(item.problem, backend.error);
        verify(!item.saved);
        compare(backend.values.typeZhuyin[0].key, Qt.Key_QuoteLeft);
        keyClick(Qt.Key_Escape);
        verify(!item.recording);
        compare(backend.calls.length, 1);
    }
    function test_modifierOnlyAndFocusLoss() {
        const item = makeRecorder();
        click(findChild(item, "shortcutAdd"));
        keyPress(Qt.Key_Shift);
        compare(backend.calls.length, 0);
        verify(item.recording);
        keyRelease(Qt.Key_Shift);
        click(outside);
        verify(!item.recording);
        compare(backend.calls.length, 0);
    }
    function test_addRemoveAndReset() {
        const item = makeRecorder();
        click(findChild(item, "shortcutAdd"));
        keyClick(Qt.Key_Tab);
        compare(backend.calls[0].index, -1);
        compare(backend.values.typeZhuyin.length, 2);
        const chip = findChild(item, "shortcutBinding1");
        verify(chip);
        chip.forceActiveFocus();
        keyClick(Qt.Key_Backspace);
        compare(backend.calls[1].action, "remove");
        compare(backend.calls[1].index, 1);
        compare(backend.values.typeZhuyin.length, 1);
        click(findChild(item, "shortcutBinding0"));
        keyClick(Qt.Key_U, Qt.AltModifier);
        click(findChild(item, "shortcutReset"));
        compare(backend.calls[backend.calls.length - 1].action, "reset");
        verify(!item.customized);
    }
    function test_fiveBindingsAndNarrowLayout() {
        backend.values = {
            typeZhuyin: [backend.initial[0], backend.initial[0], backend.initial[0], backend.initial[0], backend.initial[0]],
            nextPage: []
        };
        const item = makeRecorder();
        item.width = 140;
        waitForRendering(item);
        // At five keys there is no ＋ at all.
        verify(!findChild(item, "shortcutAdd"));
        for (let i = 0; i < 5; ++i) {
            const chip = findChild(item, "shortcutBinding" + i);
            verify(chip.x >= 0 && chip.x + chip.width <= item.width + 1);
        }
    }
    function test_disableCancelsRecording() {
        const item = makeRecorder();
        click(findChild(item, "shortcutBinding0"));
        item.enabled = false;
        verify(!item.recording);
        compare(backend.calls.length, 0);
    }
}
