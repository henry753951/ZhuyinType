//! The setup wizard's window (macOS `WizardWindowController`).
//!
//! TODO(wizard): STUB.  The wizard window is being built on another branch; it replaces this file.  The 介紹 page
//! calls `open` for its three 設定嚮導 rows and `new_lessons_since` to decide whether 新功能導覽 shows.

use gpui_kit::App;

/// Which wizard to show (macOS `WizardKind`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WizardKind {
    /// 開始設定嚮導: choose a way of typing and try it.
    Quick,
    /// 新功能導覽: only the lessons new since this version.
    NewSince(String),
    /// 你會用到的按鍵: the keys for the current settings.
    KeysOnly,
}

/// Opens (or brings forward) the wizard window showing `kind`.
pub fn open(kind: WizardKind, cx: &mut App) {
    // TODO(wizard): open the wizard window.  Until then nothing happens.
    let _ = cx;
    log::info!("wizard not built yet: {kind:?}");
}

/// The version whose lessons the user saw last, when this version has lessons they have not seen (macOS
/// `WizardProgress.hasNewLessons` / `seenVersion`); None hides 新功能導覽.
pub fn new_lessons_since(cx: &App) -> Option<String> {
    // TODO(wizard): read the wizard's progress.
    let _ = cx;
    None
}
