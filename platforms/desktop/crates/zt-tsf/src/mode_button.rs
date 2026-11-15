//! The 中/英 indicator in the taskbar's input indicator (`GUID_LBI_INPUTMODE`), like the built-in Bopomofo IME: a
//! monochrome 中 or 英 in the taskbar's text colour.  A left click asks the host to switch modes; a right click opens
//! the input method's menu (switch, settings).  The button keeps no state but what the host last reported.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use windows::Win32::Foundation::{COLORREF, E_INVALIDARG, E_NOTIMPL, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLIP_DEFAULT_PRECIS, CreateBitmap, CreateCompatibleDC,
    CreateDIBSection, CreateFontW, DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, DT_CENTER, DT_NOPREFIX,
    DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, FW_NORMAL, GetDC, HGDIOBJ, OUT_DEFAULT_PRECIS,
    ReleaseDC, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::Ole::{CONNECT_E_ADVISELIMIT, CONNECT_E_CANNOTCONNECT, CONNECT_E_NOCONNECTION};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi};
use windows::Win32::UI::TextServices::{
    GUID_LBI_INPUTMODE, ITfLangBarItem_Impl, ITfLangBarItemButton, ITfLangBarItemButton_Impl, ITfLangBarItemSink,
    ITfMenu, ITfSource, ITfSource_Impl, TF_LANGBARITEMINFO, TF_LBI_CLK_RIGHT, TF_LBI_ICON, TF_LBI_STATUS,
    TF_LBI_STATUS_DISABLED, TF_LBI_STYLE_BTN_BUTTON, TF_LBI_STYLE_SHOWNINTRAY, TF_LBI_TEXT, TF_LBI_TOOLTIP, TfLBIClick,
};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO, SM_CXSMICON};
use windows::core::{BOOL, BSTR, GUID, IUnknown, Interface, Ref, Result, implement, w};

use crate::module::{ObjectCount, SERVICE_ID, guard};

/// What a click does: `(right_click, screen point)`.
pub(crate) type Action = Rc<dyn Fn(bool, POINT)>;

const SINK_COOKIE: u32 = 1;

#[implement(ITfLangBarItemButton, ITfSource)]
pub(crate) struct ModeButton {
    action: RefCell<Option<Action>>,
    sink: RefCell<Option<ITfLangBarItemSink>>,
    english: Cell<bool>,
    enabled: Cell<bool>,
    _count: ObjectCount,
}

impl ModeButton {
    pub(crate) fn new(action: Action) -> Self {
        Self {
            action: RefCell::new(Some(action)),
            sink: RefCell::new(None),
            english: Cell::new(false),
            enabled: Cell::new(true),
            _count: ObjectCount::new(),
        }
    }

    /// Shows the host's mode; repaints the taskbar only when it changed.
    pub(crate) fn set_english(&self, english: bool) {
        if self.english.replace(english) != english {
            self.notify(TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_TOOLTIP);
        }
    }

    /// Greys the button out while the field cannot take Chinese input (secure, or no host).
    pub(crate) fn set_enabled(&self, enabled: bool) {
        if self.enabled.replace(enabled) != enabled {
            self.notify(TF_LBI_STATUS);
        }
    }

    /// Drops the click action before the text service that owns it goes away; TSF may still hold the button.
    pub(crate) fn clear_action(&self) {
        let old = self.action.try_borrow_mut().ok().and_then(|mut action| action.take());
        drop(old);
    }

    fn notify(&self, flags: u32) {
        let sink = self.sink.try_borrow().ok().and_then(|sink| sink.clone());
        if let Some(sink) = sink {
            // SAFETY: a sink TSF advised.
            let _ = unsafe { sink.OnUpdate(flags) };
        }
    }

    fn glyph(&self) -> &'static str {
        if self.english.get() { "英" } else { "中" }
    }
}

impl ITfLangBarItem_Impl for ModeButton_Impl {
    fn GetInfo(&self, info: *mut TF_LANGBARITEMINFO) -> Result<()> {
        guard(|| {
            if info.is_null() {
                return Err(E_INVALIDARG.into());
            }
            let mut description = [0u16; 32];
            for (slot, unit) in description.iter_mut().zip("中英切換".encode_utf16()) {
                *slot = unit;
            }
            // SAFETY: checked for null; TSF passes a writable structure.
            unsafe {
                *info = TF_LANGBARITEMINFO {
                    clsidService: SERVICE_ID,
                    guidItem: GUID_LBI_INPUTMODE,
                    dwStyle: TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY,
                    ulSort: 0,
                    szDescription: description,
                };
            }
            Ok(())
        })
    }

    fn GetStatus(&self) -> Result<u32> {
        Ok(if self.enabled.get() { 0 } else { TF_LBI_STATUS_DISABLED })
    }

    fn Show(&self, _show: BOOL) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn GetTooltipString(&self) -> Result<BSTR> {
        Ok(BSTR::from(if self.english.get() {
            "英文（按一下切換成中文）"
        } else {
            "中文（按一下切換成英文）"
        }))
    }
}

