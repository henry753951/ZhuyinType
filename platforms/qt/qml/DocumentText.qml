import QtQuick
import QtQuick.Controls.Basic

// A long plain text the reader may select and copy (a licence, a notice) on a field-coloured block. Set
// `monospace: true` only for licence text. For Markdown documents use MarkdownText.
TextArea {
    id: document
    required property Theme theme
    property bool monospace: false
    readOnly: true
    selectByMouse: true
    wrapMode: TextEdit.Wrap
    textFormat: TextEdit.PlainText
    color: theme.text
    selectionColor: theme.selection
    selectedTextColor: theme.accentText
    font.family: !monospace ? "" : Qt.platform.os === "windows" ? "Consolas" : Qt.platform.os === "osx" ? "Menlo" : "monospace"
    font.pixelSize: monospace ? theme.detailSize : theme.bodySize
    padding: 16
    background: Rectangle {
        radius: document.theme.controlRadius
        color: document.theme.field
        border.color: document.activeFocus ? document.theme.focusRing : document.theme.line
    }
}
