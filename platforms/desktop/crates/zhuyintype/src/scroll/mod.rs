//! Scroll views that move the way Chrome and Edge move on the same computer: a wheel notch glides over 100–200 ms
//! (Chromium's curve, `curve.rs`), a touchpad or high-resolution wheel follows the fingers 1:1 (`input.rs`), and
//! Page Up/Down, Home/End and scroll-to glide the same way.  When the system asks for less motion (Windows 「動畫效果」,
//! the desktop's reduced-motion or GNOME's 「動畫」, `ZT_REDUCE_MOTION`) everything arrives at once, as in Chromium.
//!
//! GPUI scrolls a view by a wheel event's lines × the text's line height, at once.  [`SmoothScroll`] takes the
//! vertical events of one scroll view before GPUI's own handler does and moves the view itself:
//!
//! ```ignore
//! div().overflow_y_scroll().track_scroll(scroll.handle()).child(content).child(scroll.wheel())
//! ```
//!
//! [`SmoothScroll::wheel`] is an element added as the scroll view's first child (or as a later sibling of a
//! `uniform_list`/`list`, which take no children).  GPUI calls scroll listeners in the reverse of the order they were
//! painted, so it sees each event after the views nested in the content and before the view's own listener, which it
//! stops.  Horizontal events go on to GPUI.
//!
//! Nested views follow Chromium's wheel latching: the innermost view that can move in the event's direction takes a
//! scroll sequence (events less than 500 ms apart with the pointer within 10 px, `kDefaultMouseWheelLatchingTransaction`
//! and `kWheelLatchingSlopRegion`) and keeps it, at its edge too, so a list scrolled to its end does not start moving
//! the page until the wheel rests, and a page that started moving keeps moving when a list comes under the pointer.
//!
//! Testing: `ZT_SCROLL_LOG=<file>` appends every event and every animation frame (`event`, `frame` lines with
//! milliseconds); `ZT_SMOOTH_SCROLL=0` leaves the events to GPUI (the behaviour before this module) and only logs.

mod curve;
mod input;

use std::cell::{Cell, RefCell};
use std::io::Write as _;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use gpui_kit::base::ScrollbarHandle;
use gpui_kit::{
    App, Bounds, DispatchPhase, Element, ElementId, EntityId, GlobalElementId, Hitbox, HitboxBehavior,
    InspectorElementId, IntoElement, KeyDownEvent, LayoutId, Pixels, Point, Position, ScrollEvent, Style, TouchPhase,
    Window, point, px,
};

use curve::Curve;
pub use curve::Kind;
use input::{Step, WheelUnits, page_step};

/// Chromium's `kDefaultMouseWheelLatchingTransaction`.
const LATCH_TIMEOUT: Duration = Duration::from_millis(500);
/// Chromium's `kWheelLatchingSlopRegion` (pixels).
const LATCH_SLOP: f32 = 10.;
/// A movement smaller than this is no movement (half a pixel).
const STILL: f64 = 0.5;
/// How far another writer may leave the view from where an animation put it before the animation gives way (a
/// scrollbar drag, a jump to a row, content that shrank).  `ListState` reads back through item heights, so not 0.
const OTHER_WRITER: f64 = 1.0;

/// The scroll view that owns the current scroll sequence.
#[derive(Clone)]
struct Latch {
    scroller: u64,
    owner: Rc<dyn Owner>,
    at: Instant,
    origin: Point<Pixels>,
}

/// A scroll view a latched sequence goes to, whichever view is under the pointer.
trait Owner {
    fn apply(&self, step: Step, window: &mut Window, cx: &mut App);
}

impl Latch {
    fn holds(&self, event: &ScrollEvent, now: Instant) -> bool {
        let moved = event.position - self.origin;
        event.touch_phase != TouchPhase::Started
            && now.duration_since(self.at) < LATCH_TIMEOUT
            && f32::from(moved.x).hypot(f32::from(moved.y)) <= LATCH_SLOP
    }
}

thread_local! {
    static LATCH: RefCell<Option<Latch>> = const { RefCell::new(None) };
    static NEXT_ID: Cell<u64> = const { Cell::new(1) };
}

/// A scroll view's handle and its animation.  Clones share both.
pub struct SmoothScroll<H> {
    handle: H,
    state: Rc<RefCell<State>>,
}

