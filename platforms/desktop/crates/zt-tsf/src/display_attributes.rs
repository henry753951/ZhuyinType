//! How applications draw the composition (TSF display attributes), the way macOS marks it: every word underlined,
//! the word the candidate list replaces highlighted with a thick underline, and the word ↓ would change (no list
//! open) with a thick underline.  Text converted through an automatic correction has a dotted underline in the
//! selection colour instead (marks.rs decides which part gets which).
//!
//! The text service is the attributes' provider (`GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER`, registration.rs): TSF asks it
//! for each attribute by GUID.  `mark` writes them into the composition as `GUID_PROP_ATTRIBUTE` values, the atoms
//! the category manager gives those GUIDs.  Neighbouring words alternate between two attributes that look the same,
//! so applications that leave a gap between attribute runs (browsers, Word) show where each word ends.

use std::cell::Cell;

use windows::Win32::Foundation::{COLORREF, E_INVALIDARG, S_FALSE};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Variant::{VARIANT, VT_I4};
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, GUID_PROP_ATTRIBUTE, IEnumTfDisplayAttributeInfo, IEnumTfDisplayAttributeInfo_Impl,
    ITfCategoryMgr, ITfContext, ITfDisplayAttributeInfo, ITfDisplayAttributeInfo_Impl, ITfRange, TF_ANCHOR_START,
    TF_ATTR_INPUT, TF_ATTR_TARGET_CONVERTED, TF_ATTR_TARGET_NOTCONVERTED, TF_CT_NONE, TF_CT_SYSCOLOR, TF_DA_ATTR_INFO,
    TF_DA_COLOR, TF_DA_COLOR_0, TF_DA_LINESTYLE, TF_DISPLAYATTRIBUTE, TF_LS_DOT, TF_LS_SOLID,
};
use windows::core::{BSTR, GUID, Result, implement};

use crate::marks;
use crate::module::ObjectCount;
use crate::protocol::Reply;

/// `GetSysColor` indices: the selection's background and text.
const COLOR_HIGHLIGHT: i32 = 13;
const COLOR_HIGHLIGHTTEXT: i32 = 14;

/// One display attribute this text service provides.
#[derive(Clone, Copy)]
pub(crate) struct Style {
    guid: GUID,
    description: &'static str,
    /// A system colour index for the text and the background, or none (the application's own).
    text: Option<i32>,
    background: Option<i32>,
    bold: bool,
    line: TF_DA_LINESTYLE,
    /// A system colour index for the underline, or none (the text's).
    line_color: Option<i32>,
    kind: TF_DA_ATTR_INFO,
}

/// A word of the composition.
const WORD: Style = Style {
    guid: GUID::from_u128(0xa2a5adbf_8521_4224_9762_a0ee462a3064),
    description: "ZhuyinType 組字",
    text: None,
    background: None,
    bold: false,
    line: TF_LS_SOLID,
    line_color: None,
    kind: TF_ATTR_INPUT,
};
/// The next word: the same look, so the two alternate and every word keeps its own underline.
const WORD_NEXT: Style = Style { guid: GUID::from_u128(0xd1600990_7199_4935_8fdc_6e6f7271ddd8), ..WORD };
/// The word the candidate list replaces: highlighted like a selection, with a thick underline.
const FOCUS: Style = Style {
    guid: GUID::from_u128(0x0263f980_5055_4c06_a948_05ba42bca135),
    description: "ZhuyinType 正在選字",
    text: Some(COLOR_HIGHLIGHTTEXT),
    background: Some(COLOR_HIGHLIGHT),
    bold: true,
    line: TF_LS_SOLID,
    line_color: None,
    kind: TF_ATTR_TARGET_CONVERTED,
};
/// The word ↓ would change, with the cursor inside the composition and no list open: a thick underline.
const TARGET: Style = Style {
    guid: GUID::from_u128(0x0a8b9f79_2a12_4092_a7e0_6577df33e2e4),
    description: "ZhuyinType 游標所在的詞",
    text: None,
    background: None,
    bold: true,
    line: TF_LS_SOLID,
    line_color: None,
    kind: TF_ATTR_TARGET_NOTCONVERTED,
};

