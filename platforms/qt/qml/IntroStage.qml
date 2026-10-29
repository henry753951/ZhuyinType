pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts

// The 介紹 page's story (macOS IntroStage.swift): a document where typing plays in a loop — Zhuyin typed key by key, the
// sentence converts, ↓ opens the list, the corrections list, a pick, a phrase added, the text sent, the 中/英 badge —
// drawn with the real CandidatePanel and the current settings and keys. The key just pressed sits beside the caret.
// With reduced motion it rests on the open list until played. Playback never writes to settings or the engine.
ColumnLayout {
    id: stage
    required property Theme theme
    /// Settings values (Settings.values).
    required property var values
    /// Shortcuts.values, or null: the keys the steps show.
    property var shortcutValues: null
    /// Show one step and stay there (renders, tests); -1 plays.
    property int pinned: -1
    property bool playing: pinned < 0 && !theme.reduceMotion
    property int index: 0
    readonly property int restingStep: 13
    readonly property var steps: script()
    readonly property int current: Math.min(pinned >= 0 ? pinned : index, steps.length - 1)
    readonly property var step: steps[current]
    readonly property bool above: values.candidatePlacement === "above"
    readonly property real lineSize: 16
    readonly property real lineHeight: Math.ceil(lineSize * 1.3)
    readonly property real margin: 20
    readonly property real gap: 4
    spacing: 8
    objectName: "introStage"
    Accessible.role: Accessible.Animation
    Accessible.name: "候選字視窗預覽"

    Component.onCompleted: {
        if (!playing)
            index = restingStep;
    }
    onPlayingChanged: {
        if (!playing)
            index = restingStep;
    }

    // The keys of a command as keycaps, or null when the command is off or has no key.
    function caps(command, feature) {
        if (feature && values[feature] === false)
            return null;
        const list = shortcutValues ? shortcutValues[command] : null;
        if (!list || list.length === 0)
            return null;
        const label = list[0].label;
        return label.indexOf("+") > 0 ? label.split("+") : [label];
    }
    function modeKey() {
        if (values.shiftSwitch)
            return values.shiftToggle === "left" ? "左 Shift" : values.shiftToggle === "right" ? "右 Shift" : "Shift";
        return values["feature.capsLockEnglish"] !== false ? "Caps Lock" : "";
    }
    function segment(text, focus) {
        return {
            text: text,
            focus: !!focus
        };
    }
    function script() {
        const s = "週末約了";
        const typing = "打注音：ㄨㄤˊ ㄒㄧㄠˇ ㄇㄧㄥˊ";
        const w = segment("王");
        const keys = String(values.selectionKeys || "123456789");
        const pick = keys.length > 1 ? keys.charAt(1).toUpperCase() : "2";
        const corrections = caps("correctionsList", "feature.correctionsList");
        const selectLeft = caps("selectLeft", "feature.selectPhrase");
        const mode = modeKey();
        const vertical = values.candidateLayout !== "horizontal";
        function at(marked, extra) {
            return Object.assign({
                committed: s,
                marked: marked,
                list: "",
                notice: "",
                toast: "",
                keys: [],
                caption: typing,
                hold: 0.16
            }, extra);
        }
        const list = [at([], {
                caption: "在文件中打字",
                hold: 1.0
            }), at([segment("ㄨ")], {
                keys: ["ㄨ"],
                hold: 0.18
            }), at([segment("ㄨㄤ")], {
                keys: ["ㄤ"],
                hold: 0.18
            }), at([w], {
                keys: ["ˊ"],
                hold: 0.3
            }), at([w, segment("ㄒ")], {
                keys: ["ㄒ"]
            }), at([w, segment("ㄒㄧ")], {
                keys: ["ㄧ"]
            }), at([w, segment("ㄒㄧㄠ")], {
                keys: ["ㄠ"],
                hold: 0.18
            }), at([w, segment("小")], {
                keys: ["ˇ"],
                hold: 0.3
            }), at([w, segment("小"), segment("ㄇ")], {
                keys: ["ㄇ"]
            }), at([w, segment("小"), segment("ㄇㄧ")], {
                keys: ["ㄧ"]
            }), at([w, segment("小"), segment("ㄇㄧㄥ")], {
                keys: ["ㄥ"],
                hold: 0.18
            }), at([w, segment("曉明")], {
                keys: ["ˊ"],
                caption: "整句隨打隨選",
                hold: 1.3
            }), at([w, segment("曉明", true)], {
                list: "normal:0",
                keys: ["↓"],
                caption: values.spaceAction === "candidates" ? "按 ↓ 或空白，開啟候選字視窗" : "按 ↓，開啟候選字視窗",
                hold: 1.4
            }), at([w, segment("曉明", true)], {
                list: "normal:1",
                keys: [vertical ? "↓" : "→"],
                caption: "移動反白",
                hold: 1.0
            })];
        if (corrections)
            list.push(at([w, segment("曉明", true)], {
                list: "tolerant:0",
                keys: corrections,
                caption: "列出按錯修正的字",
                hold: 1.8
            }), at([w, segment("曉明", true)], {
                list: "normal:1",
                keys: corrections,
                caption: "再按一次，回到一般清單",
                hold: 1.0
            }));
        list.push(at([w, segment("小明")], {
            keys: [pick],
            caption: "選字",
            hold: 1.2
        }));
        if (selectLeft)
            list.push(at([w, segment("小"), segment("明", true)], {
                notice: "Enter 加入自訂詞：明",
                keys: selectLeft,
                caption: "選取字詞",
                hold: 0.45
            }), at([w, segment("小明", true)], {
                notice: "Enter 加入自訂詞：小明",
                keys: selectLeft,
                caption: "選取字詞",
                hold: 0.45
            }), at([segment("王小明", true)], {
                notice: "Enter 加入自訂詞：王小明",
                keys: selectLeft,
                caption: "選取字詞",
                hold: 1.4
            }), at([segment("王小明")], {
                notice: "已加入自訂詞：王小明",
                keys: ["Return"],
                caption: "加入自訂詞，下次直接打得出",
                hold: 1.6
            }));
        list.push(at([], {
            committed: s + "王小明",
            keys: ["Return"],
            caption: "送出",
            hold: 1.0
        }));
        if (values.showModeToast !== false && mode)
            list.push(at([], {
                committed: s + "王小明",
                toast: "英",
                keys: [mode],
                caption: "按 " + mode + " 切換中英",
                hold: 1.6
            }));
        list.push(at([], {
            committed: s + "王小明",
            caption: "",
            hold: 0.8
        }));
        return list;
    }

    // The steps' lists, as the CandidatePanel's model (LiveInput's candidateState, with fixed rows).
    QtObject {
        id: demoList
        readonly property var words: ["曉明", "小明", "小名", "明", "名", "鳴", "銘", "冥", "茗", "溟", "螟", "暝", "瞑", "酩"]
        property string kind: ""
        readonly property bool tolerant: kind.indexOf("tolerant") === 0
        readonly property var all: (tolerant ? [{
                    text: "校名",
                    annotation: "ㄒㄧㄠˋ ㄇㄧㄥˊ"
                }] : []).concat(words.map(word => ({
                    text: word,
                    annotation: ""
                })))
        readonly property int pageSize: Math.max(1, stage.values.candidatesPerPage || 9)
        readonly property int page: 0
        readonly property int pageCount: Math.ceil(all.length / pageSize)
        readonly property var rows: kind ? all.slice(0, pageSize) : []
        readonly property int selected: kind ? Number(kind.split(":")[1]) : -1
        readonly property string reading: "ㄒㄧㄠˇ ㄇㄧㄥˊ"
        readonly property bool single: false
        readonly property bool corrections: tolerant
        signal changed
        signal boundaryReached(int direction)
        function pick(row) {
        }
        function turnPage(delta) {
        }
        function moveSelection(delta) {
        }
        function toggleSingle() {
        }
        function toggleCorrections() {
        }
        onKindChanged: changed()
    }
    // The tallest list of the story, measured once, so the stage never changes height while it plays.
    QtObject {
        id: tallest
        readonly property var all: demoList.words.map(word => ({
                    text: word,
                    annotation: ""
                }))
        readonly property int pageSize: demoList.pageSize
        readonly property int page: 0
        readonly property int pageCount: demoList.pageCount + 1
        readonly property var rows: [{
                text: "校名",
                annotation: "ㄒㄧㄠˋ ㄇㄧㄥˊ"
            }].concat(all).slice(0, pageSize)
        readonly property int selected: 0
        readonly property string reading: demoList.reading
        readonly property bool single: false
        readonly property bool corrections: true
        signal changed
        function pick(row) {
        }
        function turnPage(delta) {
        }
        function moveSelection(delta) {
        }
        function toggleSingle() {
        }
        function toggleCorrections() {
        }
    }
    Binding {
        target: demoList
        property: "kind"
        value: stage.step.list
    }

    Timer {
        // The story's clock: each step holds for its own time.
        running: stage.playing && stage.visible && stage.pinned < 0
        interval: Math.round(stage.step.hold * 1000)
        repeat: false
        onTriggered: {
            stage.index = (stage.index + 1) % stage.steps.length;
            restart();
        }
    }

    Rectangle {
        id: documentView
        Layout.fillWidth: true
        readonly property real scale: Math.min(1, Math.max(0.3, (width - 16) / Math.max(1, measure.implicitWidth)))
        readonly property real panelHeight: measure.implicitHeight * scale
        readonly property real lineTop: stage.above ? height - 18 - stage.lineHeight : 18
        implicitHeight: 18 + stage.lineHeight + stage.gap + panelHeight + 22
        radius: 10
        color: stage.theme.field
        border.color: stage.theme.line
        clip: true

        CandidatePanel {
            id: measure
            visible: false
            candidates: tallest
            preferences: stage.values
            theme: stage.theme
            maximumWidth: 4000
        }
        // A few grey lines of text, kept clear of the typed line.
        Column {
            x: 20
            y: 14
            width: parent.width - 40
            spacing: 11
            Repeater {
                model: [0.92, 0.74, 0.86, 0.55, 0.8, 0.66, 0.9, 0.5, 0.78, 0.6, 0.84, 0.7]
                delegate: Rectangle {
                    required property real modelData
                    width: parent.width * modelData
                    height: 7
                    radius: 3.5
                    color: stage.theme.line
                    opacity: 0.8
                }
            }
        }
        Rectangle {
            x: 1
            y: documentView.lineTop - 5
            width: parent.width - 2
            height: 30
            color: stage.theme.field
        }
        // The line: sent text, then the composition, underlined, with the focused part tinted.
        DemoLine {
            id: line
            x: stage.margin
            y: documentView.lineTop
            theme: stage.theme
            fontSize: stage.lineSize
            blinks: stage.pinned < 0
            segments: [{
                    text: stage.step.committed,
                    style: "document"
                }].concat(stage.step.marked.map(part => ({
                        text: part.text,
                        style: part.focus ? "focus" : "marked"
                    })))
        }
        TextMetrics {
            id: committedText
            font.pixelSize: stage.lineSize
            text: stage.step.committed
        }
        // The key just pressed, beside the caret.
        Row {
            id: pressed
            visible: stage.step.keys.length > 0
            x: Math.min(line.x + line.width + 9, documentView.width - width - 8)
            y: documentView.lineTop - 1
            spacing: 3
            Repeater {
                model: stage.step.keys
                delegate: KeyCap {
                    required property string modelData
                    theme: stage.theme
                    text: modelData
                }
            }
            onVisibleChanged: if (visible)
                press.restart()
            Connections {
                target: stage
                function onCurrentChanged() {
                    press.restart();
                }
            }
            SequentialAnimation {
                id: press
                NumberAnimation {
                    target: pressed
                    property: "scale"
                    to: stage.theme.motionEnabled && !stage.theme.reduceMotion && stage.pinned < 0 ? 0.88 : 1
                    duration: 60
                }
                NumberAnimation {
                    target: pressed
                    property: "scale"
                    to: 1
                    duration: 140
                    easing.type: Easing.OutBack
                }
            }
        }
        // The real candidate window, lined up with the composition, kept inside the stage; drawn smaller when wider.
        Item {
            id: panelSlot
            readonly property real start: line.x + committedText.advanceWidth
            visible: stage.step.list !== "" || stage.step.notice !== ""
            x: Math.min(Math.max(start - 6 * documentView.scale, 8), Math.max(8, documentView.width - width - 8))
            y: stage.above ? documentView.lineTop - stage.gap - height : documentView.lineTop + stage.lineHeight + stage.gap
            width: panel.implicitWidth * documentView.scale
            height: panel.implicitHeight * documentView.scale
            opacity: visible ? 1 : 0
            Behavior on opacity {
                enabled: stage.theme.motionEnabled && stage.pinned < 0
                NumberAnimation {
                    duration: 160
                    easing.type: Easing.OutCubic
                }
            }
            CandidatePanel {
                id: panel
                transformOrigin: Item.TopLeft
                scale: documentView.scale
                width: implicitWidth
                height: implicitHeight
                candidates: demoList
                hint: stage.step.notice
                preferences: stage.values
                theme: stage.theme
                maximumWidth: 4000
            }
        }
        // The 中/英 badge.
        Rectangle {
            visible: stage.step.toast !== ""
            x: Math.min(line.x + line.width, documentView.width - 60)
            y: stage.above ? documentView.lineTop - stage.gap - height : documentView.lineTop + stage.lineHeight + stage.gap
            width: 52
            height: 48
            radius: stage.theme.radius
            color: stage.theme.surface
            border.color: stage.theme.fieldBorder
            Text {
                anchors.centerIn: parent
                text: stage.step.toast
                color: stage.theme.text
                font.pixelSize: stage.theme.titleSize
            }
        }
        Text {
            visible: documentView.scale < 0.99
            anchors {
                top: parent.top
                right: parent.right
                topMargin: 8
                rightMargin: 44
            }
            text: "縮小為 " + Math.round(documentView.scale * 100) + "%"
            color: stage.theme.secondary
            font.pixelSize: stage.theme.captionSize
        }
        IconButton {
            objectName: "introPlayPause"
            visible: stage.pinned < 0
            anchors {
                top: parent.top
                right: parent.right
                margins: 8
            }
            theme: stage.theme
            iconName: stage.playing ? "pause" : "play"
            Accessible.name: stage.playing ? "暫停" : "播放"
            onClicked: stage.playing = !stage.playing
        }
    }
    Text {
        Layout.fillWidth: true
        Layout.minimumHeight: implicitHeight
        text: stage.step.caption || " "
        horizontalAlignment: Text.AlignHCenter
        color: stage.theme.secondary
        font.pixelSize: stage.theme.detailSize
        wrapMode: Text.Wrap
    }
}
