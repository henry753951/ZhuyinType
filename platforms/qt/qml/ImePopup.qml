import QtQuick
import QtQuick.Controls.Basic

// The input method's floating windows: the candidate window (macOS CandidatePanelController) and the 中／英 toast
// (ModeToast).  Neither takes focus.  The candidate window goes below the caret's line or above it (位置), its
// candidates lined up with the composition, and appears with a short fade and a 4-point slide (動畫; a fade only under
// Reduce Motion).
Window {
    id: popup
    required property var host
    required property var settings
    property var shortcuts: null
    readonly property bool above: settings.values.candidatePlacement === "above"
    readonly property bool animates: settings.values.panelAnimation !== false
    flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowDoesNotAcceptFocus
    visible: host.visible
    color: "transparent"
    width: panel.implicitWidth
    height: panel.implicitHeight
    readonly property point placement: {
        const anchor = host.anchor;
        return host.popupPosition(width, height, above, panel.textInset);
    }
    x: placement.x
    y: placement.y
    title: "ZhuyinType 候選字"
    Theme {
        id: popupTheme
        appearance: popup.settings.values.panelAppearance
        motionEnabled: popup.animates
    }
    onVisibleChanged: {
        if (!visible || !animates) {
            appear.stop();
            panel.opacity = 1;
            slide.y = 0;
            return;
        }
        slide.y = popupTheme.reduceMotion ? 0 : (above ? 4 : -4);
        panel.opacity = 0;
        appear.restart();
    }
    ParallelAnimation {
        id: appear
        NumberAnimation { target: panel; property: "opacity"; to: 1; duration: 120; easing.type: Easing.OutCubic }
        NumberAnimation { target: slide; property: "y"; to: 0; duration: 120; easing.type: Easing.OutCubic }
    }
    CandidatePanel {
        id: panel
        // Where the candidate text starts in the window, so it lines up with the composition.
        readonly property real textInset: hasRows ? inset + (compact ? 6 : 8) + textSize * 0.68 + (compact ? 5 : 8) : 14
        width: popup.width
        height: popup.height
        transform: Translate { id: slide }
        candidates: popup.host
        composition: popup.host.inlinePreedit ? "" : popup.host.composition
        hint: popup.host.notice
        preferences: popup.settings.values
        shortcuts: popup.shortcuts
        theme: popupTheme
        onReadingClicked: popup.host.typeReading()
    }

    // 中／英: 52 × 48, shown 0.8 s, then faded out over 0.2 s (removed at once with animation off).
    Window {
        id: toast
        objectName: "modeToast"
        flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowDoesNotAcceptFocus | Qt.WindowTransparentForInput
        color: "transparent"
        width: 52
        height: 48
        property string text: "中"
        function show(english) {
            fade.stop();
            badge.opacity = 1;
            text = english ? "英" : "中";
            const at = popup.host.popupPosition(width, height, popup.above, 0);
            x = at.x;
            y = at.y;
            visible = true;
            hold.restart();
        }
        Rectangle {
            id: badge
            anchors.fill: parent
            radius: 14
            color: popupTheme.surface
            border.color: popupTheme.line
            Text {
                anchors.centerIn: parent
                text: toast.text
                color: popupTheme.text
                font.pixelSize: 23
                font.weight: Font.Medium
            }
        }
        Timer {
            id: hold
            interval: 800
            onTriggered: {
                if (popup.animates)
                    fade.restart();
                else
                    toast.visible = false;
            }
        }
        NumberAnimation {
            id: fade
            target: badge
            property: "opacity"
            to: 0
            duration: 200
            onFinished: toast.visible = false
        }
    }
    Connections {
        target: popup.host
        function onModeSwitched(english) {
            if (popup.settings.values.showModeToast !== false)
                toast.show(english);
        }
    }
}