/// A part of a word converted through an automatic correction or a fuzzy sound, not as typed: a thin dotted
/// underline in the selection colour (the system's accent), still an input word.
const CORRECTED: Style = Style {
    guid: GUID::from_u128(0xd691ecc4_1cf0_4751_bfb1_91205bf2b0f1),
    description: "ZhuyinType 自動修正的字",
    line: TF_LS_DOT,
    line_color: Some(COLOR_HIGHLIGHT),
    ..WORD
};
/// The same look, alternating like `WORD_NEXT`.
const CORRECTED_NEXT: Style = Style { guid: GUID::from_u128(0x5ae61e1d_dab2_439b_b72a_0cd70b1902f7), ..CORRECTED };

/// In the order of `marks::Mark`.
pub(crate) const STYLES: [Style; 6] = [WORD, WORD_NEXT, FOCUS, TARGET, CORRECTED, CORRECTED_NEXT];

/// The category manager's atom for each of `STYLES`.
pub(crate) type Atoms = [u32; STYLES.len()];

fn color(index: Option<i32>) -> TF_DA_COLOR {
    match index {
        Some(index) => TF_DA_COLOR { r#type: TF_CT_SYSCOLOR, Anonymous: TF_DA_COLOR_0 { nIndex: index } },
        None => TF_DA_COLOR { r#type: TF_CT_NONE, Anonymous: TF_DA_COLOR_0 { cr: COLORREF(0) } },
    }
}

impl Style {
    fn attribute(&self) -> TF_DISPLAYATTRIBUTE {
        TF_DISPLAYATTRIBUTE {
            crText: color(self.text),
            crBk: color(self.background),
            lsStyle: self.line,
            fBoldLine: self.bold.into(),
            crLine: color(self.line_color),
            bAttr: self.kind,
        }
    }
}

#[implement(ITfDisplayAttributeInfo)]
struct Info {
    style: Style,
    _count: ObjectCount,
}

impl ITfDisplayAttributeInfo_Impl for Info_Impl {
    fn GetGUID(&self) -> Result<GUID> {
        Ok(self.style.guid)
    }

    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from(self.style.description))
    }

    fn GetAttributeInfo(&self, attribute: *mut TF_DISPLAYATTRIBUTE) -> Result<()> {
        if attribute.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: checked for null; the caller passes a writable structure.
        unsafe { *attribute = self.style.attribute() };
        Ok(())
    }

    fn SetAttributeInfo(&self, _attribute: *const TF_DISPLAYATTRIBUTE) -> Result<()> {
        // The look is fixed.
        Err(windows::Win32::Foundation::E_NOTIMPL.into())
    }

    fn Reset(&self) -> Result<()> {
        Ok(())
    }
}

/// The attribute for `guid`, if it is one of ours.
pub(crate) fn info(guid: &GUID) -> Result<ITfDisplayAttributeInfo> {
    let style = STYLES.iter().find(|style| style.guid == *guid).ok_or(windows::core::Error::from(E_INVALIDARG))?;
    Ok(Info { style: *style, _count: ObjectCount::new() }.into())
}

#[implement(IEnumTfDisplayAttributeInfo)]
pub(crate) struct Enumeration {
    next: Cell<usize>,
    _count: ObjectCount,
}

impl Enumeration {
    pub(crate) fn create(next: usize) -> IEnumTfDisplayAttributeInfo {
        Self { next: Cell::new(next), _count: ObjectCount::new() }.into()
    }
}

impl IEnumTfDisplayAttributeInfo_Impl for Enumeration_Impl {
    fn Clone(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(Enumeration::create(self.next.get()))
    }

