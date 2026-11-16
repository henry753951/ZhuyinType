//! The candidate window and the 中/英 badge, drawn by the text service itself in the application's own process, as
//! Windows' own input methods do.
//!
//! - **Z-order.**  Each window is created owned by the top-level window of the field being typed in, with an input
//!   method's window class (`CS_IME`).  An owned window always stays above its owner and lives in the owner's
//!   z-order band, so the list shows over the Start menu's search box and immersive applications, which no window of
//!   another process can.
//! - **Look.**  The system draws the edge as for its own menus: Windows 11 rounds the corners, draws the hairline
//!   border and the shadow, and puts its acrylic material (`DWMSBT_TRANSIENTWINDOW`) behind the content, in the
//!   window's light or dark appearance.  Where there is none of that (Windows 10, transparency effects off) the
//!   window is the panel colour with its own hairline.  The content is Direct2D (`panel_draw`), with the app's
//!   geometry and colours (`zt_panel`).
//! - **Focus.**  The windows never activate (`WS_EX_NOACTIVATE`, `MA_NOACTIVATE`); clicks post to the text service's
//!   message window, which sends them to the host inside an edit session.
//! - **DPI.**  Created and moved per-monitor aware: sizes are the logical layout times the caret's monitor scale.

use std::cell::{Cell, RefCell};
use std::time::Instant;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_SIZE_U, D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_FEATURE_LEVEL_DEFAULT, D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_TYPE_SOFTWARE,
    D2D1_RENDER_TARGET_USAGE_NONE, ID2D1DCRenderTarget, ID2D1HwndRenderTarget, ID2D1RenderTarget,
};
use windows::Win32::Graphics::Dwm::{
    DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetMonitorInfoW, HDC, InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect,
    PAINTSTRUCT,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::Controls::{MARGINS, WM_MOUSELEAVE};
use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CS_IME, CreateWindowExW, DefWindowProcW, DestroyWindow, GA_ROOT, GWLP_USERDATA, GetAncestor,
    GetWindowLongPtrW, HTCLIENT, HWND_TOP, IDC_ARROW, IsWindowVisible, KillTimer, LoadCursorW, MA_NOACTIVATE,
    PostMessageW, RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SetCursor, SetTimer, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, UnregisterClassW, WM_APP, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_MOUSEACTIVATE, WM_MOUSEMOVE,
    WM_NCCALCSIZE, WM_NCCREATE, WM_NCHITTEST, WM_PAINT, WM_SETCURSOR, WM_TIMER, WNDCLASSW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_POPUP, WS_THICKFRAME,
};
use windows::core::{BOOL, PCWSTR, w};
use zt_panel::layout::{self as panel_layout, BADGE_H, BADGE_W, Frame, PanelLayout, RowLayout};
use zt_panel::position::{Rect, popup_position_with_gap};
use zt_panel::style::{Appearance, Panel};
use zt_panel::tokens::{self, motion};

use crate::composition::per_monitor;
use crate::module::instance;
use crate::panel_draw::{self, Control, DirectWriteMeasure, Hit, Motion, Painter, graphics};
use crate::protocol::PanelUpdate;

/// Posted to the text service's message window for a click: `wparam` the action (`ACTION_*`), `lparam` its value.
pub(crate) const WM_PANEL_ACTION: u32 = WM_APP + 0x31;
pub(crate) const ACTION_PICK: usize = 1;
pub(crate) const ACTION_PAGE: usize = 2;
pub(crate) const ACTION_SINGLE: usize = 3;
pub(crate) const ACTION_CORRECTIONS: usize = 4;
pub(crate) const ACTION_ZHUYIN: usize = 5;

/// The broker's name for an action.
pub(crate) fn action_name(action: usize) -> Option<&'static str> {
    Some(match action {
        ACTION_PICK => "pick",
        ACTION_PAGE => "page",
        ACTION_SINGLE => "single",
        ACTION_CORRECTIONS => "corrections",
        ACTION_ZHUYIN => "zhuyin",
        _ => return None,
    })
}

const ANIMATION_TIMER: usize = 1;
const TOAST_TIMER: usize = 2;
const FRAME_MS: u32 = 8;

