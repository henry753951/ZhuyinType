import QtQuick
import QtQuick.Layouts

// A hairline between the rows of a card.
Rectangle {
    required property Theme theme
    Layout.fillWidth: true
    implicitHeight: 1
    color: theme.line
}
