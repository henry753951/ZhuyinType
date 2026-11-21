//! A window that takes text the way games do, for testing the text service with them on a test machine: no TSF
//! document of its own (Windows gives it a transitory one through IMM32), WM_CHAR for text, and a choice of what it
//! does with the IME's messages.
//!
//!   game_window.exe [--mode glfw|eat|noime] [--fullscreen] [--log FILE]
//!
//! - `glfw` (default): as GLFW (Minecraft: Java Edition) does, WM_IME_* go to DefWindowProc.
//! - `eat`: WM_IME_STARTCOMPOSITION, WM_IME_COMPOSITION and WM_IME_ENDCOMPOSITION are answered without
//!   DefWindowProc, as many game engines do, so Windows draws no composition window.
//! - `noime`: the window has no input context (ImmAssociateContextEx with none), as games that turn the IME off.
//! - `--fullscreen`: a borderless topmost window covering its monitor.
//!
//! The window draws what it received; the log records the IME messages, the characters and each change of keyboard
//! layout.  Test machines only: it logs what is typed.

#[cfg(not(windows))]
fn main() {
    eprintln!("game_window runs on Windows");
}

#[cfg(windows)]
fn main() {
    windows_main::run();
}

#[cfg(windows)]
mod windows_main {
    use std::cell::RefCell;
    use std::fs::OpenOptions;
    use std::io::Write as _;

    use windows::Win32::Foundation::POINT;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, COLOR_WINDOW, DT_LEFT, DT_WORDBREAK, DrawTextW, EndPaint, GetMonitorInfoW, GetSysColorBrush,
        InvalidateRect, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint, PAINTSTRUCT,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::Ime::{IACE_CHILDREN, ImmAssociateContextEx};
    use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
    use windows::Win32::UI::WindowsAndMessaging::{
        CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, IDC_ARROW, LoadCursorW, MSG,
        PostQuitMessage, RegisterClassW, SW_SHOW, SetTimer, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WM_CHAR,
        WM_DESTROY, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION, WM_IME_NOTIFY, WM_IME_SETCONTEXT,
        WM_IME_STARTCOMPOSITION, WM_INPUTLANGCHANGE, WM_PAINT, WM_TIMER, WNDCLASSW, WS_EX_TOPMOST, WS_OVERLAPPEDWINDOW,
        WS_POPUP, WS_VISIBLE,
    };
    use windows::core::w;

    #[derive(Clone, Copy, PartialEq)]
    enum Mode {
        Glfw,
        Eat,
        NoIme,
    }

    struct State {
        mode: Mode,
        log: Option<std::fs::File>,
        text: String,
        layout: isize,
    }

    thread_local! {
        static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    }

    fn log(line: &str) {
        STATE.with_borrow_mut(|state| {
            if let Some(file) = state.as_mut().and_then(|state| state.log.as_mut()) {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                let _ = writeln!(file, "{} {line}", now.as_millis());
            }
        });
    }

    fn mode() -> Mode {
        STATE.with_borrow(|state| state.as_ref().map_or(Mode::Glfw, |state| state.mode))
    }

