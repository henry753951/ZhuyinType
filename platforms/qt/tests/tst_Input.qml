pragma ComponentBehavior: Bound

import QtQuick
import QtTest
import "../qml"

TestCase {
    id: test
    name: "InputPages"
    width: 700
    height: 950
    visible: true
    when: windowShown

    Theme {
        id: testTheme
    }
    QtObject {
        id: testSettings
        property var values: ({})
        // Only the control contract is mocked; the page and its real shared controls are exercised.
        property var definitions: {
            const choices = {
                spaceAction: ["commit", "insert", "candidates"],
                escape: ["composition", "reading"],
                punctuationKeys: ["windows", "mac"],
                shiftToggle: ["both", "left", "right"],
                candidateAnchor: ["after", "before"],
                characterForms: ["common", "standard"],
                typoTolerance: ["standard", "off", "aggressive"]
            };
            const toggles = ["shiftSwitch", "startInEnglish", "showModeToast", "readingChannel", "usesPrecedingText", "panelAnimation", "feature.capsLockEnglish", "feature.deleteWord", "feature.correctionsList", "feature.typeZhuyin", "feature.selectPhrase", "feature.deleteAfterCommit", "feature.controlPunctuation", "feature.optionSymbols", "fuzzy.zZh", "fuzzy.cCh", "fuzzy.sSh", "fuzzy.enEng", "fuzzy.anAng"];
            return Object.keys(choices).map(key => ({
                        key: key,
                        title: key,
                        kind: "choice",
                        default: choices[key][0],
                        options: choices[key].map(value => ({
                                    value: value,
                                    label: value
                                }))
                    })).concat(toggles.map(key => ({
                        key: key,
                        title: key,
                        kind: "toggle",
                        default: !key.startsWith("fuzzy."),
                        page: key.startsWith("fuzzy.") ? "fuzzy" : "",
                        group: "模糊音"
                    })));
        }
        function setValue(key, value) {
            const next = Object.assign({}, values);
            next[key] = value;
            values = next;
            return true;
        }
    }
    Component {
        id: inputPage
        InputPages {
            settings: testSettings
            theme: testTheme
            width: 423
        }
    }

    function init() {
        failOnWarning(/.*(TypeError|ReferenceError|Cannot read|Binding loop).*/);
        testTheme.motionEnabled = true;
        testTheme.appearance = "light";
        const defaults = {};
        for (const definition of testSettings.definitions)
            defaults[definition.key] = definition.default;
        testSettings.values = defaults;
    }

    function test_routes_data() {
        const rows = [];
        for (const route of ["typing", "choosing", "punctuation", "typingKeys", "fuzzy", "context"])
            for (const width of [423, 640])
                for (const appearance of ["light", "dark"])
                    rows.push({
                        tag: route + width + appearance,
                        route: route,
                        width: width,
                        appearance: appearance
                    });
        return rows;
    }

    function test_routes(data) {
        testTheme.appearance = data.appearance;
        const view = createTemporaryObject(inputPage, test, {
            pageId: data.route,
            width: data.width
        });
        verify(view);
        verify(waitForRendering(view));
        verify(isFinite(view.implicitHeight) && view.implicitHeight > 0);
        compare(view.width, data.width);
    }

    function test_collapseKeepsDelegateAndHonorsMotion() {
        const view = createTemporaryObject(inputPage, test);
        verify(waitForRendering(view));
        const shift = findChild(view, "shiftToggle");
        verify(shift && shift.implicitHeight > 0);
        testSettings.setValue("shiftSwitch", false);
        compare(findChild(view, "shiftToggle"), shift);
        tryCompare(shift, "implicitHeight", 0);
        compare(shift.enabled, false);
        testSettings.setValue("shiftSwitch", true);
        tryVerify(() => shift.implicitHeight > 0);
        view.reduceMotion = true;
        testSettings.setValue("shiftSwitch", false);
        compare(view.motionEnabled, false);
        tryCompare(shift, "implicitHeight", 0);
        view.reduceMotion = false;
        testSettings.setValue("panelAnimation", false);
        compare(view.motionEnabled, false);
        testSettings.setValue("panelAnimation", true);
        testTheme.motionEnabled = false;
        compare(view.motionEnabled, false);
    }

    function demos(item) {
        const found = [];
        const visit = node => {
            if (node.objectName === "miniDemo" && node.visible)
                found.push(node);
            for (const child of node.children || [])
                visit(child);
        };
        visit(item);
        return found;
    }

    // An ⓘ card shows every choice with its own demo, the current one marked, next to the ⓘ; Esc closes it.
    function test_infoCardStacksChoices() {
        const view = createTemporaryObject(inputPage, test);
        verify(waitForRendering(view));
        testSettings.setValue("spaceAction", "insert");
        view.infoKey = "spaceAction";
        view.information.show(view);
        tryCompare(view.information, "opened", true);
        verify(waitForRendering(view.information.contentItem));
        let shown = test.demos(view.information.contentItem);
        compare(shown.length, 3);
        compare(shown.map(demo => demo.frames.length).join(), "4,7,4");
        compare(shown[1].frames[6].caption, "按 Return：連同空白一起送出");
        for (const key of ["escape", "candidateAnchor"]) {
            view.infoKey = key;
            verify(waitForRendering(view.information.contentItem));
            shown = test.demos(view.information.contentItem);
            compare(shown.length, 2);
            verify(shown.every(demo => demo.frame.caption.length > 0));
        }
        view.infoKey = "typoTolerance";
        verify(waitForRendering(view.information.contentItem));
        shown = test.demos(view.information.contentItem);
        compare(shown.length, 1);
        // Without the corrections key the demo ends at the correction.
        compare(shown[0].frames.length, 3);
        compare(shown[0].frames[2].caption, "自動改成「今」");
        keyClick(Qt.Key_Escape);
        tryCompare(view.information, "visible", false);
    }

    function test_fuzzyDemoTypesEveryKey() {
        const view = createTemporaryObject(inputPage, test, {
            pageId: "fuzzy"
        });
        verify(waitForRendering(view));
        const demo = test.demos(view)[0];
        verify(demo);
        compare(demo.frames.map(frame => frame.keys.join("")).join(""), "ㄙㄨㄈㄨˊ");
        compare(demo.frames[4].caption, "開啟「ㄙ 與 ㄕ」：也打得出「舒服」");
    }

    function test_contextClockAndReducedMotion() {
        const view = createTemporaryObject(inputPage, test, {
            pageId: "context"
        });
        verify(waitForRendering(view));
        const story = findChild(view, "contextStory");
        verify(story);
        compare(story.captions[0], "開始組字時，讀取游標前最多 128 個字");
        tryVerify(() => story.scene > 0, 4000);
        view.reduceMotion = true;
        const scene = story.scene;
        wait(3300);
        compare(story.scene, scene);
    }
}
