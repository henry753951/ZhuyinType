pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// 關於 and 授權與資料來源, as on macOS (SettingsPanes.swift AboutPane, SourcesPane). Every licence and notice opens in
// the DocumentViewer inside the app; nothing is sent to a browser or an editor to be read.
ColumnLayout {
    id: page
    required property var settings
    required property Theme theme
    property string pageId: "about"
    /// version, build, licenseName, and `documents` (LegalDocuments: sources, text(name), reveal(name)).
    property var appInfo: ({})
    property var services: null
    readonly property var update: services ? services.update || ({}) : ({})
    readonly property var documents: appInfo.documents || null
    readonly property var sources: documents && documents.sources ? documents.sources : ({
            groups: [],
            error: "無法讀取授權與資料來源"
        })
    readonly property string version: appInfo.version ? appInfo.version + (appInfo.build ? "（" + appInfo.build + "）" : "") : "開發版"
    readonly property string sourceUrl: "https://github.com/henry753951/ZhuyinType/tree/" + (appInfo.version ? "v" + appInfo.version : "main")
    signal navigateRequested(string pageId, string title, string row)
    spacing: 12

    /// Opens a file of the list in the viewer, or its folder when it is not text (archives, data lists).
    function open(file, entryName) {
        if (!documents)
            return;
        if (file.kind === "folder") {
            documents.reveal(file.name);
            return;
        }
        viewer.revealName = file.notice ? file.name : "";
        viewer.show(file.name, entryName, documents.text(file.name) || "無法讀取這個檔案。", file.kind === "markdown", !!file.notice);
    }
    function distribution(value) {
        return value === "bundled" ? "隨 App 提供" : value === "training" ? "用於模型訓練" : "僅用於評測";
    }
    function checkedAt(value) {
        const date = new Date(value);
        return isNaN(date.getTime()) ? "" : date.toLocaleString(Qt.locale(), Locale.ShortFormat);
    }

    Loader {
        Layout.fillWidth: true
        sourceComponent: page.pageId === "dataSources" ? sources : about
    }

    // MARK: 關於

    Component {
        id: about
        ColumnLayout {
            spacing: page.theme.sectionSpacing
            SectionCard {
                theme: page.theme
                padded: true
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 6
                    Image {
                        source: "qrc:/branding/AppIcon.icns"
                        Layout.preferredWidth: 80
                        Layout.preferredHeight: 80
                        Layout.alignment: Qt.AlignHCenter
                        sourceSize.width: 160
                        sourceSize.height: 160
                        Accessible.ignored: true
                    }
                    Text {
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignHCenter
                        text: "ZhuyinType"
                        color: page.theme.text
                        font.pixelSize: page.theme.displaySize
                        font.weight: Font.DemiBold
                    }
                    Text {
                        objectName: "aboutVersion"
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignHCenter
                        text: "版本 " + page.version
                        color: page.theme.secondary
                        font.pixelSize: page.theme.bodySize
                        font.features: ({ "tnum": 1 })
                    }
                }
            }
            // macOS also turns on automatic checks and installs here (Sparkle); this client only checks and links
            // the download, so only those rows are shown.
            SectionCard {
                visible: page.services !== null
                theme: page.theme
                title: "更新"
                footer: page.services && page.services.updateError ? page.services.updateError : ""
                SettingsRow {
                    theme: page.theme
                    anchor: "checkUpdate"
                    title: "檢查更新"
                    subtitle: page.update.checkedAt ? "上次檢查：" + page.checkedAt(page.update.checkedAt) : ""
                    objectName: "updateStatus"
                    AppButton {
                        objectName: "checkUpdate"
                        theme: page.theme
                        compact: true
                        text: page.services && page.services.checkingUpdate ? "正在檢查…" : "立即檢查…"
                        enabled: page.services !== null && !page.services.checkingUpdate
                        onClicked: page.services.checkForUpdates()
                    }
                }
                SettingsRow {
                    visible: !!page.update.version && page.update.newer === true
                    theme: page.theme
                    separated: true
                    title: "有新版本"
                    value: page.update.version || ""
                    AppButton {
                        objectName: "downloadUpdate"
                        theme: page.theme
                        compact: true
                        text: "下載…"
                        onClicked: page.services.openReleasePage()
                    }
                }
                SettingsRow {
                    visible: !!page.update.version && page.update.newer !== true
                    theme: page.theme
                    separated: true
                    title: "目前是最新版本"
                    value: page.update.version || ""
                }
            }
            SectionCard {
                theme: page.theme
                SettingsRow {
                    theme: page.theme
                    anchor: "license"
                    title: "程式授權"
                    AppButton {
                        objectName: "readLicense"
                        theme: page.theme
                        kind: "plain"
                        compact: true
                        text: "AGPL 3.0"
                        Accessible.name: "閱讀程式授權 AGPL 3.0"
                        onClicked: page.open({
                            name: "AGPL-3.0.txt",
                            kind: "text",
                            notice: false
                        }, "ZhuyinType")
                    }
                }
                SettingsRow {
                    theme: page.theme
                    separated: true
                    title: "原始碼"
                    AppButton {
                        objectName: "sourceCode"
                        theme: page.theme
                        kind: "plain"
                        compact: true
                        text: "查看此版本"
                        onClicked: Qt.openUrlExternally(page.sourceUrl)
                    }
                }
                NavigationRow {
                    theme: page.theme
                    separated: true
                    anchor: "sources"
                    title: "授權與資料來源"
                    detail: "程式庫、詞庫、資料集與模型的授權與署名"
                    onActivated: page.navigateRequested("dataSources", "授權與資料來源", "")
                }
            }
        }
    }

    // MARK: 授權與資料來源

    Component {
        id: sources
        ColumnLayout {
            spacing: page.theme.sectionSpacing
            SectionCard {
                visible: page.sources.groups.length === 0
                theme: page.theme
                SettingsRow {
                    theme: page.theme
                    title: "無法讀取授權與資料來源"
                }
            }
            Repeater {
                model: page.sources.groups
                delegate: SectionCard {
                    id: group
                    required property var modelData
                    theme: page.theme
                    title: modelData.title
                    Repeater {
                        model: group.modelData.entries
                        delegate: DisclosureRow {
                            id: entry
                            required property var modelData
                            required property int index
                            objectName: "source-" + modelData.id
                            theme: page.theme
                            separated: index > 0
                            title: modelData.name
                            subtitle: [modelData.use, modelData.license, page.distribution(modelData.distribution)].join("・")
                            Text {
                                visible: !!entry.modelData.copyright
                                Layout.fillWidth: true
                                text: entry.modelData.copyright || ""
                                color: page.theme.text
                                font.pixelSize: page.theme.detailSize
                                wrapMode: Text.Wrap
                                textFormat: Text.PlainText
                            }
                            Text {
                                visible: !!entry.modelData.version
                                Layout.fillWidth: true
                                text: "版本：" + (entry.modelData.version || "")
                                color: page.theme.text
                                font.pixelSize: page.theme.detailSize
                                wrapMode: Text.Wrap
                                textFormat: Text.PlainText
                            }
                            Text {
                                visible: !!entry.modelData.changes
                                Layout.fillWidth: true
                                text: "修改：" + (entry.modelData.changes || "")
                                color: page.theme.text
                                font.pixelSize: page.theme.detailSize
                                wrapMode: Text.Wrap
                                textFormat: Text.PlainText
                            }
                            Flow {
                                Layout.fillWidth: true
                                spacing: 4
                                AppButton {
                                    visible: !!entry.modelData.url
                                    theme: page.theme
                                    kind: "plain"
                                    compact: true
                                    iconName: "external"
                                    text: "來源"
                                    Accessible.name: entry.modelData.name + " 的來源"
                                    onClicked: Qt.openUrlExternally(entry.modelData.url)
                                }
                                Repeater {
                                    model: entry.modelData.files || []
                                    delegate: AppButton {
                                        required property var modelData
                                        objectName: "file-" + modelData.name
                                        theme: page.theme
                                        kind: "plain"
                                        compact: true
                                        iconName: modelData.kind === "folder" ? "folder" : "document"
                                        text: modelData.name
                                        onClicked: page.open(modelData, entry.modelData.name)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    property alias viewer: viewer
    DocumentViewer {
        id: viewer
        property string revealName: ""
        theme: page.theme
        onRevealRequested: if (page.documents && revealName)
            page.documents.reveal(revealName)
    }
}
