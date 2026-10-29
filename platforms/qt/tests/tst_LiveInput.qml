pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../qml"

TestCase {
    id: test
    name: "TrialFocusIsolation"
    width: 700
    height: 900
    visible: true
    when: windowShown
    Theme {
        id: theme
    }
    QtObject {
        id: settings
        property var values: ({
                usesPrecedingText: true,
                candidateLayout: "vertical",
                candidateDensity: "standard",
                candidateFontSize: 18,
                selectionKeys: "123456789",
                showsAnnotations: true,
                showsZhuyinHint: true,
                showsListSwitches: true
            })
    }
    QtObject {
        id: engine
        property bool ready: true
        property bool composing: true
        property string composition: "試打組字"
        property string reading: "ㄕˋ"
        property var rows: [
            {
                text: "試",
                annotation: ""
            }
        ]
        property int page: 0
        property int pageSize: 9
        property int pageCount: 1
        property int selected: 0
        property bool single: false
        property bool corrections: false
        property string notice: ""
        property string error: ""
        property int resets: 0
        property int preceding: 0
        property int keys: 0
        signal committedText(string text)
        signal changed
        function setPrecedingText(text) {
            preceding++;
        }
        function reset() {
            resets++;
        }
        function key(key, modifiers) {
            keys++;
        }
    }
    QtObject {
        id: controller
        property bool english: false
        property int presses: 0
        property int picks: 0
        property int deactivations: 0
        function press(key, modifiers, repeat) {
            presses++;
            return true;
        }
        function release(key, modifiers) {
        }
        function choose(index) {
            picks++;
        }
        function deactivate() {
            deactivations++;
        }
    }
    TextField {
        id: outside
        y: 820
        width: 200
        placeholderText: "其他設定欄位"
    }
    Component {
        id: component
        LiveInput {
            engineSession: engine
            inputController: controller
            settings: settings
            theme: theme
            width: 420
        }
    }
    function init() {
        failOnWarning(/.*(qml:|file:|TypeError|ReferenceError|Binding loop|Unable to assign|Cannot).*/);
        outside.forceActiveFocus();
        engine.resets = 0;
        engine.preceding = 0;
        engine.keys = 0;
        controller.presses = 0;
        controller.picks = 0;
        controller.deactivations = 0;
    }
    function makeTrial() {
        const trial = createTemporaryObject(component, test);
        verify(trial);
        waitForRendering(trial);
        return trial;
    }
    function test_nativeCommitsNeverReachInactiveTrial() {
        const trial = makeTrial();
        const document = findChild(trial, "liveDocument");
        outside.forceActiveFocus();
        engine.committedText("原生輸入範例");
        compare(document.text, "");
        verify(!findChild(trial, "trialComposition").visible);
        verify(!findChild(trial, "trialCandidates").visible);
        const clear = findChild(trial, "clearTrial");
        verify(!clear.enabled);
        clear.clicked();
        compare(engine.resets, 0);
        compare(engine.keys, 0);
        compare(engine.preceding, 0);
    }
    function test_onlyFocusedTrialReceivesCommitsAndKeys() {
        const first = makeTrial();
        const second = makeTrial();
        const firstDocument = findChild(first, "liveDocument");
        const secondDocument = findChild(second, "liveDocument");
        firstDocument.forceActiveFocus();
        engine.committedText("第一個");
        compare(firstDocument.text, "第一個");
        compare(secondDocument.text, "");
        verify(findChild(first, "trialCandidates").visible);
        keyClick(Qt.Key_A);
        compare(controller.presses, 1);
        compare(engine.preceding, 1);
        secondDocument.forceActiveFocus();
        engine.committedText("第二個");
        compare(firstDocument.text, "第一個");
        compare(secondDocument.text, "第二個");
        outside.forceActiveFocus();
        engine.committedText("其他欄位");
        compare(secondDocument.text, "第二個");
        compare(engine.resets, 0);
    }
    function test_hiddenTrialLeavesEngineUntouched() {
        const trial = makeTrial();
        const document = findChild(trial, "liveDocument");
        document.forceActiveFocus();
        trial.visible = false;
        const calls = controller.deactivations;
        engine.committedText("外部欄位");
        engine.changed();
        compare(document.text, "");
        compare(engine.resets, 0);
        compare(engine.keys, 0);
        compare(controller.deactivations, calls);
    }
}
