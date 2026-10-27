//! Strings and string lists across the boundary.

use std::ffi::{CStr, CString, c_char};

/// A borrowed UTF-8 C string; `None` for NULL or invalid UTF-8.
///
/// # Safety
/// `text` is NULL or a NUL-terminated string that lives for `'a`.
pub(crate) unsafe fn borrowed<'a>(text: *const c_char) -> Option<&'a str> {
    if text.is_null() {
        return None;
    }
    // SAFETY: the caller passes a NUL-terminated string valid for 'a.
    unsafe { CStr::from_ptr(text) }.to_str().ok()
}

/// An owned C string for the caller (NUL characters, which engine text never has, are dropped).
pub(crate) fn owned(text: &str) -> *mut c_char {
    let string = CString::new(text).unwrap_or_else(|_| CString::new(text.replace('\0', "")).unwrap_or_default());
    string.into_raw()
}

/// `owned` for an optional string; NULL for `None`.
pub(crate) fn owned_optional(text: Option<&str>) -> *mut c_char {
    text.map_or(std::ptr::null_mut(), owned)
}

/// Stores `message` in `error` when it is not NULL.
///
/// # Safety
/// `error` is NULL or valid for writing one pointer.
pub(crate) unsafe fn set_error(error: *mut *mut c_char, message: &str) {
    if !error.is_null() {
        // SAFETY: checked non-null; the caller provides a writable pointer slot.
        unsafe { *error = owned(message) };
    }
}

/// Frees a string returned by this library.  NULL is ignored.
///
/// # Safety
/// `text` is NULL or a string returned by this library that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_string_free(text: *mut c_char) {
    if !text.is_null() {
        // SAFETY: the string came from `CString::into_raw` in this library.
        drop(unsafe { CString::from_raw(text) });
    }
}

/// A list of owned strings.
#[repr(C)]
#[derive(Debug)]
pub struct ZtStringList {
    /// `count` strings.
    pub items: *mut *mut c_char,
    /// How many.
    pub count: usize,
}

pub(crate) fn string_list<S: AsRef<str>>(strings: &[S]) -> *mut ZtStringList {
    let items: Box<[*mut c_char]> = strings.iter().map(|s| owned(s.as_ref())).collect();
    let count = items.len();
    Box::into_raw(Box::new(ZtStringList { items: Box::into_raw(items).cast(), count }))
}

/// Frees a list returned by this library.  NULL is ignored.
///
/// # Safety
/// `list` is NULL or a list returned by this library that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_string_list_free(list: *mut ZtStringList) {
    if list.is_null() {
        return;
    }
    // SAFETY: the list and its items were allocated by `string_list`.
    unsafe {
        let list = Box::from_raw(list);
        let items = Box::from_raw(std::ptr::slice_from_raw_parts_mut(list.items, list.count));
        for &item in &*items {
            zt_string_free(item);
        }
    }
}

/// A view of strings during a callback (the candidates a candidate scorer orders).  Read with [`zt_strings_count`]
/// and [`zt_strings_get`], or hand it to `zt_reranker_candidate_scores_view`.
#[derive(Debug)]
pub struct ZtStrings {
    pub(crate) strings: *const String,
    pub(crate) count: usize,
}

impl ZtStrings {
    pub(crate) fn new(strings: &[String]) -> Self {
        ZtStrings { strings: strings.as_ptr(), count: strings.len() }
    }

    /// # Safety
    /// The view is used while the slice it was made from lives (during the callback).
    pub(crate) unsafe fn slice(&self) -> &[String] {
        // SAFETY: made from a live slice by `new`; the callback contract keeps it alive.
        unsafe { std::slice::from_raw_parts(self.strings, self.count) }
    }
}

/// How many strings the view has.
///
/// # Safety
/// `strings` is a view passed to the running callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_strings_count(strings: *const ZtStrings) -> usize {
    // SAFETY: the caller passes the live view.
    unsafe { strings.as_ref() }.map_or(0, |s| s.count)
}

/// A copy of string `index` (free with [`zt_string_free`]); NULL when out of range.
///
/// # Safety
/// `strings` is a view passed to the running callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zt_strings_get(strings: *const ZtStrings, index: usize) -> *mut c_char {
    // SAFETY: the caller passes the live view.
    let Some(view) = (unsafe { strings.as_ref() }) else { return std::ptr::null_mut() };
    // SAFETY: the view is live during the callback.
    unsafe { view.slice() }.get(index).map_or(std::ptr::null_mut(), |s| owned(s))
}

/// Strings from a C array of `count` UTF-8 strings; `None` when one is NULL or not UTF-8.
///
/// # Safety
/// `strings` is NULL with `count` 0, or points to `count` valid C strings.
pub(crate) unsafe fn string_array(strings: *const *const c_char, count: usize) -> Option<Vec<String>> {
    if count == 0 {
        return Some(vec![]);
    }
    if strings.is_null() {
        return None;
    }
    // SAFETY: the caller passes `count` pointers.
    let pointers = unsafe { std::slice::from_raw_parts(strings, count) };
    // SAFETY: each pointer is a valid C string per the contract.
    pointers.iter().map(|&p| unsafe { borrowed(p) }.map(str::to_owned)).collect()
}