/// Which of the two windows a window procedure call is for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Panel,
    Badge,
}

/// The work area of the monitor `rect` is on, physical pixels.
fn work_area(rect: RECT) -> RECT {
    per_monitor(|| {
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        // SAFETY: a valid rectangle and a sized MONITORINFO.
        let _ = unsafe { GetMonitorInfoW(MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST), &mut info) };
        info.rcWork
    })
}

fn to_rect(r: RECT) -> Rect {
    Rect::new(f64::from(r.left), f64::from(r.top), f64::from(r.right - r.left), f64::from(r.bottom - r.top))
}

/// The apps' light or dark choice (Settings › Personalization › Colors).
fn system_dark() -> bool {
    let mut value: u32 = 1;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: a DWORD read into a DWORD.
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&raw mut size),
        )
    };
    result.is_ok() && value == 0
}

/// A window's render target: on the window itself, or (where the application cannot make one) on its paint DC.
#[derive(Clone)]
enum Target {
    Window(ID2D1HwndRenderTarget),
    Dc(ID2D1DCRenderTarget),
}

/// One of the two windows and its render target.
struct Surface {
    window: Cell<HWND>,
    target: RefCell<Option<Target>>,
    /// The system draws the corners, border and shadow (Windows 11).
    system_edge: Cell<bool>,
    /// The system's acrylic is behind the content.
    backdrop: Cell<bool>,
    /// The render target has no alpha: the content cannot let the backdrop through.
    opaque: Cell<bool>,
    dark: Cell<Option<bool>>,
    /// Physical size and the scale it was laid out at.
    size: Cell<(i32, i32)>,
    scale: Cell<f32>,
}

impl Surface {
    fn new() -> Self {
        Self {
            window: Cell::new(HWND::default()),
            target: RefCell::new(None),
            system_edge: Cell::new(false),
            backdrop: Cell::new(false),
            opaque: Cell::new(false),
            dark: Cell::new(None),
            size: Cell::new((0, 0)),
            scale: Cell::new(1.),
        }
    }

    fn destroy(&self) {
        self.target.borrow_mut().take();
        self.dark.set(None);
        let window = self.window.replace(HWND::default());
        if !window.is_invalid() {
            // SAFETY: our own window; the back pointer is cleared first.
            unsafe {
                SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                let _ = DestroyWindow(window);
            }
        }
    }

    fn hide(&self) {
        let window = self.window.get();
        if !window.is_invalid() {
            // SAFETY: our own window.
            let _ = unsafe { ShowWindow(window, SW_HIDE) };
        }
    }

    fn visible(&self) -> bool {
        let window = self.window.get();
        // SAFETY: any window handle.
        !window.is_invalid() && unsafe { IsWindowVisible(window) }.as_bool()
    }

    /// Light or dark: the system's edge and acrylic follow.
    fn set_dark(&self, dark: bool) {
        let window = self.window.get();
        if window.is_invalid() || self.dark.get() == Some(dark) {
            return;
        }
        self.dark.set(Some(dark));
        let value = BOOL::from(dark);
        // SAFETY: our own window and a 4-byte BOOL.
        let _ = unsafe {
            DwmSetWindowAttribute(
                window,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                (&raw const value).cast(),
                size_of::<BOOL>() as u32,
            )
        };
    }

    /// Places the window (physical pixels) on top of its owner and shows it, without activating it.
    fn place(&self, x: i32, y: i32, width: i32, height: i32, scale: f32) {
        let window = self.window.get();
        if window.is_invalid() {
            return;
        }
        self.size.set((width, height));
        self.scale.set(scale);
        let visible = self.visible();
        per_monitor(|| {
            // SAFETY: our own window.
            unsafe {
                let _ = SetWindowPos(window, Some(HWND_TOP), x, y, width, height, SWP_NOACTIVATE);
                let _ = InvalidateRect(Some(window), None, false);
                if !visible {
                    let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
                }
            }
        });
    }

