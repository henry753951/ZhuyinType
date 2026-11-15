//! Fixed states of the 快捷鍵 page for screenshots, chosen with `ZT_DEMO` (comma-separated) or `--demo NAME`
//! (the macOS renderer's `customKeys`, `featuresOff` and `-chips` states).  Applied once, to the temporary settings
//! file the screenshots use.

use gpui_kit::App;
use serde_json::{Value, json};
use zt_core::keys::{code, modifier};

use crate::app;
use crate::ui::shortcuts::{FieldPreview, set_preview};

/// Whether demo state `name` was asked for.
pub fn active(name: &str) -> bool {
    let env = std::env::var("ZT_DEMO").unwrap_or_default();
    if env.split(',').any(|item| item.trim() == name) {
        return true;
    }
    let args: Vec<String> = std::env::args().collect();
    args.windows(2).any(|pair| pair[0] == "--demo" && pair[1].split(',').any(|item| item == name))
}

/// The value of demo setting `name=VALUE`.
pub fn value(name: &str) -> Option<String> {
    let env = std::env::var("ZT_DEMO").unwrap_or_default();
    env.split(',').find_map(|item| item.trim().strip_prefix(name)?.strip_prefix('=').map(str::to_owned))
}

fn k(key: i32, modifiers: u32) -> Value {
    json!({ "key": key, "modifiers": modifiers })
}

fn custom_keys(cx: &mut App) {
    use code::*;
    use modifier::{ALT, CONTROL, SHIFT};
    let letter = |c: u8| i32::from(c);
    let stored = json!({
        "correctionsList": [k(letter(b'U'), CONTROL | ALT), k(UP, SHIFT)],
        "singleCharacterList": [k(DOWN, ALT)],
        "typeZhuyin": [k(letter(b'Z'), CONTROL | ALT)],
        "selectLeft": [k(LEFT, CONTROL | SHIFT)],
        "nextPage": [k(letter(b'N'), CONTROL | ALT), k(TAB, 0), k(PAGE_DOWN, 0)],
        "previousPage": [k(TAB, SHIFT), k(PAGE_UP, ALT)],
    });
    app::set_setting("shortcuts", stored, cx);
    app::set_setting("selectionKeys", json!("asdfghjkl"), cx);
    app::set_setting("candidatesPerPage", json!(6), cx);
    app::set_setting("shiftToggle", json!("right"), cx);
    app::set_setting("spaceAction", json!("candidates"), cx);
}

/// Applies the demo states asked for.
pub fn apply(cx: &mut App) {
    if active("custom-keys") {
        custom_keys(cx);
    }
    if active("features-off") {
        for feature in ["feature.correctionsList", "feature.typeZhuyin", "feature.controlPunctuation"] {
            app::set_setting(feature, json!(false), cx);
        }
        app::set_setting("shiftSwitch", json!(false), cx);
        let stored = json!({
            "nextPage": [k(code::TAB, 0), k(code::LEFT, modifier::CONTROL | modifier::SHIFT)],
            "selectLeft": [k(code::LEFT, modifier::SHIFT), k(code::LEFT, modifier::CONTROL | modifier::SHIFT)],
        });
        app::set_setting("shortcuts", stored, cx);
    }
    if active("chips") {
        custom_keys(cx);
        use code::*;
        use modifier::{ALT, CONTROL, SHIFT};
        let stored = json!({
            "correctionsList": [k(i32::from(b'U'), CONTROL | ALT), k(UP, SHIFT)],
            "singleCharacterList": [],
            "typeZhuyin": [],
            "selectLeft": [k(LEFT, CONTROL | SHIFT)],
            "nextPage": [k(TAB, 0), k(PAGE_DOWN, 0), k(DOWN, ALT), k(i32::from(b'N'), CONTROL | ALT)],
            "previousPage": [k(TAB, SHIFT), k(PAGE_UP, 0), k(UP, ALT)],
        });
        app::set_setting("shortcuts", stored, cx);
        set_preview("previousPage", FieldPreview::Expanded, cx);
        set_preview("selectLeft", FieldPreview::ChipHover(0), cx);
        set_preview("selectRight", FieldPreview::Hover, cx);
        set_preview("typeZhuyin", FieldPreview::Hover, cx);
        set_preview("correctionsList", FieldPreview::Recording { index: 1, held: ALT }, cx);
        set_preview("deleteWord", FieldPreview::Accepted(0), cx);
        set_preview("commitWithNewline", FieldPreview::Refused("已用於「下一頁」".into()), cx);
    }
}
