import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The settings of one page, from settings.json: one SectionCard per `group` (an empty group is a card without a
// heading), the rows in the file's order, and under a setting with `commands` its keys as indented 「快捷鍵」 rows
// (named after the command when there are several), shown while the feature is on — CommandKeyRows on macOS.
ColumnLayout {
    id: page
    required property string pageId
    required property var settings
    required property Theme theme
    property var shortcuts: null
    signal navigateRequested(string pageId, string title, string row)
    readonly property var definitions: settings.definitions.filter(item => item.page === pageId)
    readonly property var groups: [...new Set(definitions.map(item => item.group || ""))]
    spacing: theme.sectionSpacing
    Repeater {
        model: page.groups
        delegate: SectionCard {
            id: group
            required property string modelData
            theme: page.theme
            title: modelData
            Repeater {
                model: page.definitions.filter(item => (item.group || "") === group.modelData)
                delegate: ColumnLayout {
                    id: settingEntry
                    required property var modelData
                    required property int index
                    readonly property var commands: modelData.commands || []
                    Layout.fillWidth: true
                    spacing: 0
                    SettingRow {
                        definition: settingEntry.modelData
                        settings: page.settings
                        shortcuts: page.shortcuts
                        theme: page.theme
                        separated: settingEntry.index > 0
                    }
                    Repeater {
                        model: page.shortcuts && page.settings.values[settingEntry.modelData.key] !== false ? settingEntry.commands : []
                        delegate: SettingsRow {
                            id: shortcutRow
                            required property string modelData
                            theme: page.theme
                            separated: true
                            indent: true
                            anchor: "keys-" + modelData
                            // macOS CommandKeyRows: named after the command (its short label) when there are several.
                            title: settingEntry.commands.length > 1 ? (page.shortcuts.commands.find(command => command.id === modelData)?.label || modelData) : "快捷鍵"
                            marked: !!page.shortcuts.values && page.shortcuts.isCustomized(modelData)
                            ShortcutRecorder {
                                Layout.preferredWidth: Math.min(implicitWidth, shortcutRow.width * 0.6)
                                shortcuts: page.shortcuts
                                command: shortcutRow.modelData
                                title: shortcutRow.title
                                theme: page.theme
                                rowHovered: shortcutRow.hovered
                                onNavigateRequested: (pageId, title, row) => page.navigateRequested(pageId, title, row)
                            }
                        }
                    }
                }
            }
        }
    }
}