    /// Draws with `paint` in logical pixels, inside WM_PAINT (`dc` is the paint DC).
    ///
    /// A hardware render target on the window first.  Some sandboxed applications (the Start menu's search) may not
    /// create one at all (DXGI_ERROR_NOT_CURRENTLY_AVAILABLE, software included); there Direct2D draws in software
    /// into the paint DC instead, with alpha for the backdrop if it can, else opaque (the window then draws its own
    /// surface).
    fn render(&self, dc: HDC, paint: impl FnOnce(&ID2D1RenderTarget)) {
        let window = self.window.get();
        let (width, height) = self.size.get();
        if window.is_invalid() || width <= 0 || height <= 0 {
            return;
        }
        let Some(g) = graphics() else { return };
        let pixels = D2D_SIZE_U { width: width as u32, height: height as u32 };
        let properties = |kind, alpha| D2D1_RENDER_TARGET_PROPERTIES {
            r#type: kind,
            pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: alpha },
            dpiX: 0.,
            dpiY: 0.,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let existing = self.target.borrow().clone();
        let target = match existing {
            Some(target) => target,
            None => {
                let hwnd_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                    hwnd: window,
                    pixelSize: pixels,
                    presentOptions: D2D1_PRESENT_OPTIONS_NONE,
                };
                // SAFETY: a valid factory and window.
                let made = unsafe {
                    g.d2d
                        .CreateHwndRenderTarget(
                            &properties(D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_ALPHA_MODE_PREMULTIPLIED),
                            &hwnd_properties,
                        )
                        .map(|target| (Target::Window(target), false))
                        .or_else(|_| {
                            g.d2d
                                .CreateDCRenderTarget(&properties(
                                    D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                                    D2D1_ALPHA_MODE_PREMULTIPLIED,
                                ))
                                .map(|target| (Target::Dc(target), false))
                        })
                        .or_else(|_| {
                            g.d2d
                                .CreateDCRenderTarget(&properties(
                                    D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                                    D2D1_ALPHA_MODE_IGNORE,
                                ))
                                .map(|target| (Target::Dc(target), true))
                        })
                };
                let Ok((target, opaque)) = made else { return };
                self.opaque.set(opaque);
                *self.target.borrow_mut() = Some(target.clone());
                target
            }
        };
        let scale = self.scale.get();
        // SAFETY: a valid target, drawn between BeginDraw and EndDraw.
        let result = unsafe {
            let base: ID2D1RenderTarget = match &target {
                Target::Window(target) => {
                    let _ = target.Resize(&pixels);
                    target.clone().into()
                }
                Target::Dc(target) => {
                    let _ = target.BindDC(dc, &RECT { left: 0, top: 0, right: width, bottom: height });
                    target.clone().into()
                }
            };
            base.SetDpi(96. * scale, 96. * scale);
            base.BeginDraw();
            base.Clear(Some(&D2D1_COLOR_F { r: 0., g: 0., b: 0., a: 0. }));
            paint(&base);
            base.EndDraw(None, None)
        };
        if result.is_err() {
            // The device went away (D2DERR_RECREATE_TARGET): a new target next time.
            self.target.borrow_mut().take();
            // SAFETY: our own window.
            let _ = unsafe { InvalidateRect(Some(window), None, false) };
        }
    }

    /// The surface under the content: the system's backdrop with a tint, or the panel colour; the hairline where
    /// the system draws no edge.
    fn base(&self, painter: &Painter<'_>, size: (f32, f32)) {
        let whole = Frame::new(0., 0., size.0, size.1);
        let c = painter.c;
        let translucent = self.backdrop.get() && !self.opaque.get();
        painter.fill(whole, 0., if translucent { c.backdrop_tint } else { c.surface });
        if !self.system_edge.get() {
            painter.outline(whole, 0., c.border);
        }
    }
}

/// What the candidate window shows and how it moves.
#[derive(Default)]
struct State {
    panel: Option<Panel>,
    layout: Option<PanelLayout>,
    /// The size of the list shown, so turning pages never resizes the window.
    steady: (f32, f32),
    /// What makes two lists the same list, and the page shown.
    shown: Option<(Identity, usize)>,
    outgoing: Option<Vec<RowLayout>>,
    old_label: Option<String>,
    turn: Option<(i32, Instant)>,
    bounce: Option<(i32, Instant)>,
    hover: Option<Hit>,
    /// The caret of the last placement and the window's x then (it keeps its x while the caret stays on the line).
    last_caret: Option<RECT>,
    last_x: i32,
    toast: Option<String>,
    /// The last list's appearance and placement, which the 中/英 badge follows after the list closed.
    appearance: Appearance,
    above: bool,
}