impl ITfLangBarItemButton_Impl for ModeButton_Impl {
    fn OnClick(&self, click: TfLBIClick, point: &POINT, _area: *const RECT) -> Result<()> {
        guard(|| {
            let action = self.action.try_borrow().ok().and_then(|action| action.clone());
            if let Some(action) = action {
                action(click == TF_LBI_CLK_RIGHT, *point);
            }
            Ok(())
        })
    }

    fn InitMenu(&self, _menu: Ref<ITfMenu>) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn OnMenuSelect(&self, _id: u32) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn GetIcon(&self) -> Result<HICON> {
        // One plain glyph, like Windows' own IMEs; the input method's square logo sits to its right.
        guard(|| glyph_icon(self.glyph()))
    }

    fn GetText(&self) -> Result<BSTR> {
        Ok(BSTR::from(self.glyph()))
    }
}

impl ITfSource_Impl for ModeButton_Impl {
    fn AdviseSink(&self, iid: *const GUID, sink: Ref<IUnknown>) -> Result<u32> {
        guard(|| {
            // SAFETY: TSF passes a valid GUID pointer or null.
            if iid.is_null() || unsafe { *iid } != ITfLangBarItemSink::IID {
                return Err(CONNECT_E_CANNOTCONNECT.into());
            }
            let sink = sink.ok()?.cast::<ITfLangBarItemSink>()?;
            let mut slot = self.sink.try_borrow_mut().map_err(|_| windows::core::Error::from(CONNECT_E_ADVISELIMIT))?;
            if slot.is_some() {
                return Err(CONNECT_E_ADVISELIMIT.into());
            }
            *slot = Some(sink);
            Ok(SINK_COOKIE)
        })
    }

    fn UnadviseSink(&self, cookie: u32) -> Result<()> {
        guard(|| {
            let old = match self.sink.try_borrow_mut() {
                Ok(mut slot) if cookie == SINK_COOKIE && slot.is_some() => slot.take(),
                _ => return Err(CONNECT_E_NOCONNECTION.into()),
            };
            drop(old);
            Ok(())
        })
    }
}

/// Creates the COM object and returns the button interface TSF takes, plus the object itself.
pub(crate) fn create(action: Action) -> (ITfLangBarItemButton, windows::core::ComObject<ModeButton>) {
    let object = windows::core::ComObject::new(ModeButton::new(action));
    (object.to_interface(), object)
}

/// True when the taskbar is light (dark glyphs), from the user's personalisation settings.
fn taskbar_is_light() -> bool {
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: a DWORD-sized buffer.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&mut size),
        )
    };
    status.is_ok() && value != 0
}

/// Draws one character as a small icon in the taskbar's text colour: coverage first (white on black, no ClearType,
/// so every channel carries it), then that colour premultiplied by the coverage.
fn glyph_icon(glyph: &str) -> Result<HICON> {
    // SAFETY: GDI objects are created, used and deleted here; the DIB's bits stay valid while the bitmap lives.
    unsafe {
        let dpi = GetDpiForSystem();
        let size = GetSystemMetricsForDpi(SM_CXSMICON, dpi).max(16);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -size,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let mut bits = std::ptr::null_mut();
        let color = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0);
        ReleaseDC(None, screen);
        let color = match color {
            Ok(color) if !dc.is_invalid() && !bits.is_null() => color,
            other => {
                if let Ok(color) = other {
                    let _ = DeleteObject(color.into());
                }
                if !dc.is_invalid() {
                    let _ = DeleteDC(dc);
                }
                return Err(windows::core::Error::from(windows::Win32::Foundation::E_FAIL));
            }
        };
        let count = usize::try_from(size * size).unwrap_or(0);
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u32>(), count);
        pixels.fill(0);
        let old_bitmap = SelectObject(dc, color.into());
        let font = CreateFontW(
            -(size * 15 / 16),
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            ANTIALIASED_QUALITY,
            u32::from(DEFAULT_PITCH.0),
            w!("Microsoft JhengHei UI"),
        );
        let old_font = SelectObject(dc, font.into());
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, COLORREF(0x00ff_ffff));
        let mut area = RECT { left: 0, top: 0, right: size, bottom: size };
        let mut text: Vec<u16> = glyph.encode_utf16().collect();
        DrawTextW(dc, &mut text, &mut area, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        SelectObject(dc, old_font);
        let _ = DeleteObject(HGDIOBJ(font.0));
        SelectObject(dc, old_bitmap);
        let _ = DeleteDC(dc);
        let ink: u32 = if taskbar_is_light() { 0x1b } else { 0xff };
        for pixel in pixels.iter_mut() {
            let coverage = *pixel & 0xff;
            let value = ink * coverage / 255;
            *pixel = (coverage << 24) | (value << 16) | (value << 8) | value;
        }
        let stride = usize::try_from((size + 15) / 16 * 2).unwrap_or(0);
        let mask_bits = vec![0u8; stride * usize::try_from(size).unwrap_or(0)];
        let mask = CreateBitmap(size, size, 1, 1, Some(mask_bits.as_ptr().cast()));
        let icon = ICONINFO { fIcon: true.into(), xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
        let result =
            if mask.is_invalid() { Err(windows::Win32::Foundation::E_FAIL.into()) } else { CreateIconIndirect(&icon) };
        if !mask.is_invalid() {
            let _ = DeleteObject(mask.into());
        }
        let _ = DeleteObject(color.into());
        result
    }
}
