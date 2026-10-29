pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The candidate window itself (macOS CandidateContentView), shared by the floating window and the previews.  Its
// keycaps are the current shortcuts (`shortcuts`); a list whose key is turned off has no switch.  `hint` is the
// engine's one-line notice: 「Enter 加入自訂詞：…」 shows a ↩ keycap, 「已加入…」 a check; alone it is a compact pill.
Rectangle {
    id: panel
    required property var candidates
    required property var preferences
    required property Theme theme
    property var shortcuts: null
    property bool keyboardEnabled: false
    property string hint: ""
    property string composition: ""
    property real maximumWidth: 650
    signal readingClicked
    readonly property bool horizontal: preferences.candidateLayout === "horizontal"
    readonly property bool compact: preferences.candidateDensity === "compact"
    readonly property real textSize: preferences.candidateFontSize
    readonly property real rowHeight: Math.ceil(textSize * (compact ? 1.32 : 1.55))
    readonly property int inset: compact ? 4 : 6
    readonly property bool hasRows: candidates.rows.length > 0
    // The keys on the keycaps (macOS CandidateKeyLabels): empty for a command that is off or has no key.
    function keyLabel(command, fallback, feature) {
        if (shortcuts && shortcuts.values !== undefined)
            return shortcuts.activeLabel(command);
        return preferences[feature] === false ? "" : fallback;
    }
    readonly property string zhuyinKey: keyLabel("typeZhuyin", "`", "feature.typeZhuyin")
    readonly property string singleKey: keyLabel("singleCharacterList", "⇧↓", "feature.singleCharacterList")
    readonly property string correctionsKey: keyLabel("correctionsList", "⇧↑", "feature.correctionsList")
    readonly property bool readable: hasRows && candidates.reading.length > 0
    // A switch whose list is turned off has no key and is not shown; with the switches hidden, one still shows while
    // it is on, so the list in use is always visible.
    readonly property bool hasSingle: readable && singleKey.length > 0 && (preferences.showsListSwitches !== false || candidates.single)
    readonly property bool hasCorrections: hasRows && candidates.tolerant !== -1 && correctionsKey.length > 0
        && (preferences.showsListSwitches !== false || candidates.corrections)
    readonly property bool hasFooter: hasRows && (hasSingle || hasCorrections || candidates.pageCount > 1)
    readonly property bool inlineFooter: horizontal
    readonly property string listIdentity: candidates.revision !== undefined ? String(candidates.revision) : [candidates.reading, candidates.pageSize, candidates.pageCount, candidates.single, candidates.corrections].join("|")
    readonly property string sizingStyle: [horizontal, compact, textSize, annotationsShown, preferences.selectionKeys, preferences.showsListSwitches, singleKey, correctionsKey, zhuyinKey, maximumWidth].join("|")
    property var shownRows: []
    property var outgoingRows: []
    property int shownPage: 0
    property int shownSelected: -1
    property string shownIdentity: ""
    property int outgoingSelected: -1
    property real steadyWidth: 0
    property bool initialized: false
    readonly property real footerRoom: inlineFooter && hasFooter ? footer.implicitWidth + 6 : 0
    // A horizontal list too wide for its room gives up the gaps between candidates, then narrows them (CandidateRows).
    readonly property bool annotationsShown: preferences.showsAnnotations !== false
    readonly property var naturalWidths: widthsFor(shownRows)
    readonly property real naturalTotal: naturalWidths.reduce((sum, value) => sum + value, 0)
    readonly property real naturalWidth: !hasRows ? 0 : horizontal ? naturalTotal + Math.max(0, shownRows.length - 1) * (compact ? 4 : 10) + footerRoom : Math.max(compact ? 96 : 160, ...naturalWidths)
    readonly property bool pageAnimating: pageMotion.running
    readonly property bool bouncing: bounceMotion.running
    activeFocusOnTab: keyboardEnabled
    readonly property bool noticeOnly: hint.length > 0 && !hasRows && composition.length === 0
    implicitWidth: Math.max(0, Math.min(maximumWidth, Math.max(steadyWidth, naturalWidth + inset * 2, hasFooter ? footer.implicitWidth + inset * 2 : 0,
        hint.length ? notice.implicitWidth + 28 : 0, composition.length ? compositionText.implicitWidth + 28 : 0,
        hasRows || composition.length ? (compact ? 96 : 160) : 0)))
    implicitHeight: content.implicitHeight + inset * 2
    color: theme.surface
    radius: noticeOnly ? height / 2 : 14
    border.color: activeFocus && keyboardEnabled ? theme.accent : theme.line
    Accessible.role: Accessible.Pane
    Accessible.name: "候選字視窗"

    function widthsFor(rows, withAnnotations) {
        const annotated = withAnnotations === undefined ? annotationsShown : withAnnotations;
        return rows.map(entry => Math.ceil((compact ? 6 : 8) + textSize * 0.68 + (compact ? 5 : 8) + textMetrics.advanceWidth(entry.text) + (compact ? 7 : 10) + (annotated && entry.annotation ? annotationMetrics.advanceWidth(entry.annotation) + 10 : 0)));
    }
    function stopMotion() {
        pageMotion.stop();
        bounceMotion.stop();
        motion.inOffset = 0;
        motion.outOffset = 0;
        motion.inOpacity = 1;
        motion.outOpacity = 0;
        motion.bounceOffset = 0;
        motion.bounceOpacity = 1;
        motion.pageOpacity = 1;
        outgoingRows = [];
    }
    function sync() {
        const nextRows = candidates.rows.map(entry => ({
                    text: entry.text,
                    annotation: entry.annotation || ""
                }));
        const turning = initialized && shownIdentity === listIdentity && shownPage !== candidates.page && shownRows.length > 0 && nextRows.length > 0;
        if (turning) {
            const oldRows = shownRows;
            const oldSelected = shownSelected;
            const direction = candidates.page > shownPage ? 1 : -1;
            stopMotion();
            outgoingRows = oldRows;
            outgoingSelected = oldSelected;
            motion.travel = theme.reduceMotion ? 0 : 10 * direction;
            motion.inOffset = motion.travel;
            motion.inOpacity = 0;
            motion.outOpacity = 1;
        } else if (!initialized || shownIdentity !== listIdentity) {
            stopMotion();
            steadyWidth = 0;
        }
        if (shownRows.length !== nextRows.length || shownRows.some((entry, index) => entry.text !== nextRows[index].text || entry.annotation !== nextRows[index].annotation))
            shownRows = nextRows;
        shownPage = candidates.page;
        shownSelected = candidates.selected;
        shownIdentity = listIdentity;
        initialized = true;
        steadyWidth = Math.max(steadyWidth, naturalWidth + inset * 2);
        if (turning && theme.motionEnabled)
            pageMotion.start();
        else if (turning)
            stopMotion();
    }
    function bounce(direction) {
        stopMotion();
        if (!theme.motionEnabled || !hasRows)
            return;
        motion.nudge = theme.reduceMotion ? 0 : 3 * direction;
        bounceMotion.start();
    }
    function move(delta) {
        const next = candidates.selected + delta;
        if (preferences.pageAtListEdge === false && (next < 0 || next >= candidates.rows.length)) {
            bounce(delta > 0 ? 1 : -1);
            return;
        }
        candidates.moveSelection(delta);
    }
    Keys.onPressed: event => {
        if (!keyboardEnabled)
            return;
        event.accepted = true;
        if (event.modifiers & Qt.ShiftModifier && event.key === Qt.Key_Down && preferences["feature.singleCharacterList"] !== false)
            candidates.toggleSingle();
        else if (event.modifiers & Qt.ShiftModifier && event.key === Qt.Key_Up && preferences["feature.correctionsList"] !== false)
            candidates.toggleCorrections();
        else if (preferences["feature.pageKeys"] !== false && (event.key === Qt.Key_PageDown || event.key === Qt.Key_Space || event.key === Qt.Key_Tab || horizontal && event.key === Qt.Key_Down))
            candidates.turnPage(event.modifiers & Qt.ShiftModifier ? -1 : 1);
        else if (preferences["feature.pageKeys"] !== false && (event.key === Qt.Key_PageUp || event.key === Qt.Key_Backtab || horizontal && event.key === Qt.Key_Up))
            candidates.turnPage(-1);
        else if (horizontal ? event.key === Qt.Key_Right : event.key === Qt.Key_Down)
            move(1);
        else if (horizontal ? event.key === Qt.Key_Left : event.key === Qt.Key_Up)
            move(-1);
        else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
            candidates.pick(candidates.selected);
        else if (event.text === "`" && preferences["feature.typeZhuyin"] !== false)
            readingClicked();
        else {
            const index = !(event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)) && event.text.length === 1 ? preferences.selectionKeys.toLowerCase().indexOf(event.text.toLowerCase()) : -1;
            if (index >= 0)
                candidates.pick(index);
            else
                event.accepted = false;
        }
    }
    Component.onCompleted: sync()
    onCandidatesChanged: if (initialized) {
        initialized = false;
        sync();
    }
    onSizingStyleChanged: if (initialized) {
        stopMotion();
        steadyWidth = 0;
        sync();
    }
    Connections {
        target: panel.candidates
        ignoreUnknownSignals: true
        function onChanged() {
            panel.sync();
        }
        function onBoundaryReached(direction) {
            panel.bounce(direction);
        }
    }
    Connections {
        target: panel.theme
        function onMotionEnabledChanged() {
            if (!panel.theme.motionEnabled)
                panel.stopMotion();
        }
        function onReduceMotionChanged() {
            panel.stopMotion();
        }
    }
    FontMetrics {
        id: textMetrics
        font.family: Qt.application.font.family
        font.pixelSize: panel.textSize
    }
    FontMetrics {
        id: annotationMetrics
        font.family: Qt.application.font.family
        font.pixelSize: panel.textSize * 0.68
    }
    QtObject {
        id: motion
        property real travel: 0
        property real nudge: 0
        property real inOffset: 0
        property real outOffset: 0
        property real inOpacity: 1
        property real outOpacity: 0
        property real bounceOffset: 0
        property real bounceOpacity: 1
        property real pageOpacity: 1
    }
    ParallelAnimation {
        id: pageMotion
        onFinished: {
            panel.outgoingRows = [];
            motion.inOpacity = 1;
            motion.pageOpacity = 1;
        }
        NumberAnimation {
            target: motion
            property: "inOffset"
            to: 0
            duration: 140
            easing.type: Easing.OutCubic
        }
        NumberAnimation {
            target: motion
            property: "outOffset"
            to: -motion.travel
            duration: 50
            easing.type: Easing.OutCubic
        }
        NumberAnimation {
            target: motion
            property: "outOpacity"
            to: 0
            duration: 50
            easing.type: Easing.OutCubic
        }
        SequentialAnimation {
            PauseAnimation {
                duration: 42
            }
            NumberAnimation {
                target: motion
                property: "inOpacity"
                to: 1
                duration: 98
                easing.type: Easing.OutCubic
            }
        }
        SequentialAnimation {
            NumberAnimation {
                target: motion
                property: "pageOpacity"
                to: 0.3
                duration: 50
            }
            NumberAnimation {
                target: motion
                property: "pageOpacity"
                to: 1
                duration: 90
            }
        }
    }
    SequentialAnimation {
        id: bounceMotion
        ParallelAnimation {
            NumberAnimation {
                target: motion
                property: "bounceOffset"
                to: motion.nudge
                duration: 60
                easing.type: Easing.OutCubic
            }
            NumberAnimation {
                target: motion
                property: "bounceOpacity"
                to: panel.theme.reduceMotion ? 0.7 : 1
                duration: 60
            }
        }
        ParallelAnimation {
            NumberAnimation {
                target: motion
                property: "bounceOffset"
                to: 0
                duration: 120
                easing.type: Easing.InOutCubic
            }
            NumberAnimation {
                target: motion
                property: "bounceOpacity"
                to: 1
                duration: 120
            }
        }
    }
    ColumnLayout {
        id: content
        anchors {
            fill: parent
            margins: panel.inset
        }
        spacing: 0
        // The engine's notice, in the footer's keycap style.
        Item {
            id: notice
            objectName: "candidateNotice"
            readonly property string message: panel.hint
            readonly property bool enter: message.indexOf("Enter ") === 0
            readonly property string rest: enter ? message.slice(6) : message
            readonly property bool done: rest.indexOf("已") === 0
            readonly property int colon: rest.indexOf("：")
            readonly property string label: colon >= 0 ? rest.slice(0, colon) : rest
            readonly property string value: colon >= 0 ? rest.slice(colon + 1) : ""
            readonly property real fontSize: Math.round(panel.textSize * 0.72)
            visible: message.length > 0
            Layout.fillWidth: true
            Layout.leftMargin: 8
            Layout.rightMargin: 8
            Layout.preferredHeight: Math.ceil(panel.textSize * 1.25) + (panel.hasRows || panel.composition.length ? 3 : 0)
            implicitWidth: noticeRow.implicitWidth
            Accessible.role: Accessible.StaticText
            Accessible.name: message
            Row {
                id: noticeRow
                y: (Math.ceil(panel.textSize * 1.25) - height) / 2
                spacing: 7
                KeyCap {
                    visible: notice.enter
                    anchors.verticalCenter: parent.verticalCenter
                    theme: panel.theme
                    outline: true
                    text: "↩"
                    fontSize: notice.fontSize
                    implicitHeight: Math.ceil(notice.fontSize * 1.45)
                }
                Rectangle {
                    visible: !notice.enter && notice.done
                    anchors.verticalCenter: parent.verticalCenter
                    width: notice.fontSize
                    height: width
                    radius: width / 2
                    color: panel.theme.success
                    Icon {
                        anchors.centerIn: parent
                        name: "check"
                        size: parent.width * 0.75
                        weight: 2.4
                        color: "white"
                    }
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: notice.label + (notice.value.length ? "：" : "")
                    textFormat: Text.PlainText
                    color: panel.theme.secondary
                    font.pixelSize: notice.fontSize
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: notice.value
                    textFormat: Text.PlainText
                    color: panel.theme.text
                    font.pixelSize: notice.fontSize
                    font.weight: Font.Medium
                }
            }
        }
        Text {
            id: compositionText
            visible: panel.composition.length > 0
            text: panel.composition
            textFormat: Text.PlainText
            color: panel.theme.text
            font {
                pixelSize: panel.textSize
                underline: true
            }
            Layout.fillWidth: true
            Layout.margins: 8
            wrapMode: Text.Wrap
        }
        Button {
            objectName: "readingButton"
            visible: panel.readable && panel.preferences.showsZhuyinHint !== false
            enabled: panel.preferences["feature.zhuyinRowClick"] !== false
            Layout.fillWidth: true
            Layout.preferredHeight: panel.compact ? 23 : 26
            focusPolicy: Qt.NoFocus
            padding: 4
            background: Rectangle {
                color: parent.hovered ? panel.theme.hover : "transparent"
                radius: 5
            }
            contentItem: RowLayout {
                spacing: 8
                // No keycap when typing the Zhuyin itself is turned off.
                KeyCap {
                    visible: panel.zhuyinKey.length > 0
                    theme: panel.theme
                    outline: true
                    small: true
                    implicitHeight: 19
                    fontSize: 12
                    text: panel.zhuyinKey
                }
                Text {
                    text: panel.candidates.reading
                    textFormat: Text.PlainText
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    font.pixelSize: 12
                    color: panel.theme.secondary
                }
            }
            Accessible.name: panel.zhuyinKey + "，" + panel.candidates.reading
            onClicked: panel.readingClicked()
        }
        RowLayout {
            Layout.fillWidth: true
            visible: panel.hasRows
            spacing: 6
            Item {
                id: rowClip
                objectName: "candidateClip"
                Layout.fillWidth: true
                Layout.preferredHeight: !panel.hasRows ? 0 : panel.horizontal ? panel.rowHeight : (panel.candidates.pageCount > 1 ? panel.candidates.pageSize : panel.shownRows.length) * panel.rowHeight
                clip: true
                Rectangle {
                    id: highlight
                    objectName: "candidateHighlight"
                    // The selected row's slot; the list never scrolls, so its coordinates are the clip's.
                    readonly property Item chosen: list.count > 0 && panel.candidates.selected >= 0 ? list.itemAtIndex(panel.candidates.selected) : null
                    visible: chosen !== null
                    x: (chosen ? chosen.x - list.contentX : 0) + (panel.horizontal ? motion.bounceOffset : 0)
                    y: (chosen ? chosen.y - list.contentY : 0) + 1 + (panel.horizontal ? 0 : motion.bounceOffset)
                    width: chosen ? chosen.width : 0
                    height: panel.rowHeight - 2
                    radius: panel.compact ? 6 : 8
                    opacity: motion.bounceOpacity
                    color: panel.theme.selection
                }
                CandidateRows {
                    id: outgoing
                    objectName: "outgoingCandidateList"
                    anchors.fill: parent
                    entries: panel.outgoingRows
                    selection: panel.outgoingSelected
                    interactiveRows: false
                    opacity: motion.outOpacity
                    transform: Translate {
                        x: panel.horizontal ? motion.outOffset : 0
                        y: panel.horizontal ? 0 : motion.outOffset
                    }
                    Accessible.ignored: true
                }
                CandidateRows {
                    id: list
                    objectName: "candidateList"
                    anchors.fill: parent
                    entries: panel.shownRows
                    selection: panel.candidates.selected
                    interactiveRows: true
                    opacity: motion.inOpacity * motion.bounceOpacity
                    transform: Translate {
                        x: panel.horizontal ? motion.inOffset + motion.bounceOffset : 0
                        y: panel.horizontal ? 0 : motion.inOffset + motion.bounceOffset
                    }
                }
            }
            Item {
                id: inlineFooterSlot
                visible: panel.inlineFooter && panel.hasFooter
                Layout.preferredWidth: footer.implicitWidth
                Layout.preferredHeight: panel.rowHeight
            }
        }
        Item {
            id: footerBelow
            visible: !panel.inlineFooter && panel.hasFooter
            Layout.fillWidth: true
            Layout.preferredHeight: panel.compact ? 23 : 25
        }
    }
    RowLayout {
        id: footer
        parent: panel.inlineFooter ? inlineFooterSlot : footerBelow
        objectName: "candidateFooter"
        visible: panel.hasFooter
        width: parent.width
        anchors.verticalCenter: parent.verticalCenter
        spacing: 0
        Button {
            id: singleToggle
            objectName: "singleButton"
            text: panel.singleKey + " 單字"
            visible: panel.hasSingle
            focusPolicy: Qt.NoFocus
            leftPadding: 4
            rightPadding: 6
            topPadding: 2
            bottomPadding: 2
            contentItem: Row {
                spacing: 5
                KeyCap {
                    anchors.verticalCenter: parent.verticalCenter
                    theme: panel.theme
                    outline: true
                    small: true
                    implicitHeight: 19
                    fontSize: 12
                    text: panel.singleKey
                    active: panel.candidates.single
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "單字"
                    color: panel.candidates.single ? panel.theme.accent : panel.theme.secondary
                    font.pixelSize: 12
                }
            }
            background: Rectangle {
                radius: 5
                color: parent.hovered ? panel.theme.hover : "transparent"
            }
            Accessible.checkable: true
            Accessible.checked: panel.candidates.single
            onClicked: panel.candidates.toggleSingle()
        }
        Button {
            id: correctionsToggle
            objectName: "correctionsButton"
            text: panel.correctionsKey + " 修正"
            visible: panel.hasCorrections
            focusPolicy: Qt.NoFocus
            leftPadding: 4
            rightPadding: 6
            topPadding: 2
            bottomPadding: 2
            contentItem: Row {
                spacing: 5
                KeyCap {
                    anchors.verticalCenter: parent.verticalCenter
                    theme: panel.theme
                    outline: true
                    small: true
                    implicitHeight: 19
                    fontSize: 12
                    text: panel.correctionsKey
                    active: panel.candidates.corrections
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "修正"
                    color: panel.candidates.corrections ? panel.theme.accent : panel.theme.secondary
                    font.pixelSize: 12
                }
            }
            background: Rectangle {
                radius: 5
                color: parent.hovered ? panel.theme.hover : "transparent"
            }
            Accessible.checkable: true
            Accessible.checked: panel.candidates.corrections
            onClicked: panel.candidates.toggleCorrections()
        }
        Item {
            Layout.fillWidth: true
        }
        PageButton {
            iconName: "chevronLeft"
            enabled: panel.candidates.page > 0
            Accessible.name: "上一頁"
            onClicked: panel.candidates.turnPage(-1)
        }
        Text {
            objectName: "candidatePageLabel"
            text: (panel.candidates.page + 1) + " / " + panel.candidates.pageCount
            color: panel.theme.secondary
            font {
                pixelSize: panel.theme.captionSize
                preferShaping: false
                features: ({ "tnum": 1 })
            }
            opacity: motion.pageOpacity
        }
        PageButton {
            iconName: "chevronRight"
            enabled: panel.candidates.page + 1 < panel.candidates.pageCount
            Accessible.name: "下一頁"
            onClicked: panel.candidates.turnPage(1)
        }
    }
    component PageButton: ToolButton {
        id: pageButton
        property string iconName
        implicitWidth: 22
        implicitHeight: 22
        padding: 0
        focusPolicy: Qt.NoFocus
        background: Rectangle {
            radius: 5
            color: pageButton.hovered && pageButton.enabled ? panel.theme.hover : "transparent"
        }
        contentItem: Item {
            Icon {
                anchors.centerIn: parent
                name: pageButton.iconName
                size: 14
                weight: 2
                color: pageButton.enabled ? panel.theme.secondary : panel.theme.track
            }
        }
    }
    component CandidateRows: ListView {
        id: rows
        required property var entries
        required property int selection
        required property bool interactiveRows
        readonly property var measured: panel.widthsFor(entries)
        readonly property real total: measured.reduce((sum, value) => sum + value, 0)
        readonly property real gap: panel.horizontal && count > 1 ? Math.min(panel.compact ? 4 : 10, Math.max(0, (width - total) / (count - 1))) : 0
        readonly property real fit: Math.min(1, Math.max(0, width - gap * Math.max(0, count - 1)) / Math.max(1, total))
        orientation: panel.horizontal ? ListView.Horizontal : ListView.Vertical
        spacing: gap
        model: entries
        interactive: false
        // No currentIndex: a ListView scrolls to keep its current item in view, which would slide the rows under the
        // highlight. Every row of a page is laid out, so nothing needs scrolling.
        cacheBuffer: 2000
        boundsBehavior: Flickable.StopAtBounds
        delegate: Button {
            id: candidate
            required property int index
            required property var modelData
            readonly property bool chosen: index === rows.selection
            objectName: rows.interactiveRows ? "candidate" + index : "outgoingCandidate" + index
            width: panel.horizontal ? rows.measured[index] * rows.fit : rows.width
            height: panel.rowHeight
            focusPolicy: Qt.NoFocus
            enabled: rows.interactiveRows
            // macOS CandidateRow: the key 8 in (6 compact), the text after it, 10 at the end (7 compact).
            leftPadding: panel.compact ? 6 : 8
            rightPadding: panel.compact ? 7 : 10
            topPadding: 0
            bottomPadding: 0
            Accessible.name: panel.preferences.selectionKeys.charAt(index) + " " + modelData.text + (modelData.annotation ? "，" + modelData.annotation : "")
            Accessible.role: Accessible.Button
            Accessible.selected: chosen
            Accessible.ignored: !rows.interactiveRows
            background: Rectangle {
                y: 1
                height: candidate.height - 2
                radius: panel.compact ? 6 : 8
                color: !candidate.chosen && candidate.hovered && !panel.pageAnimating ? panel.theme.hover : "transparent"
            }
            contentItem: RowLayout {
                spacing: panel.compact ? 5 : 8
                // The selection key as the user types it (a s d … for the home row), in medium monospaced digits.
                Text {
                    text: panel.preferences.selectionKeys.charAt(candidate.index)
                    color: candidate.chosen ? Qt.rgba(1, 1, 1, 0.85) : panel.theme.secondary
                    font.pixelSize: panel.textSize * 0.68
                    font.weight: Font.Medium
                    font.features: ({ "tnum": 1 })
                    Layout.preferredWidth: panel.textSize * 0.68
                }
                Text {
                    text: candidate.modelData.text
                    textFormat: Text.PlainText
                    color: candidate.chosen ? "white" : panel.theme.text
                    font.pixelSize: panel.textSize
                    Layout.minimumWidth: 0
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    clip: true
                }
                Text {
                    visible: panel.annotationsShown && text.length > 0
                    text: candidate.modelData.annotation || ""
                    textFormat: Text.PlainText
                    Layout.maximumWidth: Math.max(0, candidate.availableWidth * 0.48)
                    elide: Text.ElideRight
                    color: candidate.chosen ? Qt.rgba(1, 1, 1, 0.8) : panel.theme.tertiary
                    font.pixelSize: panel.textSize * 0.68
                }
            }
            onClicked: {
                if (panel.keyboardEnabled)
                    panel.forceActiveFocus();
                panel.candidates.pick(index);
            }
        }
    }
}