impl State {
    fn forget(&mut self) {
        let (toast, appearance, above) = (self.toast.take(), self.appearance, self.above);
        *self = Self { toast, appearance, above, ..Self::default() };
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Identity {
    count: usize,
    page_size: usize,
    tolerant: Option<bool>,
    single: bool,
    reading: Option<String>,
    focused: Option<usize>,
}

impl Identity {
    fn of(panel: &Panel) -> Option<Self> {
        let m = &panel.model;
        m.has_list().then(|| Self {
            count: m.count,
            page_size: m.page_size,
            tolerant: m.tolerant,
            single: m.single,
            reading: m.reading.clone(),
            focused: m.focused_clause,
        })
    }
}

/// The page turn and the bounce at one moment.
struct Frames {
    running: bool,
    incoming: (f32, f32, f32),
    outgoing: Option<(f32, f32, f32)>,
    highlight: (f32, f32),
    label_fade: f32,
}

/// The text service's candidate window and 中/英 badge.  Lives inside the text service, which never moves, so the
/// windows can point back at it.
pub(crate) struct PanelWindow {
    class: RefCell<Vec<u16>>,
    owner: Cell<HWND>,
    /// Where clicks are posted (the text service's message window).
    notify: Cell<HWND>,
    panel: Surface,
    badge: Surface,
    state: RefCell<State>,
}

impl PanelWindow {
    pub(crate) fn new() -> Self {
        Self {
            class: RefCell::new(Vec::new()),
            owner: Cell::new(HWND::default()),
            notify: Cell::new(HWND::default()),
            panel: Surface::new(),
            badge: Surface::new(),
            state: RefCell::new(State::default()),
        }
    }

    pub(crate) fn set_notify(&self, window: HWND) {
        self.notify.set(window);
    }

    /// The window that shows the text being typed: the windows are owned by its top-level window.  A different
    /// owner makes new windows.
    pub(crate) fn set_owner(&self, view: HWND) {
        // SAFETY: any window handle; an invalid one gives a null root.
        let root = if view.is_invalid() { view } else { unsafe { GetAncestor(view, GA_ROOT) } };
        if root != self.owner.replace(root) {
            self.panel.destroy();
            self.badge.destroy();
            self.state.borrow_mut().forget();
        }
    }

    /// Hides both windows and forgets the list (focus left the field).
    pub(crate) fn hide(&self) {
        self.panel.hide();
        self.badge.hide();
        let mut state = self.state.borrow_mut();
        state.forget();
        state.toast = None;
    }

    /// Removes the windows and their class (the text service is deactivating).
    pub(crate) fn destroy(&self) {
        self.panel.destroy();
        self.badge.destroy();
        let class = std::mem::take(&mut *self.class.borrow_mut());
        if !class.is_empty() {
            // SAFETY: the class registered by this instance.
            let _ = unsafe { UnregisterClassW(PCWSTR(class.as_ptr()), Some(instance())) };
        }
    }

    fn class(&self) -> Option<Vec<u16>> {
        let mut class = self.class.borrow_mut();
        if class.is_empty() {
            let name = format!("ZhuyinType.Panel.{:p}", std::ptr::from_ref(self));
            let name: Vec<u16> = name.encode_utf16().chain([0]).collect();
            // CS_IME: an input method's window, as Windows' own input methods register theirs.
            let klass = WNDCLASSW {
                style: CS_IME,
                hInstance: instance(),
                lpfnWndProc: Some(window_proc),
                lpszClassName: PCWSTR(name.as_ptr()),
                ..Default::default()
            };
            // SAFETY: the class name outlives the class (kept until unregistered).
            if unsafe { RegisterClassW(&klass) } == 0 {
                return None;
            }
            *class = name;
        }
        Some(class.clone())
    }

    fn surface(&self, kind: Kind) -> &Surface {
        match kind {
            Kind::Panel => &self.panel,
            Kind::Badge => &self.badge,
        }
    }

    /// The window for `kind`, made on first use: owned by the application's window, framed by the system.
    fn ensure(&self, kind: Kind) -> Option<HWND> {
        let surface = self.surface(kind);
        let window = surface.window.get();
        if !window.is_invalid() {
            return Some(window);
        }
        let class = self.class()?;
        let owner = self.owner.get();
        let this: *const PanelWindow = self;
        let params = Box::into_raw(Box::new((this, kind)));
        let created = per_monitor(|| {
            // SAFETY: a registered class; the creation parameters are taken back in WM_NCCREATE.
            unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    PCWSTR(class.as_ptr()),
                    w!(""),
                    // A frame (taken away again in WM_NCCALCSIZE) is what gives a popup the system's shadow.
                    WS_POPUP | WS_THICKFRAME,
                    0,
                    0,
                    1,
                    1,
                    (!owner.is_invalid()).then_some(owner),
                    None,
                    Some(instance()),
                    Some(params.cast()),
                )
            }
        });
        let Ok(window) = created else { return None };
        surface.window.set(window);
        // SAFETY: our own window; 4-byte attribute values as DWM expects (each fails harmlessly where unsupported).
        unsafe {
            let corner = DWMWCP_ROUND.0 as u32;
            let edge = DwmSetWindowAttribute(
                window,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&raw const corner).cast(),
                size_of::<u32>() as u32,
            );
            surface.system_edge.set(edge.is_ok());
            let backdrop = DWMSBT_TRANSIENTWINDOW.0 as u32;
            let material = DwmSetWindowAttribute(
                window,
                DWMWA_SYSTEMBACKDROP_TYPE,
                (&raw const backdrop).cast(),
                size_of::<u32>() as u32,
            );
            surface.backdrop.set(material.is_ok());
            // The whole client area is "frame": the backdrop shows where the content is transparent.
            let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
            let _ = DwmExtendFrameIntoClientArea(window, &raw const margins);
        }
        Some(window)
    }

    /// Applies a reply: the window's new state (shown, changed or hidden), a page turn that stopped at the list's
    /// end, the 中/英 badge.  `caret` is in physical screen pixels, `scale` its monitor's.
    pub(crate) fn apply(
        &self,
        update: &PanelUpdate,
        boundary: Option<i32>,
        toast: Option<&str>,
        caret: RECT,
        scale: f64,
    ) {
        let scale = (scale as f32).clamp(0.5, 8.);
        match update {
            PanelUpdate::Unchanged => {}
            PanelUpdate::Hidden => {
                self.panel.hide();
                self.state.borrow_mut().forget();
            }
            PanelUpdate::Show(panel) => self.show(panel, caret, scale),
        }
        if let Some(direction) = boundary {
            self.bounce(direction);
        }
        if let Some(text) = toast {
            self.show_toast(text, caret, scale);
        }
    }

    fn show(&self, panel: &Panel, caret: RECT, scale: f32) {
        let Some(g) = graphics() else { return };
        if self.ensure(Kind::Panel).is_none() {
            return;
        }
        self.panel.set_dark(panel.appearance.dark(system_dark()));
        let visible = self.panel.visible();
        let mut state = self.state.borrow_mut();
        // A new page of the same list turns; anything else starts over at its own size.
        let identity = Identity::of(panel);
        let model = &panel.model;
        let mut direction = 0;
        if let (Some(identity), Some((shown, page))) = (&identity, &state.shown)
            && identity == shown
            && *page != model.page
        {
            direction = if *page + 1 == model.page_count && model.page == 0 || model.page > *page { 1 } else { -1 };
        }
        if identity.is_none()
            || identity.as_ref() != state.shown.as_ref().map(|(identity, _)| identity)
            || state.panel.as_ref().map(|p| &p.style) != Some(&panel.style)
        {
            state.steady = (0., 0.);
        }
        state.shown = identity.map(|identity| (identity, model.page));
        let screen = work_area(caret);
        let max_width = ((screen.right - screen.left) as f32 / scale - 16.).max(100.);
        let layout = panel_layout::compute(model, &panel.style, max_width, state.steady, &mut DirectWriteMeasure(&g));
        if layout.list_open {
            state.steady = (layout.width, layout.height);
        }
        let previous = state.layout.take();
        if direction != 0 && panel.animates && visible {
            state.outgoing = previous.as_ref().map(|previous| previous.rows.clone());
            state.old_label = previous.and_then(|previous| previous.pager.map(|pager| pager.label));
            state.turn = Some((direction, Instant::now()));
            state.bounce = None;
        } else if direction != 0 || !layout.list_open {
            state.turn = None;
            state.bounce = None;
            state.outgoing = None;
            state.old_label = None;
        }
        let width = (layout.width * scale).ceil() as i32;
        let height = (layout.height * scale).ceil() as i32;
        let (x, y) = popup_position_with_gap(
            to_rect(caret),
            f64::from(width),
            f64::from(height),
            panel.above,
            f64::from(layout.text_inset * scale),
            to_rect(screen),
            f64::from(4. * scale),
        );
        let mut x = x as i32;
        // While the caret stays on the same line the window keeps its x, so it does not wander as the text grows.
        let tolerance = (2. * scale).ceil() as i32;
        if visible
            && let Some(last) = state.last_caret
            && (last.top - caret.top).abs() <= tolerance
            && (last.bottom - caret.bottom).abs() <= tolerance
        {
            x = state.last_x.clamp(screen.left, screen.left.max(screen.right - width));
        }
        state.last_caret = Some(caret);
        state.last_x = x;
        let animating = state.turn.is_some();
        state.layout = Some(layout);
        state.panel = Some(panel.clone());
        state.appearance = panel.appearance;
        state.above = panel.above;
        drop(state);
        self.panel.place(x, y as i32, width, height, scale);
        if animating {
            // SAFETY: our own window.
            unsafe { SetTimer(Some(self.panel.window.get()), ANIMATION_TIMER, FRAME_MS, None) };
        }
    }

    /// Paging stopped at the first or last page: the page nudges that way and settles back.
    fn bounce(&self, direction: i32) {
        let window = self.panel.window.get();
        let mut state = self.state.borrow_mut();
        if window.is_invalid()
            || !state.panel.as_ref().is_some_and(|panel| panel.animates)
            || !state.layout.as_ref().is_some_and(|layout| layout.list_open)
        {
            return;
        }
        state.turn = None;
        state.outgoing = None;
        state.bounce = Some((direction.signum(), Instant::now()));
        drop(state);
        // SAFETY: our own window.
        unsafe { SetTimer(Some(window), ANIMATION_TIMER, FRAME_MS, None) };
    }

    fn show_toast(&self, text: &str, caret: RECT, scale: f32) {
        let Some(window) = self.ensure(Kind::Badge) else { return };
        let (appearance, above) = {
            let state = self.state.borrow();
            (state.appearance, state.above)
        };
        self.badge.set_dark(appearance.dark(system_dark()));
        self.state.borrow_mut().toast = Some(text.to_owned());
        let side = (BADGE_W * scale).ceil() as i32;
        let screen = work_area(caret);
        let side_f = f64::from(side);
        let (x, y) =
            popup_position_with_gap(to_rect(caret), side_f, side_f, above, 0., to_rect(screen), f64::from(4. * scale));
        self.badge.place(x as i32, y as i32, side, (BADGE_H * scale).ceil() as i32, scale);
        // SAFETY: our own window.
        unsafe { SetTimer(Some(window), TOAST_TIMER, (motion::TOAST_HOLD + 200.) as u32, None) };
    }

    /// The page turn and the bounce at this moment.
    fn frames(state: &State, layout: &PanelLayout) -> Frames {
        let along = |amount: f32| if layout.vertical { (0., amount) } else { (amount, 0.) };
        let mut frames =
            Frames { running: false, incoming: (0., 0., 1.), outgoing: None, highlight: (0., 0.), label_fade: 1. };
        if let Some((direction, started)) = state.turn {
            let elapsed = started.elapsed().as_secs_f32() * 1000.;
            if elapsed < motion::DURATION {
                frames.running = true;
                let travel = motion::OFFSET * direction as f32;
                let t = elapsed / motion::DURATION;
                let (x, y) = along(travel * (1. - motion::ease_out(t)));
                let fade = (elapsed - motion::IN_DELAY) / (motion::DURATION - motion::IN_DELAY);
                frames.incoming = (x, y, motion::ease_out(fade));
                if elapsed < motion::OUT_DURATION {
                    let out = motion::ease_out(elapsed / motion::OUT_DURATION);
                    let (x, y) = along(-travel * out);
                    frames.outgoing = Some((x, y, 1. - out));
                }
                frames.label_fade = motion::ease_out(t);
            }
        }
        if let Some((direction, started)) = state.bounce {
            let elapsed = started.elapsed().as_secs_f32() * 1000.;
            if elapsed < motion::BOUNCE_OUT + motion::BOUNCE_BACK {
                frames.running = true;
                let amount = if elapsed < motion::BOUNCE_OUT {
                    motion::ease_out(elapsed / motion::BOUNCE_OUT)
                } else {
                    1. - motion::ease_in_out((elapsed - motion::BOUNCE_OUT) / motion::BOUNCE_BACK)
                };
                let (x, y) = along(motion::BOUNCE * direction as f32 * amount);
                frames.incoming = (x, y, 1.);
                frames.highlight = (x, y);
            }
        }
        frames
    }

    fn paint_panel(&self, dc: HDC) {
        let Some(g) = graphics() else { return };
        let state = self.state.borrow();
        let (Some(layout), Some(panel)) = (state.layout.clone(), state.panel.clone()) else { return };
        let frames = Self::frames(&state, &layout);
        let (hover, hover_control) = match state.hover {
            Some(Hit::Row(index)) => (Some(index), None),
            Some(Hit::Control(control)) => (None, Some(control)),
            None => (None, None),
        };
        let outgoing_rows = state.outgoing.clone();
        let old_label = state.old_label.clone();
        drop(state);
        let colors = tokens::colors(panel.appearance.dark(system_dark()));
        let surface = &self.panel;
        surface.render(dc, |target| {
            let Some(painter) = Painter::new(&g, target, colors) else { return };
            surface.base(&painter, (layout.width, layout.height));
            let motion = Motion {
                incoming: frames.incoming,
                outgoing: outgoing_rows.as_deref().zip(frames.outgoing).map(|(rows, (x, y, a))| (rows, x, y, a)),
                highlight: frames.highlight,
                old_label: old_label.as_deref(),
                label_fade: frames.label_fade,
                hover,
                hover_control,
            };
            panel_draw::content(&painter, &layout, &motion);
        });
        if !frames.running {
            let mut state = self.state.borrow_mut();
            if state.turn.is_some() || state.bounce.is_some() {
                state.turn = None;
                state.bounce = None;
                state.outgoing = None;
                state.old_label = None;
                // SAFETY: our own window.
                let _ = unsafe { KillTimer(Some(self.panel.window.get()), ANIMATION_TIMER) };
            }
        }
    }

    fn paint_badge(&self, dc: HDC) {
        let Some(g) = graphics() else { return };
        let state = self.state.borrow();
        let Some(text) = state.toast.clone() else { return };
        let appearance = state.appearance;
        drop(state);
        let colors = tokens::colors(appearance.dark(system_dark()));
        let surface = &self.badge;
        surface.render(dc, |target| {
            let Some(painter) = Painter::new(&g, target, colors) else { return };
            surface.base(&painter, (BADGE_W, BADGE_H));
            panel_draw::badge(&painter, &text, BADGE_W);
        });
    }

    fn mouse(&self, x: i32, y: i32, click: bool) {
        let scale = self.panel.scale.get().max(0.25);
        let (lx, ly) = (x as f32 / scale, y as f32 / scale);
        let hit = self.state.borrow().layout.as_ref().and_then(|layout| panel_draw::hit(layout, lx, ly));
        if self.state.borrow().hover != hit {
            self.state.borrow_mut().hover = hit;
            // SAFETY: our own window.
            let _ = unsafe { InvalidateRect(Some(self.panel.window.get()), None, false) };
        }
        if !click {
            return;
        }
        let (action, value) = match hit {
            Some(Hit::Row(index)) => (ACTION_PICK, index as isize),
            Some(Hit::Control(Control::Previous)) => (ACTION_PAGE, -1),
            Some(Hit::Control(Control::Next)) => (ACTION_PAGE, 1),
            Some(Hit::Control(Control::Single)) => (ACTION_SINGLE, 0),
            Some(Hit::Control(Control::Corrections)) => (ACTION_CORRECTIONS, 0),
            Some(Hit::Control(Control::Zhuyin)) => (ACTION_ZHUYIN, 0),
            None => return,
        };
        let notify = self.notify.get();
        if !notify.is_invalid() {
            // SAFETY: the text service's own message window.
            let _ = unsafe { PostMessageW(Some(notify), WM_PANEL_ACTION, WPARAM(action), LPARAM(value)) };
        }
    }

    fn message(&self, kind: Kind, window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match message {
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                // SAFETY: our own window.
                let dc = unsafe { BeginPaint(window, &mut paint) };
                match kind {
                    Kind::Panel => self.paint_panel(dc),
                    Kind::Badge => self.paint_badge(dc),
                }
                // SAFETY: paired with BeginPaint.
                let _ = unsafe { EndPaint(window, &paint) };
                Some(LRESULT(0))
            }
            WM_TIMER if wparam.0 == ANIMATION_TIMER => {
                // SAFETY: our own window.
                let _ = unsafe { InvalidateRect(Some(window), None, false) };
                Some(LRESULT(0))
            }
            WM_TIMER if wparam.0 == TOAST_TIMER => {
                // SAFETY: our own window.
                let _ = unsafe { KillTimer(Some(window), TOAST_TIMER) };
                self.badge.hide();
                self.state.borrow_mut().toast = None;
                Some(LRESULT(0))
            }
            WM_MOUSEMOVE | WM_LBUTTONDOWN if kind == Kind::Panel => {
                let x = i32::from(lparam.0 as u32 as u16 as i16);
                let y = i32::from((lparam.0 as u32 >> 16) as u16 as i16);
                if message == WM_MOUSEMOVE {
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: window,
                        dwHoverTime: 0,
                    };
                    // SAFETY: a sized structure for our own window.
                    let _ = unsafe { TrackMouseEvent(&mut track) };
                }
                self.mouse(x, y, message == WM_LBUTTONDOWN);
                Some(LRESULT(0))
            }
            WM_MOUSELEAVE => {
                if self.state.borrow().hover.is_some() {
                    self.state.borrow_mut().hover = None;
                    // SAFETY: our own window.
                    let _ = unsafe { InvalidateRect(Some(window), None, false) };
                }
                Some(LRESULT(0))
            }
            _ => None,
        }
    }
}