    fn Next(&self, count: u32, infos: *mut Option<ITfDisplayAttributeInfo>, fetched: *mut u32) -> Result<()> {
        if infos.is_null() || (count != 1 && fetched.is_null()) {
            return Err(E_INVALIDARG.into());
        }
        let mut done = 0;
        while done < count as usize && self.next.get() < STYLES.len() {
            let style = STYLES[self.next.get()];
            // SAFETY: the caller's array holds `count` entries.
            unsafe { infos.add(done).write(Some(Info { style, _count: ObjectCount::new() }.into())) };
            self.next.set(self.next.get() + 1);
            done += 1;
        }
        if !fetched.is_null() {
            // SAFETY: checked for null.
            unsafe { *fetched = done as u32 };
        }
        // S_FALSE: fewer than asked for.
        if done < count as usize { Err(S_FALSE.into()) } else { Ok(()) }
    }

    fn Reset(&self) -> Result<()> {
        self.next.set(0);
        Ok(())
    }

    fn Skip(&self, count: u32) -> Result<()> {
        let next = self.next.get() + count as usize;
        self.next.set(next.min(STYLES.len()));
        if next > STYLES.len() { Err(S_FALSE.into()) } else { Ok(()) }
    }
}

/// The atoms of `STYLES`, from the category manager (the same in every process of the session); `None` when it
/// cannot give them, and the composition is then left unmarked.
pub(crate) fn atoms() -> Option<Atoms> {
    // SAFETY: COM is initialised on a thread that activates a text service.
    let manager: ITfCategoryMgr =
        unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER) }.ok()?;
    let mut atoms = [0; STYLES.len()];
    for (atom, style) in atoms.iter_mut().zip(STYLES.iter()) {
        // SAFETY: a valid GUID.
        *atom = unsafe { manager.RegisterGUID(&style.guid) }.ok()?;
    }
    Some(atoms)
}

fn atom_value(atom: u32) -> VARIANT {
    let mut value = VARIANT::default();
    // SAFETY: writes the tag and the matching member of a fresh VARIANT; an I4 owns nothing to release.
    unsafe {
        (*value.Anonymous.Anonymous).vt = VT_I4;
        (*value.Anonymous.Anonymous).Anonymous.lVal = atom as i32;
    }
    value
}

/// The part of `range` (the composition) from `start` to `end`, in UTF-16 units.
fn part(cookie: u32, range: &ITfRange, start: usize, end: usize) -> Result<ITfRange> {
    // SAFETY: valid ranges under the edit cookie.
    unsafe {
        let part = range.Clone()?;
        part.Collapse(cookie, TF_ANCHOR_START)?;
        let mut moved = 0;
        part.ShiftEnd(cookie, end as i32, &mut moved, std::ptr::null())?;
        part.ShiftStart(cookie, start as i32, &mut moved, std::ptr::null())?;
        Ok(part)
    }
}

/// Marks the composition `range` holding `reply.preedit`: each word with its attribute, the whole of it as one word
/// when the host sends no words, and corrected parts apart (marks.rs).
pub(crate) fn mark(context: &ITfContext, cookie: u32, range: &ITfRange, atoms: &Atoms, reply: &Reply) -> Result<()> {
    // SAFETY: valid objects under the edit cookie.
    unsafe {
        let property = context.GetProperty(&GUID_PROP_ATTRIBUTE)?;
        property.Clear(cookie, range)?;
        for (start, end, mark) in marks::runs(reply) {
            property.SetValue(cookie, &part(cookie, range, start, end)?, &atom_value(atoms[mark as usize]))?;
        }
    }
    Ok(())
}

/// Removes the marks from `range` (a composition that is ending: its text stays as plain text).
pub(crate) fn clear(context: &ITfContext, cookie: u32, range: &ITfRange) {
    // SAFETY: valid objects under the edit cookie.
    unsafe {
        if let Ok(property) = context.GetProperty(&GUID_PROP_ATTRIBUTE) {
            let _ = property.Clear(cookie, range);
        }
    }
}
