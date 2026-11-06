//! Punctuation width affects only future literals, independently of keyboard style and composition state.

use std::sync::Arc;

use zhuyin_core::api::{ComposerOptions, KeyInput, KeyResult, PunctuationKeys, PunctuationWidth, TypoTolerance};
use zhuyin_core::composer::Composer;
use zhuyin_core::engine::{Engine, EngineConfig};
use zhuyin_core::keyboard;

fn composer(keys: PunctuationKeys) -> Composer {
    let engine = Arc::new(Engine::from_text("ㄋㄧˇ\t你\t-1\nㄋㄧˇ\t妳\t-2", None, None, EngineConfig::default()));
    let mut c = Composer::new(
        engine,
        ComposerOptions {
            punctuation_keys: keys,
            learning_enabled: false,
            typo_tolerance: TypoTolerance::Off,
            ..ComposerOptions::default()
        },
    );
    c.set_punctuation_width(PunctuationWidth::Full);
    c
}

#[test]
fn legacy_preserves_mixed_width_windows_mapping() {
    let mut c = composer(PunctuationKeys::Windows);
    c.set_punctuation_width(PunctuationWidth::Legacy);
    c.handle(KeyInput::ShiftedChar('3'));
    c.handle(KeyInput::ControlChar('.'));
    assert_eq!(c.presentation().text, "#。");
    assert_eq!(
        Composer::new(c.engine().clone(), ComposerOptions::default()).punctuation_width(),
        PunctuationWidth::Legacy
    );
}

#[test]
fn both_keyboard_styles_have_independent_widths() {
    for keys in PunctuationKeys::ALL {
        for (key, full, half) in [
            (KeyInput::ShiftedChar('3'), "＃", "#"),
            (KeyInput::ControlChar('.'), "。", "."),
            (KeyInput::ControlChar('<'), "《", "<"),
            (KeyInput::Char('['), "「", "["),
            (KeyInput::Char('\''), "、", ","),
            (
                KeyInput::ShiftedChar(','),
                if keys == PunctuationKeys::Mac { "，" } else { "＜" },
                if keys == PunctuationKeys::Mac { "," } else { "<" },
            ),
            (KeyInput::ShiftedChar('-'), if keys == PunctuationKeys::Mac { "——" } else { "＿" }, "_"),
            (KeyInput::ShiftedChar('6'), if keys == PunctuationKeys::Mac { "…" } else { "＾" }, "^"),
        ] {
            let mut c = composer(keys);
            assert_eq!(c.punctuation_width(), PunctuationWidth::Full);
            c.handle(key);
            assert_eq!(c.presentation().text, full);
            let before = c.presentation().clone();
            assert_eq!(c.handle(KeyInput::TogglePunctuationWidth), KeyResult::CONSUMED);
            assert_eq!(c.presentation(), &before);
            assert_eq!(c.punctuation_width(), PunctuationWidth::Half);
            c.handle(key);
            assert_eq!(c.presentation().text, format!("{full}{half}"));
            c.handle(KeyInput::TogglePunctuationWidth);
            c.handle(key);
            assert_eq!(c.presentation().text, format!("{full}{half}{full}"));
        }
    }
}

#[test]
fn letters_printed_symbols_spaces_and_zhuyin_are_not_widened() {
    for keys in PunctuationKeys::ALL {
        for width in [PunctuationWidth::Full, PunctuationWidth::Half] {
            for printed in "0123456789.,# ".chars() {
                assert_eq!(
                    keyboard::literal_with_width(KeyInput::Printed(printed), keys, width),
                    Some(printed.to_string())
                );
            }
            assert_eq!(keyboard::literal_with_width(KeyInput::ShiftedChar('a'), keys, width).as_deref(), Some("A"));
            assert_eq!(keyboard::literal_with_width(KeyInput::Char('3'), keys, width), None);
            let mut c = composer(keys);
            c.set_punctuation_width(width);
            c.handle(KeyInput::ShiftedChar('a'));
            c.handle(KeyInput::ShiftSpace);
            c.handle(KeyInput::Printed('1'));
            assert_eq!(c.presentation().text, "A 1");
            c.reset();
            assert_eq!(c.punctuation_width(), width);
            assert_eq!(c.handle(KeyInput::ShiftSpace), KeyResult::PASS_THROUGH);
        }
    }
}

#[test]
fn toggling_preserves_pending_input_selection_candidates_and_options() {
    for ending in [None, Some(KeyInput::Down), Some(KeyInput::ShiftLeft)] {
        let mut c = composer(PunctuationKeys::Windows);
        for key in "su3".chars().map(KeyInput::Char) {
            c.handle(key);
        }
        if let Some(key) = ending {
            c.handle(key);
        }
        let before = c.presentation().clone();
        c.handle(KeyInput::TogglePunctuationWidth);
        assert_eq!(c.presentation(), &before);
        let options = c.options().clone();
        c.set_options(options);
        assert_eq!(c.punctuation_width(), PunctuationWidth::Half);
        c.commit_all();
        assert_eq!(c.punctuation_width(), PunctuationWidth::Half);
    }
    let mut c = composer(PunctuationKeys::Mac);
    c.handle(KeyInput::Char('s'));
    c.handle(KeyInput::Char('u'));
    let before = c.presentation().clone();
    c.handle(KeyInput::TogglePunctuationWidth);
    assert_eq!(c.presentation(), &before);
    c.handle(KeyInput::Char('3'));
    assert_eq!(c.presentation().text, "你");
}
