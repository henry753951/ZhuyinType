//! 意見與問題 and 移除 ZhuyinType (macOS `FeedbackSection`, `RemovalPane`): links that open a GitHub issue form with the
//! version and the system filled in (`zt_core::feedback`), and how to remove the app.

use gpui_kit::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled};
use zt_core::feedback::{self, Topic, UNINSTALL_REASONS};

use super::{REMOVAL, VERSION};
use crate::app::{self, Place};
use crate::pages::Page;
use crate::theme::size;
use crate::ui::actions::{ButtonKind, push_button};
use crate::ui::{self, v};

/// The title of 關於's section.
pub const SECTION: &str = "意見與問題";

/// Opens `topic`'s form in the browser.  The user writes and sends it there.
pub fn open(topic: Topic, cx: &App) {
    cx.open_url(&feedback::url(topic, VERSION, &zt_core::health::system()));
}

/// 關於 › 意見與問題.
pub fn section() -> impl IntoElement {
    let topic = |id: &'static str, title: &'static str, subtitle: &'static str, topic: Topic| {
        ui::row(id, title).subtitle(subtitle).on_click(move |_, _, cx| open(topic, cx))
    };
    ui::section(SECTION)
        .row(topic("feedback-conversion", "轉換或選字不對", "打對了注音，卻出現不對的字", Topic::Conversion))
        .row(topic("feedback-crash", "當機、沒反應或無法輸入", "輸入法不動、打不出注音或切換不到", Topic::Crash))
        .row(topic("feedback-other", "其他問題", "介面、設定、安裝或更新", Topic::Other))
        .row(topic("feedback-suggestion", "建議", "希望增加或改變的功能", Topic::Suggestion))
        .row(
            ui::row("feedback-diagnostics", "診斷資料")
                .subtitle("檢查結果與最近的錯誤；看過內容後，自己貼到回報裡")
                .on_click(|_, window, cx| super::health::show_report(window, cx)),
        )
        .footer(
            "在瀏覽器開啟 GitHub 的回報頁面，填好版本與系統；內容由你填寫並送出，需要 GitHub 帳號。\
             ZhuyinType 不會自動送出任何資料。",
        )
}

/// The sub-page 移除 ZhuyinType.
pub fn removal(cx: &App) -> AnyElement {
    v().gap(size::SECTION_GAP)
        .child(steps(cx))
        .child(
            ui::section("告訴我們原因（選填）")
                .rows(UNINSTALL_REASONS.iter().enumerate().map(|(index, reason)| {
                    ui::row(SharedString::from(format!("reason-{index}")), *reason)
                        .on_click(move |_, _, cx| open(Topic::Uninstall(reason), cx))
                }))
                .footer("在瀏覽器開啟 GitHub 的頁面，內容由你填寫並送出。"),
        )
        .into_any_element()
}

#[cfg(windows)]
fn steps(cx: &App) -> impl IntoElement + use<> {
    ui::section("移除")
        .row(
            ui::row("remove-app", "在 Windows 設定中移除")
                .subtitle("已安裝的應用程式 › ZhuyinType 注音輸入法 › 解除安裝")
                .trailing(push_button(
                    "open-apps",
                    "開啟設定…",
                    ButtonKind::Normal,
                    true,
                    |_, _, cx| cx.open_url("ms-settings:appsfeatures"),
                    cx,
                )),
        )
        .footer("自訂詞、記住的字詞與設定會保留；解除安裝時可以選擇一併刪除。")
}

#[cfg(not(windows))]
fn steps(cx: &App) -> impl IntoElement + use<> {
    let command = match zt_core::update::asset_key() {
        Some("linux-rpm") if std::path::Path::new("/usr/bin/zypper").exists() => "sudo zypper remove zhuyintype",
        Some("linux-rpm") => "sudo dnf remove zhuyintype",
        _ => "sudo apt remove zhuyintype",
    };
    ui::section("移除")
        .row(ui::row("remove-app", "移除套件").subtitle(format!("在終端機執行 {command}")).trailing(push_button(
            "copy-command",
            "拷貝指令",
            ButtonKind::Normal,
            true,
            move |_, _, cx| cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(command.to_owned())),
            cx,
        )))
        .row(ui::row("remove-fcitx", "移除輸入方式").subtitle("在 Fcitx 5 設定的輸入法清單中刪除 ZhuyinType"))
        .footer(
            "自訂詞、記住的字詞與設定會保留在 ~/.local/share/zhuyintype 與 ~/.config/zhuyintype，\
             要一併刪除時請自行移除這兩個資料夾。",
        )
}

/// 關於's row that leads here.
pub fn removal_row() -> impl IntoElement {
    ui::row("drill-removal", "移除 ZhuyinType")
        .subtitle("移除的方式，與個人資料的位置")
        .anchor(REMOVAL)
        .on_click(|_, _, cx| app::go(Place::subpage(Page::About, REMOVAL), cx))
}
