import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "Pages.js" as Pages

ApplicationWindow {
    id: window
    required property var settings
    required property var candidates
    required property var correctionCandidates
    required property var engineSession
    required property var inputController
    required property var shortcuts
    required property var services
    required property var platformSettings
    required property var appInfo
    property string pageId: "intro"
    // KEYS's CorrectionLog, a context property (null while it is not set up).
    readonly property var contextCorrectionLog: typeof correctionLog !== "undefined" ? correctionLog : null
    property bool imeRunning: false
    /// The setup wizard's sandbox (Practice in C++); null: no wizard.
    property var practice: null
    // The setup wizard: from 介紹, and once on the first opening of the settings (main.cpp calls this).
    function showWizardOnFirstUse() {
        if (wizardLoader.item)
            wizardLoader.item.showOnFirstUse();
    }
    Loader {
        id: wizardLoader
        active: window.practice !== null
        sourceComponent: Component {
            WizardWindow {
                settings: window.settings
                shortcuts: window.shortcuts
                practice: window.practice
                engineSession: window.engineSession
                transientParent: null
                onNavigateRequested: pageId => {
                    window.navigate(pageId);
                    window.show();
                    window.raise();
                    window.requestActivate();
                }
            }
        }
    }
    readonly property var currentPage: Pages.lookup(pageId)
    readonly property string pageTitle: currentPage.title
    readonly property var renderPages: Pages.all.map(page => page.id)
    readonly property var pageComponents: ({
            overview: overviewComponent,
            about: aboutComponent,
            input: inputComponent,
            library: libraryComponent,
            appearance: appearanceComponent
        })
    width: 900
    height: 780
    minimumWidth: 640
    minimumHeight: 520
    visible: true
    title: pageTitle + " — ZhuyinType"
    color: appTheme.canvas
    palette {
        windowText: appTheme.text
        text: appTheme.text
        buttonText: appTheme.text
        button: appTheme.card
        base: appTheme.card
        highlight: appTheme.accent
    }

    // Opens a page; with `row`, scrolls to the SettingsRow whose anchor it is and plays its arrival tint.
    property string arrivalRow: ""
    function navigate(id, row) {
        window.arrivalRow = row || "";
        if (window.pageId === Pages.lookup(id).id) {
            arrivalTimer.restart();
            return;
        }
        window.pageId = Pages.lookup(id).id;
        if (scroll.contentItem)
            scroll.contentItem.contentY = 0;
        arrivalTimer.restart();
    }
    function findRow(item, anchor) {
        if (item.isSettingsRow === true && item.anchor === anchor && item.visible)
            return item;
        for (const child of item.children) {
            const found = window.findRow(child, anchor);
            if (found)
                return found;
        }
        return null;
    }
    Timer {
        id: arrivalTimer
        // After the page's enter transition has laid it out.
        interval: 260
        onTriggered: {
            if (!window.arrivalRow || !pageLoader.currentItem)
                return;
            const row = window.findRow(pageLoader.currentItem, window.arrivalRow);
            window.arrivalRow = "";
            if (!row || !scroll.contentItem)
                return;
            const flick = scroll.contentItem;
            const y = row.mapToItem(flick.contentItem, 0, 0).y;
            const target = Math.max(0, Math.min(flick.contentHeight - flick.height, y - flick.height / 3));
            if (Math.abs(flick.contentY - target) > 1)
                scrollMotion.to = target, scrollMotion.restart();
            row.arrive();
        }
    }
    NumberAnimation {
        id: scrollMotion
        target: scroll.contentItem
        property: "contentY"
        duration: appTheme.motionEnabled ? 260 : 0
        easing.type: Easing.OutCubic
    }
    Shortcut {
        sequence: "Alt+Left"
        enabled: !!window.currentPage.parent
        onActivated: window.navigate(window.currentPage.parent)
    }
    Theme {
        id: appTheme
        // Renders (--render-dir) are stills, as on macOS: demos rest on their pinned step and pages switch at once.
        reduceMotion: Qt.application.arguments.includes("--render-dir") || (Qt.styleHints.accessibility !== undefined && Qt.styleHints.accessibility.motionPreference !== undefined && Qt.styleHints.accessibility.motionPreference === Qt.ReducedMotion)
        appearance: window.settings.values.panelAppearance
        motionEnabled: window.settings.values.panelAnimation !== false
    }
    Component {
        id: overviewComponent
        OverviewPages {
            engineSession: window.engineSession
            inputController: window.inputController
            services: window.services
            shortcuts: window.shortcuts
            wizard: wizardLoader.item
            settings: window.settings
            theme: appTheme
            appInfo: window.appInfo
            onNavigateRequested: (pageId, title, row) => window.navigate(pageId, row)
        }
    }
    Component {
        id: aboutComponent
        AboutPages {
            services: window.services
            settings: window.settings
            theme: appTheme
            appInfo: window.appInfo
            onNavigateRequested: (pageId, title, row) => window.navigate(pageId, row)
        }
    }
    Component {
        id: inputComponent
        InputPages {
            shortcuts: window.shortcuts
            inputController: window.inputController
            settings: window.settings
            theme: appTheme
            appInfo: window.appInfo
            onNavigateRequested: (pageId, title, row) => window.navigate(pageId, row)
        }
    }
    Component {
        id: libraryComponent
        LibraryPages {
            scrollViewport: scroll.contentItem
            correctionLog: window.contextCorrectionLog
            engineSession: window.engineSession
            platformSettings: window.platformSettings
            shortcuts: window.shortcuts
            settings: window.settings
            theme: appTheme
            appInfo: window.appInfo
            onNavigateRequested: (pageId, title, row) => window.navigate(pageId, row)
        }
    }
    Component {
        id: appearanceComponent
        AppearancePage {
            shortcuts: window.shortcuts
            settings: window.settings
            theme: appTheme
            candidates: window.candidates
            correctionCandidates: window.correctionCandidates
            onNavigateRequested: (pageId, title, row) => window.navigate(pageId, row)
        }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0
        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: window.width < 760 ? 188 : 220
            color: appTheme.sidebar
            Flickable {
                anchors.fill: parent
                clip: true
                contentWidth: width
                contentHeight: sidebarContent.implicitHeight + 24
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {}
                ColumnLayout {
                    id: sidebarContent
                    x: 10
                    y: 14
                    width: parent.width - 20
                    height: Math.max(implicitHeight, parent.height - 28)
                    spacing: 2
                    RowLayout {
                        Layout.leftMargin: 8
                        Layout.bottomMargin: 10
                        spacing: 10
                        Image {
                            source: "qrc:/branding/AppIcon.icns"
                            sourceSize.width: 64
                            sourceSize.height: 64
                            Layout.preferredWidth: 28
                            Layout.preferredHeight: 28
                        }
                        Text {
                            text: "ZhuyinType"
                            color: appTheme.text
                            font {
                                pixelSize: appTheme.headingSize
                                weight: Font.DemiBold
                            }
                        }
                    }
                    Repeater {
                        model: Pages.sidebar
                        delegate: ColumnLayout {
                            required property var modelData
                            Layout.fillWidth: true
                            spacing: 2
                            Text {
                                visible: text.length > 0
                                text: parent.modelData.section
                                font {
                                    pixelSize: appTheme.captionSize
                                    weight: Font.DemiBold
                                }
                                color: appTheme.tertiary
                                Layout.leftMargin: 10
                                Layout.topMargin: 14
                                Layout.bottomMargin: 4
                            }
                            SidebarItem {
                                theme: appTheme
                                iconName: parent.modelData.id
                                title: parent.modelData.title
                                checked: (window.currentPage.parent || window.pageId) === parent.modelData.id
                                onClicked: window.navigate(parent.modelData.id)
                            }
                        }
                    }
                    Item {
                        Layout.fillHeight: true
                    }
                    RowLayout {
                        Layout.leftMargin: 10
                        Layout.bottomMargin: 4
                        spacing: 7
                        Rectangle {
                            width: 7
                            height: 7
                            radius: 3.5
                            color: window.engineSession.ready ? appTheme.success : appTheme.warning
                        }
                        Text {
                            text: window.engineSession.ready ? "引擎已連接" : "詞庫未載入"
                            color: appTheme.secondary
                            font.pixelSize: appTheme.captionSize
                        }
                    }
                }
            }
        }
        Rectangle {
            Layout.fillHeight: true
            width: 1
            color: appTheme.line
        }
        ColumnLayout {
            Layout.fillHeight: true
            Layout.fillWidth: true
            spacing: 0
            ColumnLayout {
                Layout.fillWidth: true
                Layout.maximumWidth: appTheme.maxContentWidth
                Layout.alignment: Qt.AlignHCenter
                Layout.leftMargin: appTheme.pageInset
                Layout.rightMargin: appTheme.pageInset
                Layout.topMargin: window.currentPage.parent ? 14 : 26
                Layout.bottomMargin: 6
                spacing: 2
                AppButton {
                    objectName: "backButton"
                    theme: appTheme
                    visible: !!window.currentPage.parent
                    kind: "plain"
                    compact: true
                    iconName: "chevronLeft"
                    text: Pages.lookup(window.currentPage.parent).title
                    leftPadding: 4
                    Layout.leftMargin: -6
                    Accessible.name: "返回" + text
                    onClicked: window.navigate(window.currentPage.parent)
                }
                Text {
                    text: window.pageTitle
                    color: appTheme.text
                    font {
                        pixelSize: appTheme.titleSize
                        weight: Font.DemiBold
                    }
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                }
            }
            ScrollView {
                id: scroll
                Layout.fillWidth: true
                Layout.fillHeight: true
                contentWidth: availableWidth
                clip: true
                ColumnLayout {
                    x: (scroll.availableWidth - width) / 2
                    width: Math.max(0, Math.min(appTheme.maxContentWidth, scroll.availableWidth - 2 * appTheme.pageInset))
                    spacing: 12
                    Item {
                        height: 10
                    }
                    StackView {
                        id: pageLoader
                        objectName: "pageLoader"
                        Layout.fillWidth: true
                        implicitHeight: currentItem ? currentItem.implicitHeight : 0
                        property bool initialized: false
                        property bool forward: true
                        property string shownRoute: ""
                        function showPage(immediate) {
                            forward = window.currentPage.parent === shownRoute;
                            replace(window.pageComponents[window.currentPage.family], {
                                pageId: window.pageId
                            }, immediate ? StackView.Immediate : StackView.ReplaceTransition);
                            shownRoute = window.pageId;
                            if (scroll.contentItem)
                                scroll.contentItem.contentY = 0;
                        }
                        Component.onCompleted: {
                            initialized = true;
                            showPage(true);
                        }
                        Connections {
                            target: window
                            function onPageIdChanged() {
                                if (pageLoader.initialized)
                                    pageLoader.showPage(false);
                            }
                        }
                        replaceEnter: Transition {
                            ParallelAnimation {
                                NumberAnimation {
                                    property: "opacity"
                                    from: 0
                                    to: 1
                                    duration: appTheme.motionEnabled ? 220 : 0
                                    easing.type: Easing.OutCubic
                                }
                                NumberAnimation {
                                    property: "x"
                                    from: appTheme.reduceMotion ? 0 : pageLoader.forward ? 24 : -12
                                    to: 0
                                    duration: appTheme.motionEnabled ? 220 : 0
                                    easing.type: Easing.OutCubic
                                }
                            }
                        }
                        replaceExit: Transition {
                            ParallelAnimation {
                                NumberAnimation {
                                    property: "opacity"
                                    from: 1
                                    to: 0
                                    duration: appTheme.motionEnabled ? 180 : 0
                                    easing.type: Easing.OutCubic
                                }
                                NumberAnimation {
                                    property: "x"
                                    from: 0
                                    to: appTheme.reduceMotion ? 0 : pageLoader.forward ? -12 : 24
                                    duration: appTheme.motionEnabled ? 220 : 0
                                    easing.type: Easing.OutCubic
                                }
                            }
                        }
                    }
                    Text {
                        visible: window.settings.error.length > 0
                        text: window.settings.error
                        color: appTheme.danger
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                    }
                    Item {
                        height: 24
                    }
                }
            }
        }
    }
    Window {
        id: floating
        objectName: "floatingPreview"
        visible: false
        transientParent: window
        flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowDoesNotAcceptFocus | Qt.WindowStaysOnTopHint
        color: "transparent"
        width: floatingPanel.implicitWidth
        height: floatingPanel.implicitHeight
        CandidatePanel {
            id: floatingPanel
            anchors.fill: parent
            candidates: window.candidates
            preferences: window.settings.values
            shortcuts: window.shortcuts
            theme: appTheme
            maximumWidth: 650
        }
    }
    onClosing: close => {
        floating.close();
        if (imeRunning) {
            close.accepted = false;
            hide();
        }
    }
}
