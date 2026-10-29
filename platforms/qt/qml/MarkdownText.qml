import QtQuick
import QtQuick.Layouts

// Markdown shown as formatted text (headings, tables, links in the accent colour). Links open in the browser.
// `text` stays the original Markdown source.
Text {
    id: document
    required property Theme theme
    Layout.fillWidth: true
    textFormat: Text.MarkdownText
    wrapMode: Text.Wrap
    color: theme.text
    linkColor: theme.accent
    // Markdown links take their colour from the palette, not from linkColor.
    palette.link: theme.accent
    palette.text: theme.text
    font.pixelSize: theme.bodySize
    lineHeight: 1.2
    onLinkActivated: link => Qt.openUrlExternally(link)
    HoverHandler {
        cursorShape: document.hoveredLink.length > 0 ? Qt.PointingHandCursor : Qt.ArrowCursor
    }
}
