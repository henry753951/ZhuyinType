//! 更新後提示更新內容: after an update, one system notification with what this version changed (docs/product/DESIGN.md
//! §3.6).  A Windows toast or the desktop's notification on Linux (GPUI posts both): the system draws it, it never
//! takes the focus, makes no sound here, keeps quiet in 「請勿打擾」, and stays in the notification centre.
//!
//! It waits for the first typing after the update to pause: not while composing, not in a password field, not over the
//! settings or the wizard, and not while something is full screen (a game, a film, a presentation).

use std::time::Duration;

use gpui_kit::{App, Global, SystemNotification, SystemNotificationAction};
use zt_core::release_notes::{self, Announcement};

use crate::app;
use crate::pages::about;

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// How often it looks whether the moment has come, and how long the typing must have paused.
const TICK: Duration = Duration::from_secs(2);
const PAUSE: Duration = Duration::from_secs(4);
const TAG: &str = "update-notes";
const FEEDBACK: &str = "feedback";
/// The app's identity for notifications (Windows' AppUserModelID; the uninstaller removes its key, installer.nsi).
const IDENTITY: &str = "org.zhuyintype.ZhuyinType";

/// The announcement still to make.
struct Pending(Announcement);

impl Global for Pending {}

/// Decides whether this start follows an update, and announces it once the user pauses.  Call before anything else
/// stores settings; the copy that serves the input method only.
pub fn start(cx: &mut App) {
    // Without the version it last announced (0.7.6 and earlier kept none): the wizard was seen or updates were
    // checked before, so this is an update and not a new install.
    let used_before = crate::wizard::seen(cx) || crate::updates::load_last_check().is_some();
    let Some(announcement) = release_notes::on_start(app::settings(cx), VERSION, used_before) else { return };
    cx.set_global(Pending(announcement));
    cx.on_system_notification_response(|response, cx| {
        if response.tag != TAG {
            return;
        }
        let place = match response.action_id.as_deref() {
            Some(FEEDBACK) => about::feedback_place(),
            _ => about::notes_place(),
        };
        crate::shell::show(cx);
        app::go(place, cx);
    });
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(TICK).await;
            if cx.update(tick) {
                break;
            }
        }
    })
    .detach();
}

/// Shows it if the moment has come; whether it is done with.
fn tick(cx: &mut App) -> bool {
    let Some(announcement) = cx.try_global::<Pending>().map(|pending| pending.0.clone()) else { return true };
    if !app::settings(cx).bool(release_notes::SHOW_KEY) {
        seen(cx);
        return true;
    }
    let paused = app::service(cx).and_then(zt_core::service::Service::typing_paused);
    let quiet = paused.is_some_and(|paused| paused >= PAUSE)
        && !crate::shell::is_open(cx)
        && !crate::wizard::is_open(cx)
        && !crate::platform::full_screen();
    if !quiet {
        return false;
    }
    show(announcement, cx);
    seen(cx);
    true
}

fn show(announcement: Announcement, cx: &App) {
    // Only now: on Windows the identity also groups the app's windows on the taskbar.
    cx.set_app_identity(IDENTITY, "ZhuyinType");
    let action =
        |id: &'static str, label: &'static str| SystemNotificationAction { id: id.into(), label: label.into() };
    cx.show_system_notification(SystemNotification {
        tag: TAG.into(),
        title: announcement.title.into(),
        body: announcement.body.into(),
        actions: vec![action("notes", "查看更新內容"), action(FEEDBACK, "回報問題")],
    });
}

/// This version's notes need no announcement any more (it was shown, or 「更新內容」 was opened).
pub fn seen(cx: &mut App) {
    if cx.has_global::<Pending>() {
        cx.remove_global::<Pending>();
        release_notes::remember(app::settings(cx), VERSION);
    }
}

/// `--update-notice`: the notification with this version's notes, at once (screenshots and by hand).
pub fn demo(cx: &mut App) {
    let dir = std::env::temp_dir().join(format!("zt-update-notice-{}", std::process::id()));
    let settings = zt_core::settings::Settings::open(Some(dir.join("settings.json")));
    if let Some(announcement) = release_notes::on_start(&settings, VERSION, true) {
        show(announcement, cx);
    }
}
