pragma ComponentBehavior: Bound

import QtQuick
import QtTest
import "../qml"

TestCase {
    id: test
    name: "CandidateMotion"
    width: 700
    height: 680
    visible: true
    when: windowShown
    Theme {
        id: theme
    }
    QtObject {
        id: state
        property var entries: []
        readonly property var rows: entries.slice(page * pageSize, (page + 1) * pageSize)
        property int page: 0
        property int pageSize: 4
        readonly property int pageCount: Math.max(1, Math.ceil(entries.length / pageSize))
        property int selected: 0
        property int revision: 1
        property string reading: "ㄇㄧㄥˊ"
        property bool single: false
        property bool corrections: false
        property int committed: -1
        signal changed
        signal boundaryReached(int direction)
        function turnPage(delta) {
            const next = Math.max(0, Math.min(pageCount - 1, page + delta));
            if (next === page) {
                boundaryReached(delta > 0 ? 1 : -1);
                return;
            }
            page = next;
            selected = Math.min(selected, rows.length - 1);
            changed();
        }
        function pick(index) {
            if (index >= 0 && index < rows.length)
                committed = page * pageSize + index;
        }
        function moveSelection(delta) {
            selected = Math.max(0, Math.min(rows.length - 1, selected + delta));
            changed();
        }
        function toggleSingle() {
            single = !single;
            revision++;
            changed();
        }
        function toggleCorrections() {
            corrections = !corrections;
            revision++;
            changed();
        }
    }
    Component {
        id: component
        CandidatePanel {
            candidates: state
            theme: theme
            preferences: ({
                    candidateLayout: "vertical",
                    candidateDensity: "standard",
                    candidateFontSize: 18,
                    selectionKeys: "123456789",
                    showsAnnotations: true,
                    showsZhuyinHint: true,
                    showsListSwitches: true
                })
            width: implicitWidth
            height: implicitHeight
        }
    }
    function init() {
        failOnWarning(/.*(qml:|file:|TypeError|ReferenceError|Binding loop|Unable to assign|Cannot).*/);
        theme.motionEnabled = true;
        theme.reduceMotion = false;
        state.entries = ["曉明", "小明", "小名", "明", "名", "鳴", "銘", "冥", "茗"].map(text => ({
                    text: text,
                    annotation: ""
                }));
        state.page = 0;
        state.selected = 0;
        state.revision = 1;
        state.single = false;
        state.corrections = false;
        state.committed = -1;
    }
    function makePanel() {
        const panel = createTemporaryObject(component, test);
        verify(panel);
        waitForRendering(panel);
        return panel;
    }
    function test_twoPagesKeepHighlightAndCommitToIncomingPage() {
        const panel = makePanel();
        const highlight = findChild(panel, "candidateHighlight");
        const before = highlight.y;
        const height = panel.height;
        state.turnPage(1);
        verify(panel.pageAnimating);
        compare(panel.shownRows[0].text, "名");
        compare(panel.outgoingRows[0].text, "曉明");
        compare(highlight.y, before);
        compare(panel.height, height);
        const outgoing = findChild(panel, "outgoingCandidate0");
        verify(outgoing && !outgoing.enabled);
        const incoming = findChild(panel, "candidate0");
        verify(incoming);
        mouseClick(incoming);
        compare(state.committed, 4);
        verify(!panel.activeFocus, "The production panel must leave focus with the native client");
        tryCompare(panel, "pageAnimating", false);
        compare(panel.outgoingRows.length, 0);
    }
    // The highlight sits on the selected row and the rows never slide under it (a ListView current item would scroll).
    function test_highlightFollowsSelectionWithoutScrolling() {
        const panel = makePanel();
        const highlight = findChild(panel, "candidateHighlight");
        const list = findChild(panel, "candidateList");
        for (const index of [1, 3, 2, 0]) {
            state.selected = index;
            state.changed();
            waitForRendering(panel);
            const row = findChild(panel, "candidate" + index);
            compare(list.contentY, 0);
            compare(highlight.y, row.y + 1);
            verify(highlight.visible);
        }
    }
    function test_rapidPageChangesReplaceOutgoingSnapshot() {
        const panel = makePanel();
        state.turnPage(1);
        state.turnPage(1);
        compare(panel.shownRows[0].text, "茗");
        compare(panel.outgoingRows[0].text, "名");
        compare(state.page, 2);
        state.turnPage(-1);
        compare(panel.outgoingRows[0].text, "茗");
        compare(panel.shownRows[0].text, "名");
        tryCompare(panel, "pageAnimating", false);
        compare(panel.outgoingRows.length, 0);
    }
    function test_reducedMotionFadesAndBoundaryReturns() {
        theme.reduceMotion = true;
        const panel = makePanel();
        state.turnPage(1);
        const list = findChild(panel, "candidateList");
        const clip = findChild(panel, "candidateClip");
        compare(list.mapToItem(clip, 0, 0).y, 0);
        verify(list.opacity < 1);
        tryCompare(panel, "pageAnimating", false);
        state.turnPage(1);
        tryCompare(panel, "pageAnimating", false);
        state.turnPage(1);
        verify(panel.bouncing);
        wait(70);
        compare(list.mapToItem(clip, 0, 0).y, 0);
        verify(list.opacity < 1);
        tryCompare(panel, "bouncing", false);
        compare(list.opacity, 1);
    }
    function test_motionOffAndListReplacementCancelOldText() {
        const panel = makePanel();
        state.turnPage(1);
        theme.motionEnabled = false;
        compare(panel.pageAnimating, false);
        compare(panel.outgoingRows.length, 0);
        state.turnPage(1);
        compare(panel.pageAnimating, false);
        state.turnPage(1);
        compare(panel.bouncing, false);
        theme.motionEnabled = true;
        state.turnPage(-1);
        state.entries = [
            {
                text: "新字",
                annotation: ""
            }
        ];
        state.page = 0;
        state.revision++;
        state.changed();
        compare(panel.outgoingRows.length, 0);
        compare(panel.shownRows[0].text, "新字");
    }
    function test_disabledFeaturesHideSwitchesAndNoticeOnlyIsCompact() {
        const panel = makePanel();
        panel.preferences = Object.assign({}, panel.preferences, {
            "feature.singleCharacterList": false,
            "feature.correctionsList": false
        });
        verify(!findChild(panel, "singleButton").visible);
        verify(!findChild(panel, "correctionsButton").visible);
        state.entries = [];
        state.revision++;
        state.changed();
        panel.hint = "已加入自訂詞：小明";
        verify(!panel.hasFooter);
        tryVerify(() => panel.height < 70);
    }
}
