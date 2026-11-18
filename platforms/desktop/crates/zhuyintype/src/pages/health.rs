//! 「檢查輸入法」 page (SETTINGS_IA.md §2): the checks of why typing with ZhuyinType might not work, their fixes and
//! 回報問題… (`about::health`, macOS `HealthPage`).  The sidebar marks the page while something needs attention.

use gpui_kit::{Context, IntoElement, Render, Window};

pub struct HealthPage;

impl HealthPage {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for HealthPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        super::about::health::page(cx)
    }
}
