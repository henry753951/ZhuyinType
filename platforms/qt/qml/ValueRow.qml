import QtQuick

// A read-only "name: value" row inside a SectionCard (a metric, a version). A missing value shows a tertiary "—".
// It is a SettingsRow; `divider` is its `separated` (false on a card's first row).
SettingsRow {
    property string detail: ""
    property bool divider: true
    subtitle: detail
    separated: divider
    value: "—"
    valueColor: value === "—" ? theme.tertiary : theme.secondary
    Accessible.role: Accessible.StaticText
}
