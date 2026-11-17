//! 符號與 Emoji: the `zhuyin-symbols` catalog, search and recent list as JSON and text.

use std::ffi::c_char;
use std::fmt::Write as _;

use zhuyin_symbols::{Recent, Symbol};

use crate::strings::{borrowed, owned};

fn push_json_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn push_symbols<'a>(out: &mut String, symbols: impl Iterator<Item = &'a Symbol>) {
    out.push('[');
    for (i, symbol) in symbols.enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"text\":");
        push_json_string(out, &symbol.text);
        out.push_str(",\"name\":");
        push_json_string(out, &symbol.name);
        out.push('}');
    }
    out.push(']');
}

/// Every category of the picker except 最近使用, as JSON:
/// `[{"id":"punctuation","title":"常用標點","symbols":[{"text":"，","name":"逗號"},…]},…]`.  Free with
/// [`zt_string_free`](crate::zt_string_free).
#[unsafe(no_mangle)]
pub extern "C" fn zt_symbols_categories_json() -> *mut c_char {
    let mut out = String::from("[");
    for (i, category) in zhuyin_symbols::categories().iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"id\":");
        push_json_string(&mut out, category.id);
        out.push_str(",\"title\":");
        push_json_string(&mut out, category.title);
        out.push_str(",\"symbols\":");
        push_symbols(&mut out, category.symbols.iter());
        out.push('}');
    }
    out.push(']');
    owned(&out)
}

/// The symbols matching `query` (raw standard-layout keys, Zhuyin, Chinese or English), best first, at most
/// `limit`, as a JSON array of `{"text","name"}`.  NULL `query` is empty.
///
/// # Safety
/// `query` is NULL or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_symbols_search_json(query: *const c_char, limit: usize) -> *mut c_char {
    // SAFETY: the caller passes NULL or a NUL-terminated string.
    let query = unsafe { borrowed(query) }.unwrap_or("");
    let mut out = String::new();
    push_symbols(&mut out, zhuyin_symbols::search(query, limit).into_iter());
    owned(&out)
}

/// How `query` is shown in the picker: Zhuyin when the keys decode, otherwise as typed.
///
/// # Safety
/// `query` is NULL or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_symbols_display_query(query: *const c_char) -> *mut c_char {
    // SAFETY: the caller passes NULL or a NUL-terminated string.
    let query = unsafe { borrowed(query) }.unwrap_or("");
    owned(&zhuyin_symbols::display_query(query))
}

/// The recent list's stored text (`stored`, NULL for none) after `symbol` was inserted: one symbol per line, newest
/// first, at most [`zt_symbols_recent_capacity`].
///
/// # Safety
/// `stored` and `symbol` are NULL or NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_symbols_recent_push(stored: *const c_char, symbol: *const c_char) -> *mut c_char {
    // SAFETY: the caller passes NULL or NUL-terminated strings.
    let (stored, symbol) = unsafe { (borrowed(stored).unwrap_or(""), borrowed(symbol).unwrap_or("")) };
    let mut recent = Recent::from_text(stored);
    recent.push(symbol);
    owned(&recent.to_text())
}

/// The most symbols the recent list keeps.
#[unsafe(no_mangle)]
pub extern "C" fn zt_symbols_recent_capacity() -> usize {
    zhuyin_symbols::RECENT_CAPACITY
}