    extern "system" fn proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match message {
            WM_IME_STARTCOMPOSITION
            | WM_IME_COMPOSITION
            | WM_IME_ENDCOMPOSITION
            | WM_IME_NOTIFY
            | WM_IME_SETCONTEXT => {
                log(&format!("ime 0x{message:04x} w=0x{:x} l=0x{:x}", wparam.0, lparam.0));
                if mode() == Mode::Eat
                    && matches!(message, WM_IME_STARTCOMPOSITION | WM_IME_COMPOSITION | WM_IME_ENDCOMPOSITION)
                {
                    return LRESULT(0);
                }
            }
            WM_CHAR => {
                let unit = wparam.0 as u16;
                log(&format!("char U+{unit:04X}"));
                STATE.with_borrow_mut(|state| {
                    if let Some(state) = state.as_mut() {
                        match unit {
                            8 => {
                                state.text.pop();
                            }
                            13 => state.text.push('\n'),
                            unit => state.text.push(char::from_u32(u32::from(unit)).unwrap_or('\u{fffd}')),
                        }
                    }
                });
                // SAFETY: our own window.
                unsafe {
                    let _ = InvalidateRect(Some(window), None, true);
                }
                return LRESULT(0);
            }
            WM_INPUTLANGCHANGE => log(&format!("inputlangchange hkl=0x{:x}", lparam.0)),
            WM_TIMER => {
                // SAFETY: plain query of this thread's layout.
                let layout = unsafe { GetKeyboardLayout(0) }.0 as isize;
                let changed = STATE.with_borrow_mut(|state| {
                    state.as_mut().is_some_and(|state| std::mem::replace(&mut state.layout, layout) != layout)
                });
                if changed {
                    log(&format!("layout 0x{layout:x}"));
                }
                return LRESULT(0);
            }
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                let text: Vec<u16> = STATE.with_borrow(|state| {
                    let state = state.as_ref();
                    let label = match state.map_or(Mode::Glfw, |state| state.mode) {
                        Mode::Glfw => "glfw",
                        Mode::Eat => "eat",
                        Mode::NoIme => "noime",
                    };
                    format!("game_window ({label}) > {}", state.map(|state| state.text.as_str()).unwrap_or_default())
                        .encode_utf16()
                        .collect()
                });
                // SAFETY: our own window, painted inside BeginPaint/EndPaint.
                unsafe {
                    let dc = BeginPaint(window, &mut paint);
                    let mut rect = RECT { left: 20, top: 20, right: 1200, bottom: 800 };
                    let mut text = text;
                    DrawTextW(dc, &mut text, &mut rect, DT_LEFT | DT_WORDBREAK);
                    let _ = EndPaint(window, &paint);
                }
                return LRESULT(0);
            }
            WM_DESTROY => {
                // SAFETY: ends this thread's loop.
                unsafe { PostQuitMessage(0) };
                return LRESULT(0);
            }
            _ => {}
        }
        // SAFETY: the default for everything else.
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    }

    pub(super) fn run() {
        let args: Vec<String> = std::env::args().collect();
        let value = |name: &str| args.iter().position(|arg| arg == name).and_then(|i| args.get(i + 1)).cloned();
        let mode = match value("--mode").as_deref() {
            Some("eat") => Mode::Eat,
            Some("noime") => Mode::NoIme,
            _ => Mode::Glfw,
        };
        let fullscreen = args.iter().any(|arg| arg == "--fullscreen");
        let log_file = value("--log").and_then(|path| OpenOptions::new().create(true).append(true).open(path).ok());
        STATE.with_borrow_mut(|state| *state = Some(State { mode, log: log_file, text: String::new(), layout: 0 }));
        // SAFETY: plain Win32 window creation and message loop on this thread.
        unsafe {
            let instance = GetModuleHandleW(None).unwrap_or_default();
            let class = w!("ZtGameWindow");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                lpszClassName: class,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                hbrBackground: GetSysColorBrush(COLOR_WINDOW),
                ..Default::default()
            };
            RegisterClassW(&wc);
            let (ex, style, x, y, width, height) = if fullscreen {
                let monitor = MonitorFromPoint(POINT::default(), MONITOR_DEFAULTTOPRIMARY);
                let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
                let _ = GetMonitorInfoW(monitor, &mut info);
                let r = info.rcMonitor;
                (WS_EX_TOPMOST, WS_POPUP | WS_VISIBLE, r.left, r.top, r.right - r.left, r.bottom - r.top)
            } else {
                (WINDOW_EX_STYLE::default(), WS_OVERLAPPEDWINDOW | WS_VISIBLE, CW_USEDEFAULT, CW_USEDEFAULT, 900, 500)
            };
            let window = CreateWindowExW(
                ex,
                class,
                w!("game_window"),
                style,
                x,
                y,
                width,
                height,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .expect("CreateWindowExW");
            if mode == Mode::NoIme {
                let _ = ImmAssociateContextEx(window, Default::default(), IACE_CHILDREN);
            }
            let _ = ShowWindow(window, SW_SHOW);
            SetTimer(Some(window), 1, 200, None);
            log(&format!("start mode={} fullscreen={fullscreen}", args.join(" ")));
            let mut message = MSG::default();
            while GetMessageW(&mut message, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}
