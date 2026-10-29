import QtQuick

// A keyboard key as it appears in text: a soft face with a slightly darker bottom edge. `active` fills it with the
// accent (a switch that is on); `small` fits it inside a candidate footer.  `outline` draws only the key's edge (the
// candidate window's keycaps, as on macOS); `tint` colours the label (a key another command also uses); `fadeEnd`
// fades the end of the label under a ✕ drawn over it.
Rectangle {
    id: cap
    required property Theme theme
    property string text: ""
    property bool active: false
    property bool small: false
    property bool outline: false
    property bool hovered: false
    property bool pressed: false
    property bool focused: false
    property bool fadeEnd: false
    property color tint: "transparent"
    property real fontSize: small ? 10 : theme.detailSize
    property color labelColor: active ? theme.accentText : tint.a > 0 ? tint : outline ? theme.secondary : theme.text
    implicitWidth: Math.max(implicitHeight, label.implicitWidth + (small ? 8 : 16))
    implicitHeight: small ? 17 : 24
    radius: small ? 4 : 6
    color: outline ? (active ? theme.accent : "transparent") : active ? Qt.darker(theme.accent, 1.15) : theme.fieldBorder
    border.width: outline && !active ? 1 : 0
    border.color: theme.fieldBorder
    Accessible.role: Accessible.StaticText
    Accessible.name: text
    Rectangle {
        visible: !cap.outline
        anchors.fill: parent
        anchors.bottomMargin: cap.small ? 1 : 1.5
        radius: cap.radius
        color: cap.active ? cap.theme.accent : cap.pressed ? cap.theme.controlPressed : cap.hovered ? cap.theme.controlHover : cap.theme.control
        border.width: cap.focused ? 2 : cap.tint.a > 0 ? 1 : 0
        border.color: cap.focused ? cap.theme.focusRing : Qt.rgba(cap.tint.r, cap.tint.g, cap.tint.b, 0.5)
        Behavior on color {
            ColorAnimation {
                duration: cap.theme.motionEnabled ? 110 : 0
            }
        }
    }
    Text {
        id: label
        anchors.centerIn: parent
        anchors.verticalCenterOffset: cap.outline ? 0 : -0.5
        text: cap.text
        color: cap.labelColor
        font.pixelSize: cap.fontSize
        font.weight: Font.Medium
    }
    // The end of the label fades under the ✕.
    Rectangle {
        visible: cap.fadeEnd
        anchors.right: parent.right
        anchors.rightMargin: 1
        anchors.verticalCenter: parent.verticalCenter
        width: 18
        height: parent.height - 4
        radius: cap.radius
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0; color: "transparent" }
            GradientStop { position: 0.45; color: cap.hovered ? cap.theme.controlHover : cap.theme.control }
        }
    }
}
