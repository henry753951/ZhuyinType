import QtQuick

// A row inside a SectionCard with free trailing content (key caps, a value, a control) given as children, and a
// chevron when `navigates`. It is a SettingsRow; prefer SettingRow for a setting and NavigationRow for a link.
SettingsRow {
    id: row
    property string detail: ""
    property color detailColor: theme.secondary
    property bool navigates: false
    subtitle: detail
    subtitleColor: detailColor
    accessory: navigates ? "chevron" : "none"
    clickable: navigates
}
