//! The licence viewer (macOS `DocumentSheet`): the file name and what it belongs to, the text scrolling and
//! selectable (monospaced for a licence, the UI font for Markdown), and 拷貝全文, 在檔案管理員中顯示, 完成.  Long texts
//! (the Rust crate list runs to hundreds of kilobytes) are drawn line by line in a virtual list.

use gpui_kit::base::{SelectableText, TextSelectionHandle};
use gpui_kit::component::WindowExt as _;
use gpui_kit::{
    App, ClipboardItem, IntoElement, ListAlignment, ListState, ParentElement, SharedString, Styled, Window, list, px,
};

use super::legal::Document;
use crate::ui::actions::{ButtonKind, push_button};
use crate::ui::content::document_frame;

/// Opens `document` (which must have text) over `window`; `entry` is what it belongs to (「ZhuyinType」).
pub fn open(document: Document, entry: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
    let Some(text) = document.shown_text() else { return };
    let entry: SharedString = entry.into();
    let lines: Vec<SharedString> = text
        .lines()
        .map(|line| SharedString::from(if line.is_empty() { " ".to_owned() } else { line.to_owned() }))
        .collect();
    let state = ListState::new(lines.len(), ListAlignment::Top, px(400.));
    let selection = TextSelectionHandle::new(text.clone(), cx);
    let markdown = document.markdown();
    let name: SharedString = document.file_name().to_owned().into();
    let path = document.path.clone();
    let text: SharedString = text.into();
    window.open_dialog(cx, move |dialog, window, cx| {
        let lines = lines.clone();
        let selection = selection.clone();
        let body = list(state.clone(), move |index, _, _| {
            SelectableText::with_handle(
                SharedString::from(format!("line-{index}")),
                selection.clone(),
                lines[index].clone(),
            )
            .document_order(index as u64)
            .into_any_element()
        })
        .size_full();
        let copy_text = text.clone();
        let reveal_path = path.clone();
        let mut buttons = vec![
            push_button(
                "copy-all",
                "拷貝全文",
                ButtonKind::Normal,
                true,
                move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy_text.to_string())),
                cx,
            )
            .into_any_element(),
        ];
        if let Some(file) = reveal_path {
            buttons.push(
                push_button(
                    "reveal-document",
                    if cfg!(windows) { "在檔案總管中顯示" } else { "在檔案管理員中顯示" },
                    ButtonKind::Normal,
                    true,
                    move |_, _, cx| cx.reveal_path(&file),
                    cx,
                )
                .into_any_element(),
            );
        }
        let done = push_button("done", "完成", ButtonKind::Default, true, |_, window, cx| window.close_dialog(cx), cx)
            .into_any_element();
        let frame = document_frame(
            name.clone(),
            entry.clone(),
            markdown,
            body.into_any_element(),
            &state,
            buttons,
            done,
            window,
            cx,
        );
        dialog.w(frame.width).p(px(0.)).gap(px(0.)).close_button(false).on_ok(|_, _, _| true).child(frame.element)
    });
}