impl Drop for PanelWindow {
    fn drop(&mut self) {
        self.destroy();
    }
}

unsafe extern "system" fn window_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: Windows calls this with valid arguments; the user data is cleared before the panel goes away.
    unsafe {
        match message {
            WM_NCCREATE => {
                let create = &*(lparam.0 as *const CREATESTRUCTW);
                let params = Box::from_raw(create.lpCreateParams.cast::<(*const PanelWindow, Kind)>());
                let (this, kind) = *params;
                // The panel pointer is aligned, so its low bit says which window this is.
                let tagged = (this as usize) | usize::from(kind == Kind::Badge);
                SetWindowLongPtrW(window, GWLP_USERDATA, tagged as _);
                return DefWindowProcW(window, message, wparam, lparam);
            }
            // The frame exists only for the system's shadow: the whole window is client area.
            WM_NCCALCSIZE if wparam.0 != 0 => return LRESULT(0),
            WM_NCHITTEST => return LRESULT(HTCLIENT as isize),
            WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
            WM_ERASEBKGND => return LRESULT(1),
            WM_SETCURSOR => {
                if let Ok(cursor) = LoadCursorW(None, IDC_ARROW) {
                    SetCursor(Some(cursor));
                }
                return LRESULT(1);
            }
            _ => {}
        }
        let tagged = GetWindowLongPtrW(window, GWLP_USERDATA) as usize;
        let this = (tagged & !1) as *const PanelWindow;
        if !this.is_null() {
            let kind = if tagged & 1 == 1 { Kind::Badge } else { Kind::Panel };
            let handled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (*this).message(kind, window, message, wparam, lparam)
            }));
            if let Ok(Some(result)) = handled {
                return result;
            }
        }
        DefWindowProcW(window, message, wparam, lparam)
    }
}
