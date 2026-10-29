import QtQuick

// A row that opens another page: a SettingsRow with a value and a chevron; the whole row is the button.
SettingsRow {
    property string detail: ""
    subtitle: detail
    accessory: "chevron"
    clickable: true
}
