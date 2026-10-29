import QtQuick

// ZhuyinType's own palette for Linux and Windows: neutral greys with a slight blue cast and one indigo accent taken
// from the app icon. It does not copy any platform's system colours, so it reads the same on KDE, GNOME and Windows.
QtObject {
    property bool motionEnabled: true
    // Qt 6.12 exposes the native preference. Older hosts can bind reduceMotion explicitly.
    property bool reduceMotion: Qt.styleHints.accessibility !== undefined && Qt.styleHints.accessibility.motionPreference !== undefined && Qt.styleHints.accessibility.motionPreference === Qt.ReducedMotion
    // Page and row metrics, the same numbers as the macOS version (RowMetrics in SettingsRows.swift).
    readonly property int pageInset: 20
    readonly property int pageTop: 16
    readonly property int maxContentWidth: 700
    readonly property int sectionSpacing: 22
    readonly property int headerGap: 7
    readonly property int cardInset: 16
    readonly property int rowPadding: 10
    readonly property int rowMinHeight: 24
    readonly property int rowIndent: 18
    readonly property int rowGap: 12
    readonly property int trailingSpacing: 8
    readonly property int tileSpacing: 10
    readonly property int tilePadding: 14
    /// Height of the controls inside a row (switch, picker, stepper, key caps).
    readonly property int rowControlHeight: 24
    property string appearance: "system"
    readonly property bool dark: appearance === "dark" || (appearance === "system" && Qt.styleHints.colorScheme === Qt.Dark)

    // Surfaces, back to front.
    // A plain page with quietly tinted cards and no card borders, as in the macOS version.
    readonly property color canvas: dark ? "#1c1d20" : "#ffffff"
    readonly property color sidebar: dark ? "#222327" : "#f3f4f6"
    readonly property color card: dark ? "#26272b" : "#f6f6f8"
    readonly property color surface: dark ? "#26282d" : "#ffffff"
    readonly property color field: dark ? "#1b1c20" : "#ffffff"
    readonly property color segment: dark ? "#2e3036" : "#e9ebef"
    readonly property color segmentSelected: dark ? "#4a4d56" : "#ffffff"

    // Text.
    readonly property color text: dark ? "#ececf0" : "#1b1c20"
    readonly property color secondary: dark ? "#a3a7b1" : "#626773"
    readonly property color tertiary: dark ? "#757a85" : "#959aa5"

    // Lines and states.
    readonly property color line: dark ? "#33353b" : "#e3e5ea"
    readonly property color fieldBorder: dark ? "#3c3f46" : "#d5d8df"
    readonly property color hover: dark ? "#303136" : "#ebecef"
    // Buttons, pickers and key caps: white with a hairline on the light tinted cards, raised grey in the dark.
    readonly property color control: dark ? "#37383e" : "#ffffff"
    readonly property color controlHover: dark ? "#404249" : "#f1f2f5"
    readonly property color controlPressed: dark ? "#4a4c54" : "#e5e7eb"
    readonly property color controlBorder: dark ? "transparent" : "#d9dbe1"
    readonly property color track: dark ? "#474a52" : "#d3d6dd"

    // Accent and status.
    readonly property color accent: dark ? "#7b8dff" : "#3d5be0"
    readonly property color accentHover: dark ? "#8f9fff" : "#334fcc"
    readonly property color accentText: "#ffffff"
    readonly property color accentSoft: dark ? Qt.rgba(0.48, 0.55, 1, 0.16) : Qt.rgba(0.24, 0.36, 0.88, 0.10)
    readonly property color selection: dark ? "#5468f0" : "#3d5be0"
    readonly property color danger: dark ? "#ff7b7b" : "#c93636"
    readonly property color warning: dark ? "#e8b04b" : "#c27a00"
    readonly property color success: dark ? "#5fcf8c" : "#1f9d55"
    readonly property color focusRing: dark ? Qt.rgba(0.48, 0.55, 1, 0.55) : Qt.rgba(0.24, 0.36, 0.88, 0.45)
    readonly property color shadow: dark ? Qt.rgba(0, 0, 0, 0.45) : Qt.rgba(0.07, 0.09, 0.15, 0.10)

    // Shape and type scale.
    readonly property int radius: 12
    readonly property int cardRadius: 10
    readonly property int controlRadius: 8
    readonly property int controlHeight: 30
    readonly property int titleSize: 20
    readonly property int displaySize: 18
    readonly property int headingSize: 15
    readonly property int sectionTitleSize: 13
    readonly property int bodySize: 13
    readonly property int detailSize: 12
    readonly property int captionSize: 11
}
