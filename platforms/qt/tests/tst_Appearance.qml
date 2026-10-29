pragma ComponentBehavior: Bound

import QtQuick
import QtTest
import "../qml"

TestCase {
    id: test
    name: "AppearancePreview"
    width: 800
    height: 1200
    visible: true
    when: windowShown
    Theme {
        id: theme
    }
    QtObject {
        id: settings
        property var definitions: []
        property var values: ({})
        function setValue(key, value) {
            values = Object.assign({}, values, {
                [key]: value
            });
        }
    }
    component Fixture: QtObject {
        readonly property var entries: ["曉明", "小明", "小名", "明", "名", "鳴", "銘", "冥", "茗"].map(text => ({
                    text: text,
                    annotation: ""
                }))
        readonly property var rows: entries.slice(page * pageSize, (page + 1) * pageSize)
        property int pageSize: 9
        property int page: 0
        readonly property int pageCount: Math.ceil(entries.length / pageSize)
        property int selected: 0
        property string reading: "ㄒㄧㄠˇ ㄇㄧㄥˊ"
        property bool single: false
        property bool corrections: false
        signal changed
        onPageSizeChanged: {
            page = 0;
            changed();
        }
        function turnPage(delta) {
            page = Math.max(0, Math.min(pageCount - 1, page + delta));
            changed();
        }
        function pick(index) {
            selected = index;
            changed();
        }
        function moveSelection(delta) {
            selected = Math.max(0, Math.min(rows.length - 1, selected + delta));
            changed();
        }
        function toggleSingle() {
            single = !single;
            changed();
        }
        function toggleCorrections() {
            corrections = !corrections;
            changed();
        }
    }
    Fixture {
        id: candidates
    }
    Fixture {
        id: corrections
    }
    Component {
        id: component
        AppearancePage {
            settings: settings
            candidates: candidates
            correctionCandidates: corrections
            theme: theme
            width: 415
        }
    }
    function init() {
        failOnWarning(/.*(qml:|file:|TypeError|ReferenceError|Binding loop|Unable to assign|Cannot).*/);
        theme.reduceMotion = false;
        theme.motionEnabled = true;
        settings.values = {
            candidateLayout: "vertical",
            candidateDensity: "standard",
            candidateFontSize: 18,
            candidatesPerPage: 9,
            candidatePlacement: "below",
            selectionKeys: "123456789",
            showsAnnotations: true,
            showsZhuyinHint: true,
            showsListSwitches: true,
            showModeToast: true,
            panelAnimation: true
        };
        candidates.page = 0;
        corrections.page = 0;
    }
    function makePage(width) {
        const page = createTemporaryObject(component, test, {
            width: width || 415
        });
        verify(page);
        waitForRendering(page);
        return page;
    }
    function test_sixStatesUseSharedPanelAndFitNarrow_data() {
        return [
            {
                tag: "narrow",
                width: 415
            },
            {
                tag: "wide",
                width: 644
            }
        ];
    }
    function test_sixStatesUseSharedPanelAndFitNarrow(data) {
        const page = makePage(data.width);
        const stage = findChild(page, "appearanceStage");
        const panel = findChild(page, "appearanceCandidates");
        compare(page.states.length, 6);
        for (const state of page.states) {
            const tab = findChild(page, "sceneTab_" + state.id);
            mouseClick(tab);
            compare(page.scene, state.id);
            waitForRendering(stage);
            verify(stage.x >= 0 && stage.x + stage.width <= page.width + 1);
            if (page.listScene) {
                verify(panel.visible);
                const right = panel.mapToItem(stage, panel.width, 0).x;
                verify(right <= stage.width, "The real panel must fit the stage at native size");
            }
        }
    }
    function test_pagingUsesRealTransitionAndSettingsSelectScene() {
        const page = makePage();
        compare(candidates.pageSize, 4);
        page.scene = "paging";
        page.frame = 0;
        page.advance();
        compare(candidates.page, 1);
        const panel = findChild(page, "appearanceCandidates");
        verify(panel.pageAnimating);
        settings.setValue("showsAnnotations", false);
        compare(page.scene, "corrections");
        settings.setValue("showModeToast", false);
        compare(page.scene, "mode");
        settings.setValue("candidatesPerPage", 6);
        compare(candidates.pageSize, 6);
        compare(page.pageLimit, 6);
    }
    function test_reducedMotionStopsAutomaticPlayback() {
        const page = makePage();
        const timer = findChild(page, "appearancePlayback");
        verify(timer);
        timer.interval = 40;
        const before = page.scene;
        tryVerify(() => page.scene !== before);
        theme.reduceMotion = true;
        const stoppedScene = page.scene;
        const stoppedFrame = page.frame;
        wait(180);
        compare(page.scene, stoppedScene);
        compare(page.frame, stoppedFrame);
        // macOS has no play button: the pointer over the preview does not pause it either.
        verify(!findChild(page, "appearancePlay"));
    }
}