impl<H: Clone> Clone for SmoothScroll<H> {
    fn clone(&self) -> Self {
        Self { handle: self.handle.clone(), state: self.state.clone() }
    }
}

#[derive(Debug)]
struct State {
    id: u64,
    running: Option<Running>,
    /// The scroll position this module last wrote, snapped to device pixels, and the exact one it was asked for:
    /// another position means someone else moved the view; the exact one keeps a touchpad's fractions.
    written: Option<(f64, f64)>,
    /// A frame callback is pending.
    ticking: bool,
    /// The view that draws the scroll view (notified each frame), from its last paint.
    view: Option<EntityId>,
    /// The scroll view's line height at its last paint: what GPUI's own handler scrolls a line (logged only).
    line_height: f32,
    /// `ZT_SMOOTH_SCROLL=0` with `ZT_SCROLL_LOG`: log positions until then.
    observe_until: Option<Instant>,
}

#[derive(Clone, Debug)]
struct Running {
    curve: Curve,
    started: Instant,
}

impl Running {
    fn t(&self, now: Instant) -> f64 {
        now.saturating_duration_since(self.started).as_secs_f64()
    }
}

impl<H: ScrollbarHandle + Clone> SmoothScroll<H> {
    pub fn new(handle: H) -> Self {
        let id = NEXT_ID.with(|next| {
            let id = next.get();
            next.set(id + 1);
            id
        });
        let state = State {
            id,
            running: None,
            written: None,
            ticking: false,
            view: None,
            line_height: 0.,
            observe_until: None,
        };
        Self { handle, state: Rc::new(RefCell::new(state)) }
    }

    /// The GPUI handle, for `track_scroll` and scrollbars.
    pub fn handle(&self) -> &H {
        &self.handle
    }

    /// The element that takes the view's wheel events; add it after the scrolled content.
    pub fn wheel(&self) -> WheelTarget<H> {
        WheelTarget { scroll: self.clone() }
    }

    /// Where the view will rest: the running animation's target, or where it is.
    pub fn destination(&self) -> Pixels {
        let state = self.state.borrow();
        px(state.running.as_ref().map_or_else(|| self.top(), |running| running.curve.target()) as f32)
    }

    /// Scrolls so the content's `top` is at the top of the view (clamped), gliding like
    /// `element.scrollTo({ behavior: "smooth" })` unless the system asks for less motion.  For links to a row and for
    /// keyboard focus; a running wheel animation turns towards it without stopping.
    pub fn scroll_to(&self, top: Pixels, window: &mut Window, cx: &mut App) {
        self.glide_to(Kind::Programmatic, f64::from(f32::from(top)), window, cx);
    }

    /// Scrolls to `top` at once, ending any animation.  May be called while the window draws (a row scrolling itself
    /// into view from its prepaint): the view also redraws on the next frame.
    pub fn jump_to(&self, top: Pixels, window: &mut Window, cx: &mut App) {
        self.state.borrow_mut().running = None;
        let top = f64::from(f32::from(top)).clamp(0., self.max());
        self.write(top, window);
        self.notify(window, cx);
        let this = self.clone();
        window.on_next_frame(move |window, cx| this.notify(window, cx));
    }

    /// Ends the animation where the view is (another page is shown).
    pub fn stop(&self) {
        self.state.borrow_mut().running = None;
    }

    /// Page Up/Down, Home/End without modifiers: true when the view moves (the key is used).
    pub fn key_down(&self, event: &KeyDownEvent, window: &mut Window, cx: &mut App) -> bool {
        if event.keystroke.modifiers.modified() || !enabled() {
            return false;
        }
        let destination = f64::from(f32::from(self.destination()));
        let page = page_step(f64::from(f32::from(self.handle.viewport_bounds().size.height)));
        let target = match event.keystroke.key.as_str() {
            "pageup" => destination - page,
            "pagedown" => destination + page,
            "home" => 0.,
            "end" => self.max(),
            _ => return false,
        };
        let target = target.clamp(0., self.max());
        if (target - destination).abs() < STILL {
            return false;
        }
        log(format_args!("event {} key {} target {target:.1}", self.id(), event.keystroke.key));
        self.glide_to(Kind::Keyboard, target, window, cx);
        true
    }

    fn id(&self) -> u64 {
        self.state.borrow().id
    }

    /// The scroll position: pixels of content above the view.
    fn top(&self) -> f64 {
        -f64::from(f32::from(self.handle.offset().y))
    }

