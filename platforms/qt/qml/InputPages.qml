pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts

// 打字 and 選字 with their sub-pages (標點與符號, 所有打字的按鍵, 模糊音, 前文), laid out as the macOS version lays them out
// (TypingPane, ChoosingPane, PunctuationPage, TypingKeysPage, FuzzyPage, ContextPane in SettingsPages.swift). The
// settings' titles and descriptions come from settings.json through SettingRow; the keys the texts name are read
// from `shortcuts`. Rows carry the macOS row ids as anchors, so links land on them.
ColumnLayout {
    id: page
    required property var settings
    required property Theme theme
    property string pageId: "typing"
    property var appInfo: ({})
    property var shortcuts: null
    /// The input controller, for the 前文 numbers (`contextRead`, `contextKept`, `contextWindow` in seconds), the
    /// constants of ControllerCore on macOS. Until it exposes them the macOS values are used.
    property var inputController: null
    signal navigateRequested(string pageId, string title, string row)
    // The shell can bind the platform's reduced-motion preference here.
    property bool reduceMotion: theme.reduceMotion
    readonly property bool motionEnabled: !reduceMotion && theme.motionEnabled && values.panelAnimation !== false
    property string infoKey: ""
    onPageIdChanged: information.close()
    spacing: theme.sectionSpacing

    readonly property var values: settings.values
    readonly property int contextRead: page.constant("contextRead", 128)
    readonly property int contextKept: page.constant("contextKept", 255)
    readonly property int contextMinutes: Math.round(page.constant("contextWindow", 600) / 60)
    readonly property int fuzzyCount: ["zZh", "cCh", "sSh", "enEng", "anAng"].filter(key => values["fuzzy." + key]).length
    readonly property int punctuationOff: ["controlPunctuation", "optionSymbols"].filter(key => values["feature." + key] === false).length
    readonly property var overviewLink: ({
            title: "在快捷鍵總覽中查看全部按鍵",
            page: "keys",
            destination: "快捷鍵"
        })

    function constant(name, fallback) {
        const value = inputController ? Number(inputController[name]) : NaN;
        return isFinite(value) && value > 0 ? value : fallback;
    }
    /// A command's first key ("Shift ＋ ↑"), or "" when its feature is off or it has no key.
    function activeKey(command) {
        if (!shortcuts)
            return "";
        const known = shortcuts.commands.find(item => item.id === command);
        if (known && known.feature && values[known.feature] === false)
            return "";
        const bound = shortcuts.values[command] || [];
        return bound.length > 0 ? bound[0].label : "";
    }
    /// A key label as separate key caps: "Shift ＋ ↑" → ["Shift", "↑"].
    function caps(label) {
        return label ? label.split(" ＋ ") : [];
    }

    // MARK: The pages' sections. A row is a setting { key }, a link { link, title, detail, value, target, row }, a
    // feature without a switch { title, detail, commands }, or a fixed fact { title, detail, keys | value }.

    readonly property var typingSections: [
        {
            title: "空白與 Esc",
            rows: [
                {
                    key: "spaceAction",
                    info: "spaceAction"
                },
                {
                    key: "escape",
                    info: "escape"
                }
            ]
        },
        {
            title: "標點",
            rows: [
                {
                    key: "punctuationKeys",
                    info: "punctuationKeys"
                },
                {
                    link: "punctuation",
                    title: "標點與符號",
                    detail: "Ctrl 加符號鍵打中文標點、Alt 加符號鍵打鍵上的符號",
                    value: punctuationOff === 0 ? "全部開啟" : "已關閉 " + punctuationOff + " 項"
                }
            ]
        },
        {
            title: "中英切換",
            rows: [
                {
                    key: "shiftSwitch"
                },
                {
                    key: "shiftToggle",
                    hiddenWhenOff: "shiftSwitch",
                    indent: true
                },
                {
                    key: "feature.capsLockEnglish"
                },
                {
                    link: "appearance",
                    row: "modeToast",
                    title: "中英提示",
                    detail: "切換時在游標旁顯示「中」或「英」",
                    value: values.showModeToast === false ? "不顯示" : "顯示"
                },
                {
                    key: "startInEnglish"
                }
            ]
        },
        {
            title: "編輯組字",
            footerLink: overviewLink,
            rows: [
                {
                    title: "送出並換行",
                    detail: "送出組字，App 再換行",
                    commands: ["commitWithNewline"],
                    anchor: "newline"
                },
                {
                    key: "feature.deleteWord"
                },
                {
                    link: "typingKeys",
                    title: "所有打字的按鍵",
                    detail: "注音鍵、聲調、Backspace、方向鍵等固定的按鍵",
                    value: ""
                }
            ]
        }
    ]
    readonly property var choosingSections: [
        {
            title: "轉換",
            rows: [
                {
                    key: "candidateAnchor",
                    info: "candidateAnchor"
                },
                {
                    key: "characterForms"
                }
            ]
        },
        {
            title: "按錯鍵",
            rows: [
                {
                    key: "typoTolerance",
                    info: "typoTolerance"
                },
                {
                    key: "readingChannel"
                },
                {
                    key: "feature.correctionsList"
                },
                {
                    link: "fuzzy",
                    anchor: "fuzzy",
                    title: "模糊音",
                    detail: "ㄗ／ㄓ、ㄣ／ㄥ 等容易混的音",
                    value: fuzzyCount === 0 ? "關閉" : "已開啟 " + fuzzyCount + " 項"
                }
            ]
        },
        {
            title: "前文",
            rows: [
                {
                    link: "context",
                    title: "參考游標前的文字",
                    detail: "依前文選字；只在記憶體中",
                    value: values.usesPrecedingText === false ? "關閉" : "開啟"
                }
            ]
        },
        {
            title: "改字",
            footerLink: overviewLink,
            rows: [
                {
                    key: "feature.typeZhuyin"
                },
                {
                    key: "feature.selectPhrase"
                },
                {
                    key: "feature.deleteAfterCommit"
                }
            ]
        }
    ]
    readonly property var punctuationSections: [
        {
            title: "可以關閉的按鍵",
            rows: [
                {
                    key: "feature.controlPunctuation"
                },
                {
                    key: "feature.optionSymbols"
                }
            ]
        },
        {
            title: "固定的按鍵",
            rows: [
                {
                    title: "「 」 、",
                    detail: "固定",
                    keys: ["[", "]", "'"]
                },
                {
                    title: "列出同類的標點",
                    detail: "在標點上按",
                    keys: ["↓"]
                },
                {
                    title: "在組字中放一個空白",
                    detail: "固定",
                    keys: ["⇧", "空白"]
                }
            ]
        }
    ]
    readonly property var typingKeySections: [
        {
            title: "",
            footer: "這些按鍵用在打字本身，無法更改。",
            rows: [
                {
                    title: "拼注音",
                    detail: "注音鍵的位置依標準注音鍵盤",
                    keys: ["注音鍵"]
                },
                {
                    title: "聲調：二聲、三聲、四聲、輕聲",
                    detail: "標準注音鍵盤的聲調鍵",
                    keys: ["ˊ 6", "ˇ 3", "ˋ 4", "˙ 7"]
                },
                {
                    title: "一聲",
                    detail: "打注音時，空白完成這個音",
                    keys: ["空白"]
                },
                {
                    title: "送出",
                    detail: "送出整段組字",
                    keys: ["Return"]
                },
                {
                    title: "刪掉注音符號，或游標前的字",
                    detail: "和一般的刪除鍵相同",
                    keys: ["⌫"]
                },
                {
                    title: "刪掉游標後的字",
                    detail: "和一般的刪除鍵相同",
                    keys: ["Delete"]
                },
                {
                    title: "在組字中移動游標",
                    detail: "Home、End 移到開頭或結尾",
                    keys: ["←", "→"]
                },
                {
                    title: "在組字中放一個空白",
                    detail: "空白本身用在一聲與送出",
                    keys: ["⇧", "空白"]
                }
            ]
        }
    ]
    readonly property var contextSections: [
        {
            title: "",
            rows: [
                {
                    key: "usesPrecedingText"
                }
            ]
        },
        {
            title: "讀取與保留",
            rows: [
                {
                    title: "讀取",
                    value: values.usesPrecedingText === false ? "不讀取" : "游標前最多 " + contextRead + " 個字"
                },
                {
                    title: "送出的字",
                    value: values.usesPrecedingText === false ? "只接著這個輸入框剛送出的字" : "接在前文後面，保留最近 " + contextKept + " 個字"
                },
                {
                    title: "App 不提供前文時",
                    value: "沿用同一個 App " + contextMinutes + " 分鐘內送出的字",
                    hiddenWhenOff: "usesPrecedingText"
                },
                {
                    title: "密碼欄位",
                    value: "不讀取，也不學習"
                },
                {
                    title: "存放位置",
                    value: "只在記憶體中"
                }
            ]
        }
    ]
    readonly property var sections: pageId === "typing" ? typingSections : pageId === "choosing" ? choosingSections : pageId === "typingKeys" ? typingKeySections : pageId === "punctuation" ? punctuationSections : pageId === "context" ? contextSections : []

    function definition(key) {
        return settings.definitions.find(item => item.key === key) || ({
                key: key,
                title: key,
                kind: "toggle"
            });
    }

    // MARK: Demos (OptionDemos.swift). A segment is { text, style }; see DemoLine.

    readonly property var start: ({
            text: "早安，",
            style: "document"
        })
    function seg(text, style) {
        return {
            text: text,
            style: style || "marked"
        };
    }
    function spaceFrames(action) {
        const s = start;
        let ending;
        if (action === "candidates")
            ending = [{ keys: ["空白"], line: [s, seg("今天", "focus")], list: ["今天", "金田", "今"], caption: "再按一次：開啟候選字視窗", hold: 2000 }];
        else if (action === "insert")
            // The space joins the composition (still underlined), typing goes on, and Return sends it all.
            ending = [
                { keys: ["空白"], line: [s, seg("今天 ")], caption: "再按一次：空白加進組字，還不送出", hold: 1500 },
                { keys: ["ㄏ"], line: [s, seg("今天 "), seg("ㄏ")], caption: "繼續打字", hold: 500 },
                { keys: ["ˇ"], line: [s, seg("今天 好")], caption: "繼續打字", hold: 1000 },
                { keys: ["Return"], line: [seg("早安，今天 好", "document")], caption: "按 Return：連同空白一起送出", hold: 2000 }
            ];
        else
            ending = [{ keys: ["空白"], line: [seg("早安，今天 ", "document")], caption: "再按一次：送出並輸入空白", hold: 2000 }];
        return [
            { keys: ["ㄊ"], line: [s, seg("今"), seg("ㄊ")], caption: "打 ㄐㄧㄣ ㄊㄧㄢ", hold: 350 },
            { keys: ["ㄢ"], line: [s, seg("今"), seg("ㄊㄧㄢ")], caption: "打 ㄐㄧㄣ ㄊㄧㄢ", hold: 1000 },
            { keys: ["空白"], line: [s, seg("今天")], caption: "完成一聲", hold: 1200 }
        ].concat(ending);
    }
    function escapeFrames(clears) {
        return [
            { keys: ["ㄧ"], line: [start, seg("今天"), seg("ㄑㄧ")], caption: "打到一半", hold: 1100 },
            { keys: ["Esc"], line: [start, seg("今天")], caption: "清除注音", hold: 1200 },
            clears ? { keys: ["Esc"], line: [start], caption: "再按一次：清除整段組字", hold: 2000 } : { keys: ["Esc"], line: [start, seg("今天")], caption: "再按一次：組字保留", hold: 2000 }
        ];
    }
    function anchorFrames(right) {
        return [
            { line: [seg("今天"), seg("天氣")], caption: "游標在句尾：改最後的詞", hold: 1400 },
            { keys: ["←"], line: [seg("今天"), seg("天", right ? "marked" : "target"), seg("氣", right ? "target" : "marked")], caret: 1, caption: "游標在句中：底線標出要改的字", hold: 1600 },
            { keys: ["↓"], line: [seg("今天"), seg("天", right ? "marked" : "focus"), seg("氣", right ? "focus" : "marked")], caret: 1, list: right ? ["氣", "器", "汽"] : ["天", "添", "田"], caption: right ? "改游標右邊的字" : "改游標左邊的字", hold: 2000 }
        ];
    }
    /// `correctionsKey`: "" when the corrections list is off, and the demo ends at the correction.
    function typoFrames(correctionsKey) {
        const frames = [
            { keys: ["ㄍ"], line: [start, seg("ㄍ")], caption: "想打 ㄐ，按到旁邊的 ㄍ", hold: 1200 },
            { keys: ["ㄣ"], line: [start, seg("ㄍㄧㄣ")], caption: "想打 ㄐ，按到旁邊的 ㄍ", hold: 900 },
            { keys: ["空白"], line: [start, seg("今")], caption: "自動改成「今」", hold: 1400 }
        ];
        if (correctionsKey)
            frames.push({ keys: caps(correctionsKey), line: [start, seg("今", "focus")], list: ["今 ㄐㄧㄣ", "金 ㄐㄧㄣ", "斤 ㄐㄧㄣ"], caption: "列出修正的字", hold: 2000 });
        return frames;
    }
    readonly property var fuzzyFrames: {
        const before = seg("今天很", "document");
        return [
            { keys: ["ㄙ"], line: [before, seg("ㄙ")], caption: "打 ㄙㄨ ㄈㄨˊ", hold: 500 },
            { keys: ["ㄨ"], line: [before, seg("ㄙㄨ")], caption: "打 ㄙㄨ ㄈㄨˊ", hold: 500 },
            { keys: ["ㄈ"], line: [before, seg("ㄙㄨ"), seg("ㄈ")], caption: "打 ㄙㄨ ㄈㄨˊ", hold: 400 },
            { keys: ["ㄨ"], line: [before, seg("ㄙㄨ"), seg("ㄈㄨ")], caption: "打 ㄙㄨ ㄈㄨˊ", hold: 700 },
            { keys: ["ˊ"], line: [before, seg("舒服")], caption: "開啟「ㄙ 與 ㄕ」：也打得出「舒服」", hold: 2200 }
        ];
    }

    // MARK: Pages

    // 標點與符號: the table of what Shift, Ctrl and Alt with a symbol key type.
    SectionCard {
        visible: page.pageId === "punctuation"
        theme: page.theme
        padded: true
        title: "標點按法：" + (page.values.punctuationKeys === "mac" ? "macOS" : "Windows")
        PunctuationTable {}
    }
    PlaceLink {
        visible: page.pageId === "punctuation"
        Layout.topMargin: -page.theme.sectionSpacing + page.theme.headerGap
        title: "標點按法在「打字」設定"
        target: "typing"
        destination: "打字"
        row: "punctuationKeys"
    }

    // 模糊音: the demo, then the five pairs.
    SectionCard {
        visible: page.pageId === "fuzzy"
        theme: page.theme
        padded: true
        footer: "開啟的兩個音互相通用；兩個詞同樣常用時，讀音完全符合的排在前面。"
        MiniDemo {
            theme: page.theme
            frames: page.fuzzyFrames
        }
    }
    SettingsPage {
        visible: page.pageId === "fuzzy"
        Layout.fillWidth: true
        pageId: "fuzzy"
        settings: page.settings
        shortcuts: page.shortcuts
        theme: page.theme
    }

    // 前文: the illustration first.
    SectionCard {
        visible: page.pageId === "context"
        theme: page.theme
        padded: true
        ContextIllustration {}
    }

    Repeater {
        model: page.sections
        delegate: ColumnLayout {
            id: section
            required property var modelData
            Layout.fillWidth: true
            spacing: 0
            SectionCard {
                theme: page.theme
                title: section.modelData.title
                footer: section.modelData.footer || ""
                Repeater {
                    model: section.modelData.rows
                    delegate: Item {
                        id: entry
                        required property var modelData
                        required property int index
                        readonly property bool collapsed: !!modelData.hiddenWhenOff && page.values[modelData.hiddenWhenOff] === false
                        property bool ready: false
                        objectName: modelData.key || modelData.title
                        Layout.fillWidth: true
                        implicitHeight: collapsed ? 0 : entryBody.implicitHeight
                        visible: implicitHeight > 0
                        enabled: !collapsed
                        clip: true
                        onCollapsedChanged: ready = true
                        Behavior on implicitHeight {
                            enabled: entry.ready && page.motionEnabled
                            NumberAnimation {
                                duration: 200
                                easing.type: Easing.InOutQuad
                            }
                        }
                        ColumnLayout {
                            id: entryBody
                            width: parent.width
                            spacing: 0
                            opacity: entry.collapsed ? 0 : 1
                            Behavior on opacity {
                                enabled: page.motionEnabled
                                NumberAnimation {
                                    duration: 150
                                }
                            }
                            Loader {
                                Layout.fillWidth: true
                                sourceComponent: entry.modelData.key ? settingRow : entry.modelData.link ? linkRow : factRow
                            }
                            // A feature's keys: one indented row per command, while the feature is on.
                            Repeater {
                                readonly property var commands: entry.modelData.commands || (entry.modelData.key ? page.definition(entry.modelData.key).commands || [] : [])
                                model: page.shortcuts && (!entry.modelData.key || page.values[entry.modelData.key] !== false) ? commands : []
                                delegate: SettingsRow {
                                    id: keyRow
                                    required property string modelData
                                    required property int index
                                    readonly property var all: entry.modelData.commands || page.definition(entry.modelData.key).commands
                                    theme: page.theme
                                    separated: true
                                    indent: true
                                    anchor: "keys-" + modelData
                                    title: all.length > 1 ? (page.shortcuts.commands.find(command => command.id === modelData)?.label || modelData) : "快捷鍵"
                                    ShortcutRecorder {
                                        Layout.preferredWidth: 200
                                        shortcuts: page.shortcuts
                                        command: keyRow.modelData
                                        title: keyRow.title
                                        theme: page.theme
                                        rowHovered: keyRow.hovered
                                    }
                                }
                            }
                        }
                        Component {
                            id: settingRow
                            SettingRow {
                                id: setting
                                definition: page.definition(entry.modelData.key)
                                settings: page.settings
                                shortcuts: page.shortcuts
                                theme: page.theme
                                separated: entry.index > 0
                                indent: !!entry.modelData.indent
                                info: !!entry.modelData.info
                                onInfoClicked: {
                                    page.infoKey = entry.modelData.info;
                                    page.information.show(setting);
                                }
                            }
                        }
                        Component {
                            id: linkRow
                            NavigationRow {
                                theme: page.theme
                                separated: entry.index > 0
                                anchor: entry.modelData.anchor || ""
                                title: entry.modelData.title
                                detail: entry.modelData.detail || ""
                                value: entry.modelData.value || ""
                                onActivated: page.navigateRequested(entry.modelData.link, entry.modelData.title, entry.modelData.row || "")
                            }
                        }
                        // A fixed fact: its keys as key caps, or its value.
                        Component {
                            id: factRow
                            SettingsRow {
                                theme: page.theme
                                separated: entry.index > 0
                                anchor: entry.modelData.anchor || ""
                                title: entry.modelData.title
                                subtitle: entry.modelData.detail || ""
                                value: entry.modelData.value || ""
                                KeyCapFlow {
                                    visible: !!entry.modelData.keys
                                    theme: page.theme
                                    keys: entry.modelData.keys || []
                                }
                            }
                        }
                    }
                }
            }
            PlaceLink {
                visible: !!section.modelData.footerLink
                Layout.topMargin: page.theme.headerGap
                title: section.modelData.footerLink ? section.modelData.footerLink.title : ""
                target: section.modelData.footerLink ? section.modelData.footerLink.page : ""
                destination: section.modelData.footerLink ? section.modelData.footerLink.destination : ""
            }
        }
    }

    // MARK: Pieces

    /// A link to a place in the settings window, under a card (PlaceLink on macOS).
    component PlaceLink: Row {
        id: link
        property string title: ""
        property string target: ""
        property string destination: ""
        property string row: ""
        Layout.leftMargin: page.theme.cardInset
        spacing: 2
        activeFocusOnTab: true
        Accessible.role: Accessible.Link
        Accessible.name: title
        Keys.onReturnPressed: page.navigateRequested(target, destination, row)
        Keys.onSpacePressed: page.navigateRequested(target, destination, row)
        Text {
            text: link.title
            color: link.activeFocus || hoverLink.hovered ? page.theme.accentHover : page.theme.accent
            font.pixelSize: page.theme.detailSize
            font.underline: hoverLink.hovered
        }
        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: "chevronRight"
            size: 10
            weight: 2
            color: page.theme.accent
        }
        HoverHandler {
            id: hoverLink
            cursorShape: Qt.PointingHandCursor
        }
        TapHandler {
            onTapped: page.navigateRequested(link.target, link.destination, link.row)
        }
    }

    /// What Shift, Ctrl and Alt with a symbol key type, for both 標點按法 settings (PunctuationTable on macOS).
    component PunctuationTable: ColumnLayout {
        id: table
        readonly property bool windows: page.values.punctuationKeys !== "mac"
        readonly property var rows: [
            { keys: ["⇧", ","], windows: "<", mac: "，" },
            { keys: ["⇧", "/"], windows: "?", mac: "？" },
            { keys: ["⇧", "1"], windows: "!", mac: "！" },
            { keys: ["⇧", "-"], windows: "_", mac: "——" },
            { keys: ["Ctrl", ","], windows: "，", mac: "，" },
            { keys: ["Alt", ","], windows: ",", mac: "," }
        ]
        Layout.fillWidth: true
        spacing: 10
        GridLayout {
            columns: 3
            columnSpacing: 18
            rowSpacing: 6
            Text {
                text: "按鍵"
                color: page.theme.secondary
                font.pixelSize: page.theme.detailSize
            }
            Repeater {
                model: [["Windows", true], ["macOS", false]]
                delegate: Text {
                    required property var modelData
                    readonly property bool current: modelData[1] === table.windows
                    Layout.minimumWidth: 64
                    text: modelData[0]
                    color: current ? page.theme.accent : page.theme.secondary
                    font.pixelSize: page.theme.detailSize
                    font.weight: current ? Font.DemiBold : Font.Normal
                }
            }
            Rectangle {
                Layout.columnSpan: 3
                Layout.fillWidth: true
                implicitHeight: 1
                color: page.theme.line
            }
            Repeater {
                model: table.rows.length * 3
                delegate: Loader {
                    id: cell
                    required property int index
                    readonly property var row: table.rows[Math.floor(index / 3)]
                    readonly property int column: index % 3
                    sourceComponent: column === 0 ? keysCell : outputCell
                    Component {
                        id: keysCell
                        Row {
                            spacing: 3
                            Repeater {
                                model: cell.row.keys
                                delegate: KeyCap {
                                    required property string modelData
                                    theme: page.theme
                                    small: true
                                    text: modelData
                                }
                            }
                        }
                    }
                    Component {
                        id: outputCell
                        Text {
                            readonly property bool current: (cell.column === 1) === table.windows
                            text: cell.column === 1 ? cell.row.windows : cell.row.mac
                            color: current ? page.theme.text : page.theme.tertiary
                            font.pixelSize: page.theme.headingSize
                            font.weight: current ? Font.Medium : Font.Normal
                        }
                    }
                }
            }
        }
        Text {
            Layout.fillWidth: true
            text: "Ctrl 加符號鍵打中文標點、Alt 加符號鍵打鍵上的符號，兩種按法都能用。"
            color: page.theme.secondary
            font.pixelSize: page.theme.detailSize
            wrapMode: Text.Wrap
        }
    }

    /// Four short scenes (ContextIllustration on macOS): the text before the cursor being read, what was sent
    /// carrying over to another field of the same app, and a password field that is never read.
    component ContextIllustration: ColumnLayout {
        id: story
        objectName: "contextStory"
        readonly property var holds: [2400, 2600, 3200, 2800]
        readonly property var captions: ["開始組字時，讀取游標前最多 " + page.contextRead + " 個字", "接著打的字，依前文選字", "App 不提供前文時，沿用同一個 App " + page.contextMinutes + " 分鐘內送出的字", "密碼欄位：不讀取，也不學習"]
        property int scene: 0
        property bool playing: page.motionEnabled
        Layout.fillWidth: true
        spacing: 10
        Accessible.role: Accessible.Animation
        Accessible.name: captions.join("。")
        Timer {
            interval: story.holds[story.scene]
            running: story.visible && story.playing && page.motionEnabled
            repeat: true
            onTriggered: story.scene = (story.scene + 1) % story.holds.length
        }
        Item {
            Layout.fillWidth: true
            implicitHeight: 116
            // The text read before the cursor, in a note.
            ColumnLayout {
                visible: story.scene <= 1
                width: parent.width
                spacing: 6
                Text {
                    Layout.leftMargin: 12
                    text: "↙ 最多 " + page.contextRead + " 個字"
                    color: page.theme.accent
                    font.pixelSize: page.theme.captionSize
                    font.weight: Font.Medium
                }
                DemoCard {
                    title: "備忘錄"
                    DemoLine {
                        theme: page.theme
                        fontSize: page.theme.bodySize + 1
                        blinks: page.motionEnabled
                        segments: [{ text: "讀書會改到星期四晚上，地點一樣在", style: "highlight" }].concat(story.scene === 1 ? [{ text: "圖書館", style: "marked" }] : [])
                    }
                }
            }
            // What was sent in one field carries over to another of the same app.
            RowLayout {
                visible: story.scene === 2
                width: parent.width
                spacing: 10
                DemoCard {
                    title: "輸入框 A"
                    DemoLine {
                        theme: page.theme
                        fontSize: page.theme.bodySize
                        showsCaret: false
                        segments: [{ text: "地點一樣在圖書館", style: "highlight" }]
                    }
                }
                Column {
                    spacing: 2
                    Icon {
                        anchors.horizontalCenter: parent.horizontalCenter
                        name: "chevronRight"
                        size: 15
                        weight: 2.2
                        color: page.theme.accent
                    }
                    Text {
                        text: page.contextMinutes + " 分鐘內"
                        color: page.theme.accent
                        font.pixelSize: page.theme.captionSize
                    }
                }
                DemoCard {
                    title: "輸入框 B"
                    DemoLine {
                        theme: page.theme
                        fontSize: page.theme.bodySize
                        blinks: page.motionEnabled
                        segments: [{ text: "七點見", style: "marked" }]
                    }
                }
            }
            // A password field: never read, nothing learnt.
            DemoCard {
                visible: story.scene === 3
                y: 18
                width: parent.width
                title: "密碼"
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Text {
                        text: "••••••••"
                        color: page.theme.text
                        font.pixelSize: page.theme.bodySize + 1
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                    Chip {
                        theme: page.theme
                        text: "不讀取"
                    }
                    Chip {
                        theme: page.theme
                        text: "不學習"
                    }
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Text {
                Layout.fillWidth: true
                text: story.captions[story.scene]
                color: page.theme.secondary
                font.pixelSize: page.theme.detailSize
                elide: Text.ElideRight
            }
            DemoControls {
                theme: page.theme
                count: story.holds.length
                current: story.scene
                playing: story.playing
                playEnabled: page.motionEnabled
                stepNames: story.captions
                onPlayToggled: story.playing = !story.playing
                onStepChosen: index => {
                    story.playing = false;
                    story.scene = index;
                }
            }
        }
    }

    /// A small labelled field inside the 前文 illustration.
    component DemoCard: Rectangle {
        id: card
        property string title: ""
        default property alias content: cardBody.data
        Layout.fillWidth: true
        implicitHeight: cardBody.implicitHeight + 20
        radius: 8
        color: page.theme.field
        border.color: page.theme.line
        ColumnLayout {
            id: cardBody
            x: 12
            y: 10
            width: parent.width - 24
            spacing: 8
            Text {
                text: card.title
                color: page.theme.secondary
                font.pixelSize: page.theme.captionSize
            }
        }
    }

    // MARK: The ⓘ cards

    property alias information: popover
    InfoPopover {
        id: popover
        theme: page.theme
        title: page.infoKey ? page.definition(page.infoKey).title : ""
        onClosed: page.infoKey = ""
        Loader {
            Layout.fillWidth: true
            active: popover.opened || popover.visible
            sourceComponent: page.infoKey === "punctuationKeys" ? punctuationCard : page.infoKey === "typoTolerance" ? typoCard : choicesCard
        }
    }
    // Every choice with its own demo; the current one is marked.
    Component {
        id: choicesCard
        ColumnLayout {
            spacing: 14
            Text {
                visible: page.infoKey === "spaceAction"
                Layout.fillWidth: true
                text: "打注音時，空白是一聲；打完以後，空白依這個設定。清單開著時，空白翻到下一頁；選字用選字鍵或 Return。"
                color: page.theme.secondary
                font.pixelSize: page.theme.detailSize
                wrapMode: Text.Wrap
            }
            Repeater {
                model: page.infoKey ? page.definition(page.infoKey).options || [] : []
                delegate: ChoiceDemo {
                    required property var modelData
                    theme: page.theme
                    title: modelData.label
                    selected: page.values[page.infoKey] === modelData.value
                    frames: page.infoKey === "spaceAction" ? page.spaceFrames(modelData.value) : page.infoKey === "escape" ? page.escapeFrames(modelData.value === "composition") : page.anchorFrames(modelData.value === "after")
                }
            }
        }
    }
    Component {
        id: typoCard
        ColumnLayout {
            readonly property string correctionsKey: page.activeKey("correctionsList")
            spacing: 12
            MiniDemo {
                theme: page.theme
                frames: page.typoFrames(parent.correctionsKey)
                restingFrame: 2
            }
            GridLayout {
                Layout.fillWidth: true
                columns: 2
                columnSpacing: 8
                rowSpacing: 6
                Repeater {
                    model: page.definition("typoTolerance").options.length * 2
                    delegate: Text {
                        required property int index
                        readonly property var option: page.definition("typoTolerance").options[Math.floor(index / 2)]
                        readonly property bool current: page.values.typoTolerance === option.value
                        Layout.fillWidth: index % 2 === 1
                        Layout.minimumWidth: index % 2 === 0 ? 34 : 0
                        text: index % 2 === 0 ? option.label : (page.definition("typoTolerance").details || {})[option.value] || ""
                        color: index % 2 === 0 ? page.theme.text : page.theme.secondary
                        font.pixelSize: page.theme.detailSize
                        font.weight: index % 2 === 0 && current ? Font.DemiBold : Font.Normal
                        wrapMode: Text.Wrap
                    }
                }
            }
            Text {
                Layout.fillWidth: true
                text: parent.correctionsKey ? "照按鍵打出的字優先；修正過的字也在 " + parent.correctionsKey + " 的清單中。" : "照按鍵打出的字優先。"
                color: page.theme.secondary
                font.pixelSize: page.theme.detailSize
                wrapMode: Text.Wrap
            }
        }
    }
    Component {
        id: punctuationCard
        PunctuationTable {}
    }
}
