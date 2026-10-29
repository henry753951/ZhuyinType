import QtQuick
import QtTest
import "../qml" as Pages

TestCase {
    id: test
    name: "AboutPages"
    visible: true
    width: 640
    height: 800
    when: windowShown
    Pages.Theme {
        id: testTheme
        motionEnabled: false
    }
    QtObject {
        id: mockSettings
        property var values: ({})
        property var definitions: []
    }
    QtObject {
        id: services
        property bool checkingUpdate: false
        property var update: ({})
        property string updateError: ""
        property int checks: 0
        property int releasePages: 0
        function checkForUpdates() {
            checks += 1;
            checkingUpdate = true;
        }
        function openReleasePage() {
            releasePages += 1;
            return true;
        }
    }
    // LegalDocuments: the filtered list and the texts it names.
    QtObject {
        id: documents
        property var reveals: []
        readonly property var texts: ({
                "AGPL-3.0.txt": "License test fixture · 授權測試文字\n".repeat(200),
                "LGPL-3.0.txt": "GNU LESSER GENERAL PUBLIC LICENSE\n",
                "lm_chat/NOTICE.md": "# 署名\n\n| 資料 | 授權 |\n|---|---|\n| 維基百科 | CC BY-SA 4.0 |\n"
            })
        property var sources: ({
                groups: [
                    {
                        id: "app",
                        title: "ZhuyinType",
                        entries: [
                            {
                                id: "zhuyintype",
                                name: "ZhuyinType",
                                use: "輸入法程式",
                                license: "AGPL-3.0-only",
                                distribution: "bundled",
                                copyright: "Copyright (c) 2026 Henry753951 and ZhuyinType contributors",
                                url: "https://github.com/henry753951/ZhuyinType",
                                files: [
                                    {
                                        name: "AGPL-3.0.txt",
                                        kind: "text",
                                        notice: false
                                    }
                                ]
                            }
                        ]
                    },
                    {
                        id: "libraries",
                        title: "程式庫",
                        entries: [
                            {
                                id: "qt",
                                name: "Qt 6",
                                use: "Linux 與 Windows 的設定視窗與候選字視窗",
                                license: "LGPL-3.0-only",
                                distribution: "bundled",
                                version: "6.12.0",
                                files: [
                                    {
                                        name: "LGPL-3.0.txt",
                                        kind: "text",
                                        notice: false
                                    }
                                ]
                            }
                        ]
                    },
                    {
                        id: "models",
                        title: "模型與訓練來源",
                        entries: [
                            {
                                id: "lm-chat",
                                name: "口語模型",
                                use: "語言模型",
                                license: "CC BY-SA 4.0",
                                distribution: "training",
                                files: [
                                    {
                                        name: "lm_chat/NOTICE.md",
                                        kind: "markdown",
                                        notice: true
                                    },
                                    {
                                        name: "lm_chat/attribution.jsonl",
                                        kind: "folder",
                                        notice: true
                                    }
                                ]
                            }
                        ]
                    }
                ],
                error: ""
            })
        function text(name) {
            return texts[name] || "";
        }
        function reveal(name) {
            reveals = reveals.concat([name]);
            return true;
        }
    }
    Pages.AboutPages {
        id: about
        width: test.width
        settings: mockSettings
        theme: testTheme
        appInfo: ({
                version: "0.4.7",
                build: "21",
                licenseName: "AGPL-3.0-only",
                documents: documents
            })
    }
    SignalSpy {
        id: navigation
        target: about
        signalName: "navigateRequested"
    }
    function init() {
        about.viewer.close();
        about.services = null;
        about.pageId = "about";
        services.checkingUpdate = false;
        services.update = {};
        services.updateError = "";
        services.checks = 0;
        services.releasePages = 0;
        documents.reveals = [];
        navigation.clear();
        testTheme.appearance = "light";
        test.width = 640;
        failOnWarning(/.*(TypeError|ReferenceError|Binding loop).*/);
    }
    function test_routes_data() {
        return ["about", "dataSources"].map(route => ({
                    tag: route,
                    route: route
                }));
    }
    function test_routes(data) {
        about.pageId = data.route;
        wait(60);
        verify(about.implicitHeight > 100);
        testTheme.appearance = "dark";
        test.width = 320;
        wait(60);
        verify(about.implicitHeight > 100);
        verify(about.children[0].item.width <= 321);
    }
    function test_aboutCard() {
        wait(30);
        compare(findChild(about, "aboutVersion").text, "版本 0.4.7（21）");
        compare(about.sourceUrl, "https://github.com/henry753951/ZhuyinType/tree/v0.4.7");
        // Without an update service there is no 更新 section.
        verify(!findChild(about, "checkUpdate").visible);
    }
    function test_licenseOpensInsideTheApp() {
        wait(30);
        const button = findChild(about, "readLicense");
        verify(button);
        button.clicked();
        tryCompare(about.viewer, "opened", true);
        compare(about.viewer.title, "AGPL-3.0.txt");
        compare(about.viewer.text, documents.texts["AGPL-3.0.txt"]);
        compare(about.viewer.markdown, false);
        const plain = findChild(about.viewer.contentItem, "documentText");
        compare(plain.text, documents.texts["AGPL-3.0.txt"]);
        plain.selectAll();
        compare(plain.selectedText, documents.texts["AGPL-3.0.txt"]);
        verify(findChild(about.viewer.footer, "copyDocument").enabled);
        keyClick(Qt.Key_Escape);
        tryCompare(about.viewer, "visible", false);
    }
    function test_sourcesAreStructured() {
        about.pageId = "dataSources";
        wait(60);
        const entry = findChild(about, "source-qt");
        verify(entry);
        compare(entry.subtitle, "Linux 與 Windows 的設定視窗與候選字視窗・LGPL-3.0-only・隨 App 提供");
        compare(entry.expanded, false);
        entry.expanded = true;
        wait(30);
        const file = findChild(entry, "file-LGPL-3.0.txt");
        verify(file && file.visible);
        file.clicked();
        tryCompare(about.viewer, "opened", true);
        compare(about.viewer.subtitle, "Qt 6");
        compare(about.viewer.text, "GNU LESSER GENERAL PUBLIC LICENSE\n");
        about.viewer.close();
        tryCompare(about.viewer, "visible", false);
        const model = findChild(about, "source-lm-chat");
        compare(model.subtitle, "語言模型・CC BY-SA 4.0・用於模型訓練");
        model.expanded = true;
        wait(30);
        findChild(model, "file-lm_chat/NOTICE.md").clicked();
        tryCompare(about.viewer, "opened", true);
        compare(about.viewer.markdown, true);
        compare(about.viewer.revealable, true);
        about.viewer.revealRequested();
        compare(documents.reveals, ["lm_chat/NOTICE.md"]);
        about.viewer.close();
        tryCompare(about.viewer, "visible", false);
        findChild(model, "file-lm_chat/attribution.jsonl").clicked();
        compare(about.viewer.visible, false);
        compare(documents.reveals[1], "lm_chat/attribution.jsonl");
    }
    function test_unreadableSources() {
        about.appInfo = {
            version: "0.4.7"
        };
        about.pageId = "dataSources";
        wait(60);
        compare(about.sources.groups.length, 0);
        verify(findChild(about, "source-qt") === null);
        about.appInfo = {
            version: "0.4.7",
            build: "21",
            licenseName: "AGPL-3.0-only",
            documents: documents
        };
    }
    function test_manualUpdateState() {
        about.services = services;
        wait(60);
        const button = findChild(about, "checkUpdate");
        verify(button.visible && button.enabled);
        button.clicked();
        compare(services.checks, 1);
        compare(button.enabled, false);
        compare(button.text, "正在檢查…");
        services.update = {
            version: "v0.5.0",
            newer: true,
            checkedAt: "2026-10-29T08:00:00Z"
        };
        services.checkingUpdate = false;
        verify(button.enabled);
        verify(findChild(about, "updateStatus").subtitle.startsWith("上次檢查："));
        const download = findChild(about, "downloadUpdate");
        verify(download.visible);
        download.clicked();
        compare(services.releasePages, 1);
    }
    function find(item, property, value) {
        if (item[property] === value)
            return item;
        for (const child of item.children || []) {
            const found = find(child, property, value);
            if (found)
                return found;
        }
        return null;
    }
    function test_sourcesLink() {
        wait(30);
        const row = find(about, "title", "授權與資料來源");
        verify(row);
        row.activated();
        compare(navigation.count, 1);
        compare(navigation.signalArguments[0][0], "dataSources");
    }
}