    fn max(&self) -> f64 {
        let content = self.handle.content_size().height;
        let viewport = self.handle.viewport_bounds().size.height;
        f64::from(f32::from(content - viewport)).max(0.)
    }

    /// Ends the animation if someone else moved the view since this module last did.
    fn yield_to_other_writers(&self) {
        let mut state = self.state.borrow_mut();
        if let Some((written, _)) = state.written
            && (written - self.top()).abs() > OTHER_WRITER
        {
            state.running = None;
            state.written = None;
        }
    }

    fn write(&self, exact: f64, window: &Window) {
        let top = f64::from(f32::from(window.pixel_snap(px(exact as f32))));
        let offset = self.handle.offset();
        self.handle.set_offset(point(offset.x, px(-top as f32)));
        // Read back: a `ListState` stores the position as an item and an offset into it.
        let written = self.top();
        self.state.borrow_mut().written = Some((written, exact));
        log(format_args!("frame {} {written:.1}", self.id()));
    }

    /// Where the view is, with the fraction of a pixel the last write snapped away.
    fn exact_top(&self) -> f64 {
        let top = self.top();
        match self.state.borrow().written {
            Some((written, exact)) if (written - top).abs() < 0.01 => exact,
            _ => top,
        }
    }

    fn notify(&self, window: &mut Window, cx: &mut App) {
        match self.state.borrow().view {
            Some(view) => cx.notify(view),
            None => window.refresh(),
        }
    }

    /// Animates to `target` (clamped): a new curve of `kind`, or the running one turned towards it.
    fn glide_to(&self, kind: Kind, target: f64, window: &mut Window, cx: &mut App) {
        let target = target.clamp(0., self.max());
        if crate::motion::reduced(cx) {
            self.jump_to(px(target as f32), window, cx);
            return;
        }
        self.yield_to_other_writers();
        let now = Instant::now();
        {
            let mut state = self.state.borrow_mut();
            match state.running.as_mut() {
                Some(running) => {
                    let t = running.t(now);
                    running.curve.update_target(t, target);
                }
                None => {
                    let from = self.top();
                    if (target - from).abs() < STILL {
                        return;
                    }
                    state.running = Some(Running { curve: Curve::new(kind, from, target), started: now });
                }
            }
        }
        self.schedule(window);
    }

    fn schedule(&self, window: &mut Window) {
        if std::mem::replace(&mut self.state.borrow_mut().ticking, true) {
            return;
        }
        let this = self.clone();
        window.on_next_frame(move |window, cx| this.frame(window, cx));
    }

    /// One animation frame: moves the view to where the curve is now.
    fn frame(&self, window: &mut Window, cx: &mut App) {
        self.state.borrow_mut().ticking = false;
        self.yield_to_other_writers();
        let now = Instant::now();
        let running = self.state.borrow().running.clone();
        let Some(running) = running else {
            self.observe(now, window);
            return;
        };
        let t = running.t(now);
        let top = running.curve.value(t).clamp(0., self.max());
        self.write(top, window);
        if running.curve.finished(t) {
            self.state.borrow_mut().running = None;
        } else {
            self.schedule(window);
        }
        self.notify(window, cx);
    }

    /// `ZT_SMOOTH_SCROLL=0`: logs where GPUI's own handler put the view.
    fn observe(&self, now: Instant, window: &mut Window) {
        let until = self.state.borrow().observe_until;
        if until.is_some_and(|until| now < until) {
            log(format_args!("frame {} {:.1}", self.id(), self.top()));
            self.schedule(window);
        }
    }

