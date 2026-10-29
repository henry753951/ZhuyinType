import QtQuick

// A small rounded label for a state or a count: "中文", "不記住", "Beta". `tone` is neutral, accent, success,
// warning or danger.
Rectangle {
    id: chip
    required property Theme theme
    property string text: ""
    property string tone: "neutral"
    readonly property color ink: tone === "accent" ? theme.accent : tone === "success" ? theme.success : tone === "warning" ? theme.warning : tone === "danger" ? theme.danger : theme.secondary
    implicitWidth: label.implicitWidth + 14
    implicitHeight: 20
    radius: height / 2
    color: tone === "neutral" ? theme.control : Qt.rgba(ink.r, ink.g, ink.b, theme.dark ? 0.18 : 0.11)
    Text {
        id: label
        anchors.centerIn: parent
        text: chip.text
        color: chip.ink
        font.pixelSize: chip.theme.captionSize
        font.weight: Font.DemiBold
    }
}
