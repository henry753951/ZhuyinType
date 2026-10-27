import SwiftUI

// Views built from the settings registry: one control per kind of setting (a switch, a choice, a number, keys) and the
// row that holds it, so a setting's title, description and options are written once, in SettingsRegistry.

/// The control for a setting, by its kind.
struct SettingControl: View {
    let setting: SettingDescriptor
    @ObservedObject var preferences: Preferences
    /// Show a two- or three-way choice as segments instead of a menu.
    var segmented = false

    var body: some View {
        let value = Binding<SettingValue>(get: { setting.read(preferences) }, set: { setting.write(preferences, $0) })
        switch setting.kind {
        case .toggle:
            RowSwitch(title: setting.title, isOn: Binding(get: { value.wrappedValue == .bool(true) }, set: { value.wrappedValue = .bool($0) }))
        case .choice(let options):
            RowPicker(title: setting.title, selection: value, options: options.map { ($0.title, $0.value) }, segmented: segmented)
        case .number(let range, let step, _):
            Stepper(setting.title, value: Binding(
                get: { if case .number(let n) = value.wrappedValue { return n } else { return range.lowerBound } },
                set: { value.wrappedValue = .number($0) }), in: range, step: step)
                .labelsHidden()
        case .flags:
            EmptyView()
        case .keys(let command):
            ShortcutField(preferences: preferences, command: command, preview: DemoPinning.fieldStates[command])
        }
    }
}

/// A settings row for a registered setting: its title, its description for the current value, a number's value, and
/// its control.
struct RegistryRow: View {
    let setting: SettingDescriptor
    @ObservedObject var preferences: Preferences
    var segmented = false
    var accessory: RowAccessory = .none
    var dimmed = false

    init(_ id: String, _ preferences: Preferences, segmented: Bool = false, accessory: RowAccessory = .none, dimmed: Bool = false) {
        guard let setting = SettingsRegistry.setting(id) else { preconditionFailure("No setting \(id)") }
        self.setting = setting
        self.preferences = preferences
        self.segmented = segmented
        self.accessory = accessory
        self.dimmed = dimmed
    }

    var body: some View {
        let current = setting.read(preferences)
        let value: String? = {
            if case .number = setting.kind { return setting.label(current) }
            return nil
        }()
        SettingRow(title: setting.title, subtitle: setting.detail?(preferences), value: value, dimmed: dimmed,
                   accessory: accessory, anchor: setting.place.row) {
            SettingControl(setting: setting, preferences: preferences, segmented: segmented).disabled(dimmed)
        }
    }
}
