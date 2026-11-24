//! Linux: whether typing reaches ZhuyinType at all.  It needs Fcitx 5 as the session's input method framework, and
//! GNOME (Ubuntu's and Fedora's default desktops) starts IBus instead; nothing else tells the user.  The settings
//! window asks the package's helper (`packaging/linux/fcitx5-setup status`) once and, when Fcitx 5 is not running,
//! says so above the page with a button that makes Fcitx 5 this user's framework (`fcitx5-setup framework`, effective
//! at the next sign-in).

use std::path::PathBuf;
use std::process::Command;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{AnyElement, App, Global, IntoElement, ParentElement, SharedString, Styled, div};

use crate::theme::{self, size as sz, space, text};
use crate::ui::actions::{ButtonKind, push_button};
use crate::ui::h;

/// The helper the package installs: beside a development build's `bin/`, else the package's place.
pub fn fcitx_helper() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok();
    [
        exe.as_ref().and_then(|exe| exe.parent()).map(|bin| bin.join("../lib/zhuyintype/fcitx5-setup")),
        Some(PathBuf::from("/usr/lib/zhuyintype/fcitx5-setup")),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.is_file())
}

/// The active X11 window is full screen.  In a Wayland session no client may know; the desktop itself holds
/// notifications back there (GNOME and KDE do over a full-screen window).
pub fn full_screen() -> bool {
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};

    fn active_window_is_full_screen() -> Option<bool> {
        let (connection, screen) = x11rb::connect(None).ok()?;
        let root = connection.setup().roots.get(screen)?.root;
        let atom = |name: &[u8]| Some(connection.intern_atom(false, name).ok()?.reply().ok()?.atom);
        let (active, state, full) =
            (atom(b"_NET_ACTIVE_WINDOW")?, atom(b"_NET_WM_STATE")?, atom(b"_NET_WM_STATE_FULLSCREEN")?);
        let window = connection.get_property(false, root, active, AtomEnum::WINDOW, 0, 1).ok()?.reply().ok()?;
        let window = window.value32()?.next()?;
        let states = connection.get_property(false, window, state, AtomEnum::ATOM, 0, 32).ok()?.reply().ok()?;
        Some(states.value32()?.any(|state| state == full))
    }
    active_window_is_full_screen().unwrap_or(false)
}

/// What the notice says.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Framework {
    /// Not known yet, or Fcitx 5 runs: no notice.
    Fine,
    /// Fcitx 5 does not run in this session.
    NotRunning,
    /// Fcitx 5 becomes the framework at the next sign-in.
    NextLogin,
    /// `fcitx5-setup framework` could not do it; its reason.
    Failed(String),
}

struct FrameworkNotice {
    state: Framework,
    switching: bool,
}

impl Global for FrameworkNotice {}

/// What `fcitx5-setup status` printed (`key=value` lines), as the notice's state.
fn read_status(status: &str) -> Framework {
    let value = |key: &str| status.lines().find_map(|line| line.strip_prefix(key)?.strip_prefix('='));
    match (value("fcitx5"), value("running"), value("framework")) {
        (Some("installed"), Some("no"), Some("fcitx5-next-login")) => Framework::NextLogin,
        (Some("installed"), Some("no"), _) => Framework::NotRunning,
        _ => Framework::Fine,
    }
}

/// Asks the helper once, off the UI thread (the settings window, when it opens).
pub fn check(cx: &mut App) {
    if cx.has_global::<FrameworkNotice>() {
        return;
    }
    cx.set_global(FrameworkNotice { state: Framework::Fine, switching: false });
    let Some(helper) = fcitx_helper() else { return };
    let task = cx.background_executor().spawn(async move {
        Command::new(helper)
            .arg("status")
            .output()
            .ok()
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
    });
    cx.spawn(async move |cx| {
        let Some(status) = task.await else { return };
        cx.update(|cx| {
            cx.global_mut::<FrameworkNotice>().state = read_status(&status);
            cx.refresh_windows();
        });
    })
    .detach();
}

/// 改用 Fcitx 5: `fcitx5-setup framework` (im-config or imsettings), effective at the next sign-in.
fn switch(cx: &mut App) {
    let Some(helper) = fcitx_helper() else { return };
    cx.global_mut::<FrameworkNotice>().switching = true;
    cx.refresh_windows();
    let task = cx.background_executor().spawn(async move { Command::new(helper).arg("framework").output() });
    cx.spawn(async move |cx| {
        let result = task.await;
        cx.update(|cx| {
            let notice = cx.global_mut::<FrameworkNotice>();
            notice.switching = false;
            notice.state = match result {
                Ok(output) if output.status.success() => Framework::NextLogin,
                Ok(output) => {
                    let reason = String::from_utf8_lossy(&output.stderr);
                    let reason = reason.trim().trim_start_matches("fcitx5-setup：").to_owned();
                    Framework::Failed(if reason.is_empty() { output.status.to_string() } else { reason })
                }
                Err(error) => Framework::Failed(error.to_string()),
            };
            cx.refresh_windows();
        });
    })
    .detach();
}

/// The notice above the settings page, when typing does not reach ZhuyinType.
pub fn notice(cx: &App) -> Option<AnyElement> {
    let notice = cx.try_global::<FrameworkNotice>()?;
    let (message, button) = match &notice.state {
        Framework::Fine => return None,
        Framework::NotRunning => (
            "Fcitx 5 沒有在執行，打字時不會用到 ZhuyinType。這台電腦的輸入法框架可能是 IBus（GNOME 的預設）；改用 Fcitx 5 \
             後，登出再登入一次即可。"
                .to_owned(),
            true,
        ),
        Framework::NextLogin => ("輸入法框架已設為 Fcitx 5，登出再登入一次後就能用 ZhuyinType 打字。".to_owned(), false),
        Framework::Failed(reason) => (
            format!("無法自動改用 Fcitx 5：{reason}。請依發行版的說明把輸入法框架設為 Fcitx 5，再登出、登入一次。"),
            false,
        ),
    };
    let p = theme::palette(cx);
    let switching = notice.switching;
    Some(
        h().mb(space::S20)
            .px(sz::ROW_PADDING_X)
            .py(space::S8)
            .gap(space::S12)
            .items_center()
            .rounded(sz::R_SM)
            .border_1()
            .border_color(p.accent)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(text::SUBTITLE)
                    .text_color(p.text)
                    .child(SharedString::from(message)),
            )
            .when(button, |this| {
                this.child(push_button(
                    "fcitx-framework",
                    if switching { "設定中…" } else { "改用 Fcitx 5" },
                    ButtonKind::Default,
                    !switching,
                    |_, _, cx| switch(cx),
                    cx,
                ))
            })
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notice_follows_the_helpers_status() {
        let status = |running: &str, framework: &str| {
            read_status(&format!(
                "fcitx5=installed\nrunning={running}\naddon=unknown\nprofile=absent\nframework={framework}\n"
            ))
        };
        assert_eq!(status("yes", "unknown"), Framework::Fine, "Fcitx runs (KDE sets no variables)");
        assert_eq!(status("no", "other"), Framework::NotRunning, "IBus");
        assert_eq!(status("no", "unknown"), Framework::NotRunning);
        assert_eq!(status("no", "fcitx5-next-login"), Framework::NextLogin);
        assert_eq!(read_status("fcitx5=missing\nrunning=no\n"), Framework::Fine);
        assert_eq!(read_status(""), Framework::Fine);
    }
}
