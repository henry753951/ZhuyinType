import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The setup wizard's practice field (macOS PracticeField): a field typed through its own engine and input controller
// on the wizard's sandbox settings, the real candidate window under it, and the lesson's task above it. It reports
// what happened (`outcome`) for the task's check; `done` turns the field's border green.
ColumnLayout {
    id: live
    required property var engineSession
    required property var inputController
    required property var settings
    required property Theme theme
    /// The key bindings the candidate keycaps show; null shows the defaults.
    property var shortcuts: null
    /// The task, e.g. 打出「今天天氣很好」; empty: a plain 試打 field.
    property string prompt: ""
    /// The Zhuyin to type, under the task.
    property string hint: ""
    property bool done: false
    /// Text already in the field (the 前文 lesson).
    property string initialText: ""
    /// What happened: { text, sent, events: ["open", "page", "corrections", "mode", "phrase", "pick"], picks: [] }.
    property var outcome: ({
            text: "",
            sent: "",
            events: [],
            picks: []
        })
    // The page's rows before a key, to tell which candidate a pick took.
    property var lastRows: []
    property int lastPage: 0
    property bool lastCorrections: false
    property bool lastEnglish: false
    function note(event, pick) {
        const next = Object.assign({}, outcome);
        next.text = document.text + (engineSession.composing ? engineSession.composition : "");
        next.sent = document.text;
        if (event)
            next.events = outcome.events.concat([event]);
        if (pick)
            next.picks = outcome.picks.concat([pick]);
        outcome = next;
    }
    function clearOutcome() {
        outcome = {
            text: "",
            sent: "",
            events: [],
            picks: []
        };
    }
    Component.onCompleted: {
        document.text = initialText;
        document.cursorPosition = document.length;
        lastEnglish = inputController.english;
    }
    Connections {
        target: live.engineSession
        function onChanged() {
            if (!live.trialActive)
                return;
            const rows = live.engineSession.rows || [];
            if (rows.length > 0 && live.lastRows.length === 0)
                live.note("open");
            if (rows.length > 0 && live.engineSession.page !== live.lastPage && live.lastRows.length > 0)
                live.note("page");
            if (live.engineSession.corrections && !live.lastCorrections)
                live.note("corrections");
            if ((live.engineSession.notice || "").indexOf("已加入自訂詞") === 0)
                live.note("phrase");
            if (rows.length > 0)
                live.lastRows = rows.slice(live.engineSession.page * live.engineSession.pageSize, (live.engineSession.page + 1) * live.engineSession.pageSize).map(row => row.text);
            else
                live.lastRows = [];
            live.lastPage = live.engineSession.page;
            live.lastCorrections = live.engineSession.corrections;
            live.note("");
        }
    }
    Connections {
        target: live.inputController
        ignoreUnknownSignals: true
        function onModeChanged() {
            if (live.inputController.english !== live.lastEnglish)
                live.note("mode");
            live.lastEnglish = live.inputController.english;
        }
        function onCandidateChosen() {
            // The candidate now in the text: the one of the page's rows that the composition or sent text took.
            const now = document.text + (live.engineSession.composing ? live.engineSession.composition : "");
            const taken = live.lastRows.filter(word => now.indexOf(word) >= 0).sort((a, b) => b.length - a.length);
            live.note("pick", taken.length ? taken[0] : "");
        }
    }
    readonly property bool trialActive: document.activeFocus && document.visible && document.Window.window !== null && document.Window.window.visible && document.Window.window.active
    spacing: 10
    Connections {
        target: live.engineSession
        function onCommittedText(text) {
            if (live.trialActive) {
                document.insert(document.cursorPosition, text);
                live.note("");
            }
        }
    }
    QtObject {
        id: candidateState
        readonly property var rows: live.engineSession.rows.slice(page * pageSize, (page + 1) * pageSize)
        readonly property int page: live.engineSession.page
        readonly property int pageSize: live.engineSession.pageSize
        readonly property int pageCount: live.engineSession.pageCount
        readonly property int selected: live.engineSession.selected < 0 ? -1 : live.engineSession.selected % pageSize
        readonly property string reading: live.engineSession.reading
        readonly property bool single: live.engineSession.single
        readonly property bool corrections: live.engineSession.corrections
        signal changed
        signal boundaryReached(int direction)
        property Connections boundaries: Connections {
            target: live.inputController
            ignoreUnknownSignals: true
            function onCandidateBoundaryReached(direction) {
                if (live.trialActive)
                    candidateState.boundaryReached(direction);
            }
        }
        property Connections relay: Connections {
            target: live.engineSession
            function onChanged() {
                candidateState.changed();
            }
        }
        function pick(row) {
            if (live.trialActive)
                live.inputController.choose(page * pageSize + row);
        }
        function turnPage(delta) {
            if (live.trialActive)
                live.engineSession.key(delta > 0 ? Qt.Key_PageDown : Qt.Key_PageUp, 0);
        }
        function toggleSingle() {
            if (live.trialActive)
                live.engineSession.key(Qt.Key_Down, Qt.ShiftModifier);
        }
        function toggleCorrections() {
            if (live.trialActive)
                live.engineSession.key(Qt.Key_Up, Qt.ShiftModifier);
        }
    }
    RowLayout {
        Layout.fillWidth: true
        spacing: 8
        Icon {
            visible: live.prompt.length > 0
            name: live.done ? "checkCircle" : "typing"
            size: 16
            color: live.done ? live.theme.success : live.theme.secondary
        }
        Text {
            objectName: "practicePrompt"
            Layout.fillWidth: live.prompt.length > 0
            text: live.prompt || "試打"
            color: live.theme.text
            font.pixelSize: live.prompt ? live.theme.bodySize : live.theme.headingSize
            font.weight: Font.DemiBold
            wrapMode: Text.Wrap
        }
        Chip {
            visible: live.done
            theme: live.theme
            text: "完成"
            tone: "success"
        }
        Chip {
            theme: live.theme
            text: live.inputController.english ? "英文" : "中文"
            tone: live.inputController.english ? "neutral" : "accent"
        }
        Item {
            visible: live.prompt.length === 0
            Layout.fillWidth: true
        }
        AppButton {
            objectName: "clearTrial"
            theme: live.theme
            kind: "plain"
            compact: true
            text: "清除"
            focusPolicy: Qt.NoFocus
            enabled: live.trialActive
            onClicked: {
                if (live.trialActive) {
                    live.engineSession.reset();
                    document.clear();
                    live.note("");
                }
            }
        }
    }
    Text {
        visible: live.hint.length > 0
        Layout.fillWidth: true
        Layout.topMargin: -6
        Layout.leftMargin: live.prompt.length > 0 ? 24 : 0
        text: live.hint
        color: live.theme.secondary
        font.pixelSize: live.theme.detailSize
        wrapMode: Text.Wrap
    }
    AppTextArea {
        id: document
        objectName: "liveDocument"
        theme: live.theme
        Layout.fillWidth: true
        Layout.preferredHeight: 130
        enabled: live.engineSession.ready
        placeholderText: live.prompt ? "在這裡試打" : "在這裡輸入注音，使用目前的設定與詞庫"
        inputMethodHints: Qt.ImhNoPredictiveText
        Keys.priority: Keys.BeforeItem
        Keys.onPressed: event => {
            if (live.settings.values.usesPrecedingText)
                live.engineSession.setPrecedingText(text.slice(0, cursorPosition));
            event.accepted = live.inputController.press(event.key, event.modifiers, event.isAutoRepeat);
        }
        Keys.onReleased: event => live.inputController.release(event.key, event.modifiers)
        onActiveFocusChanged: {
            if (!activeFocus)
                live.inputController.deactivate();
        }
    }
    Text {
        objectName: "trialComposition"
        visible: live.trialActive && live.engineSession.composing
        text: live.engineSession.composition
        textFormat: Text.PlainText
        color: live.theme.text
        font.pixelSize: live.theme.displaySize
        font.underline: true
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        Accessible.name: "組字：" + text
    }
    CandidatePanel {
        objectName: "trialCandidates"
        visible: live.trialActive && candidateState.rows.length > 0
        candidates: candidateState
        shortcuts: live.shortcuts
        preferences: live.settings.values
        theme: live.theme
        maximumWidth: live.width
        Layout.preferredWidth: implicitWidth
        Layout.preferredHeight: implicitHeight
        onReadingClicked: if (live.trialActive)
            live.engineSession.key(Qt.Key_QuoteLeft, 0)
    }
    Text {
        visible: live.trialActive && text.length > 0
        text: live.engineSession.error || live.engineSession.notice
        color: live.engineSession.error ? live.theme.danger : live.theme.secondary
        font.pixelSize: live.theme.detailSize
        Layout.fillWidth: true
        wrapMode: Text.Wrap
    }
}