    /// One scroll event over the view; true when this view used it (GPUI's handlers must not see it).
    fn scroll_event(&self, event: &ScrollEvent, window: &mut Window, cx: &mut App) -> bool {
        let viewport = self.handle.viewport_bounds();
        let Some(step) = WheelUnits::system().step(event.delta, f64::from(f32::from(viewport.size.height))) else {
            return false;
        };
        let now = Instant::now();
        log(format_args!(
            "event {} {:?} {:?} gpui {:.1}",
            self.id(),
            event.delta,
            step,
            f32::from(event.delta.pixel_delta(px(self.state.borrow().line_height)).y)
        ));
        if !enabled() {
            if log_enabled() {
                self.state.borrow_mut().observe_until = Some(now + Duration::from_millis(600));
                self.schedule(window);
            }
            return false;
        }
        let id = self.id();
        let latch = LATCH.with_borrow(|latch| latch.clone().filter(|latch| latch.holds(event, now)));
        match latch {
            // Another view's sequence goes on in that view.
            Some(latch) if latch.scroller != id => {
                latch.owner.apply(step, window, cx);
                LATCH.set(Some(Latch { at: now, ..latch }));
            }
            Some(latch) => {
                self.apply(step, window, cx);
                LATCH.set(Some(Latch { at: now, ..latch }));
            }
            None if self.can_move(step) => {
                let owner: Rc<dyn Owner> = Rc::new(self.clone());
                LATCH.set(Some(Latch { scroller: id, owner, at: now, origin: event.position }));
                self.apply(step, window, cx);
            }
            // Gliding to this edge already: nothing to add, and GPUI's handler would jump there.
            None if self.state.borrow().running.is_some() => {}
            // An outer view may move.
            None => return false,
        }
        true
    }

    fn can_move(&self, step: Step) -> bool {
        self.yield_to_other_writers();
        let (from, delta) = match step {
            Step::Notches(delta) => (f64::from(f32::from(self.destination())), delta),
            Step::Precise(delta) => (self.top(), delta),
        };
        ((from + delta).clamp(0., self.max()) - from).abs() >= STILL
    }
}

impl<H: ScrollbarHandle + Clone> Owner for SmoothScroll<H> {
    fn apply(&self, step: Step, window: &mut Window, cx: &mut App) {
        match step {
            Step::Notches(delta) => {
                let destination = f64::from(f32::from(self.destination()));
                self.glide_to(Kind::Wheel, destination + delta, window, cx);
            }
            // A touchpad takes over from a running animation where the content is (Chromium aborts it).
            Step::Precise(delta) => {
                self.state.borrow_mut().running = None;
                let top = (self.exact_top() + delta).clamp(0., self.max());
                self.write(top, window);
                self.notify(window, cx);
            }
        }
    }
}

/// Whether this module moves the views (`ZT_SMOOTH_SCROLL=0`: GPUI does, as before).
fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("ZT_SMOOTH_SCROLL").as_deref() != Ok("0"))
}

fn log_file() -> Option<&'static Mutex<std::fs::File>> {
    static FILE: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();
    FILE.get_or_init(|| {
        let path = std::env::var_os("ZT_SCROLL_LOG")?;
        std::fs::OpenOptions::new().create(true).append(true).open(path).ok().map(Mutex::new)
    })
    .as_ref()
}

fn log_enabled() -> bool {
    log_file().is_some()
}

/// One `ZT_SCROLL_LOG` line: milliseconds since the first line, then `line`.
fn log(line: std::fmt::Arguments<'_>) {
    static START: OnceLock<Instant> = OnceLock::new();
    let Some(file) = log_file() else { return };
    let ms = START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.;
    if let Ok(mut file) = file.lock() {
        // A failed test log line changes nothing for the user.
        let _ = writeln!(file, "{ms:.2} {line}");
    }
}

/// The element [`SmoothScroll::wheel`] returns: no size, a hitbox over the scroll view, and the scroll listener.
pub struct WheelTarget<H> {
    scroll: SmoothScroll<H>,
}

impl<H: ScrollbarHandle + Clone> IntoElement for WheelTarget<H> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<H: ScrollbarHandle + Clone> Element for WheelTarget<H> {
    type RequestLayoutState = ();
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let style = Style { position: Position::Absolute, ..Style::default() };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) -> Option<Hitbox> {
        // The view's bounds are current here: a scroll view lays out before its children, a list before later
        // siblings.
        let viewport = self.scroll.handle.viewport_bounds();
        let visible = viewport.size.width > px(0.) && viewport.size.height > px(0.);
        visible.then(|| window.insert_hitbox(viewport, HitboxBehavior::Normal))
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        hitbox: &mut Option<Hitbox>,
        window: &mut Window,
        _cx: &mut App,
    ) {
        {
            let mut state = self.scroll.state.borrow_mut();
            state.view = Some(window.current_view());
            state.line_height = f32::from(window.line_height());
        }
        let Some(hitbox) = hitbox.take() else { return };
        let scroll = self.scroll.clone();
        window.on_mouse_event(move |event: &ScrollEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble
                && hitbox.should_handle_scroll(window)
                && scroll.scroll_event(event, window, cx)
            {
                cx.stop_propagation();
            }
        });
    }
}
