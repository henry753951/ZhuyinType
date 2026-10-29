import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// A licence or notice read inside the app (授權與資料來源), the same sheet as on macOS: the file name as the title,
// what it belongs to below it, the text selectable and scrolling — monospace for a licence, formatted for Markdown —
// and 拷貝全文, a folder button for files on disk, and 完成. `show(name, subtitle, text, markdown, revealable)` opens it.
AppDialog {
    id: viewer
    property string subtitle: ""
    property string text: ""
    property bool markdown: false
    property bool revealable: false
    signal revealRequested
    width: Math.min(720, parent ? parent.width - 48 : 720)
    height: parent ? parent.height - 48 : 600
    padding: 0
    topPadding: 0
    bottomPadding: 0

    function show(name, subtitle, text, markdown, revealable) {
        title = name;
        viewer.subtitle = subtitle;
        viewer.text = text;
        viewer.markdown = markdown;
        viewer.revealable = revealable;
        open();
        scroller.ScrollBar.vertical.position = 0;
    }
    function copyAll() {
        clipboard.text = viewer.text;
        clipboard.selectAll();
        clipboard.copy();
        clipboard.text = "";
    }
    header: ColumnLayout {
        spacing: 3
        Text {
            Layout.fillWidth: true
            Layout.topMargin: 20
            Layout.leftMargin: 22
            Layout.rightMargin: 22
            text: viewer.title
            color: viewer.theme.text
            font.pixelSize: viewer.theme.headingSize + 1
            font.weight: Font.DemiBold
            elide: Text.ElideMiddle
            Accessible.role: Accessible.Heading
        }
        Text {
            Layout.fillWidth: true
            Layout.leftMargin: 22
            Layout.rightMargin: 22
            Layout.bottomMargin: 12
            visible: text.length > 0
            text: viewer.subtitle
            color: viewer.theme.secondary
            font.pixelSize: viewer.theme.detailSize
            elide: Text.ElideRight
        }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 1
            color: viewer.theme.line
        }
    }
    contentItem: ScrollView {
        id: scroller
        objectName: "documentScroller"
        clip: true
        contentWidth: availableWidth
        ColumnLayout {
            width: scroller.availableWidth
            spacing: 0
            TextArea {
                id: plain
                objectName: "documentText"
                visible: !viewer.markdown
                Layout.fillWidth: true
                readOnly: true
                selectByMouse: true
                wrapMode: TextEdit.Wrap
                textFormat: TextEdit.PlainText
                text: viewer.markdown ? "" : viewer.text
                color: viewer.theme.text
                selectionColor: viewer.theme.selection
                selectedTextColor: viewer.theme.accentText
                font.family: Qt.platform.os === "windows" ? "Consolas" : Qt.platform.os === "osx" ? "Menlo" : "monospace"
                font.pixelSize: viewer.theme.detailSize
                padding: 22
                background: Item {}
                Accessible.name: viewer.title
            }
            MarkdownText {
                objectName: "documentMarkdown"
                visible: viewer.markdown
                theme: viewer.theme
                Layout.margins: 22
                text: viewer.markdown ? viewer.text : ""
                Accessible.name: viewer.title
            }
        }
    }
    footer: ColumnLayout {
        spacing: 0
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 1
            color: viewer.theme.line
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 14
            Layout.leftMargin: 22
            Layout.rightMargin: 22
            spacing: 8
            AppButton {
                objectName: "copyDocument"
                theme: viewer.theme
                iconName: "copy"
                text: "拷貝全文"
                enabled: viewer.text.length > 0
                onClicked: viewer.copyAll()
            }
            AppButton {
                visible: viewer.revealable
                theme: viewer.theme
                iconName: "folder"
                text: Qt.platform.os === "windows" ? "在檔案總管中顯示" : Qt.platform.os === "osx" ? "在 Finder 中顯示" : "在檔案管理員中顯示"
                onClicked: viewer.revealRequested()
            }
            Item {
                Layout.fillWidth: true
            }
            AppButton {
                objectName: "closeDocument"
                theme: viewer.theme
                kind: "primary"
                text: "完成"
                onClicked: viewer.close()
            }
        }
    }
    // The clipboard is reached through a text control; this one is never shown.
    TextEdit {
        id: clipboard
        visible: false
        textFormat: TextEdit.PlainText
    }
}
