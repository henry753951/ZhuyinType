//! 「分析」 (macOS `AnalyticsPane`): a recap of this computer's typing (counts only), the typing rhythm, the counters,
//! how the input method itself performs (the 效能 sub-page) and the switch for counting.  Refreshes every two
//! seconds only while the page is on screen.  Without the service it shows the empty state: zeros, no daily chart, and
//! 清除 unavailable.

mod data;

use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants as _};
use gpui_kit::component::{Disableable as _, Sizable as _, WindowExt as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, px,
};
use serde_json::Value;
use zt_core::stats::{Period, ProcessMetrics, Usage};

use super::Page;
use crate::app::{self, Place};
use crate::theme::{self, size};
use crate::ui::charts::{self, grouped};
use crate::ui::{self, v};

pub struct AnalyticsPage {
    period: Period,
    /// The day under the pointer in the daily chart.
    hovered_day: Option<usize>,
    /// (user phrases, learned words), counted every refresh while the service runs.
    words: Option<(usize, usize)>,
    revision: u64,
}

fn sample_requested() -> bool {
    std::env::var("ZT_DEMO").is_ok_and(|demo| demo.split(',').any(|item| item.trim() == "analytics-sample"))
}

impl AnalyticsPage {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let period = app::service(cx).map_or(Period::Month, |service| service.stats().period());
        // Every two seconds while the page is on screen: new numbers when the statistics changed, and the process's
        // CPU and memory on the 效能 sub-page.  Nothing is redrawn while another page is shown.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                let alive = this.update(cx, |page, cx| {
                    let nav = app::nav(cx);
                    if nav.page != Page::Analytics {
                        return;
                    }
                    let performance = nav.subpage.as_deref() == Some("performance");
                    let revision = app::service(cx).map_or(0, |service| service.stats().revision());
                    if performance || revision != page.revision {
                        page.revision = revision;
                        page.count_words(cx);
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        let mut page = Self { period, hovered_day: None, words: None, revision: 0 };
        page.count_words(cx);
        page
    }

    fn count_words(&mut self, cx: &App) {
        self.words = app::service(cx).map(|service| (service.user_phrases().len(), service.learned_entries().len()));
    }

    fn usage(&self, cx: &App) -> Usage {
        match app::service(cx) {
            Some(service) => service.stats().usage(),
            None if sample_requested() => data::sample(self.period),
            None => data::empty(),
        }
    }

    fn set_period(&mut self, period: Period, cx: &mut Context<Self>) {
        self.period = period;
        if let Some(service) = app::service(cx) {
            service.stats().set_period(period);
        }
        cx.notify();
    }

    fn recap(&self, usage: &Usage, counting: bool, cx: &App) -> AnyElement {
        let c = &usage.counters;
        let mut chips: Vec<AnyElement> = Vec::new();
        if usage.streak > 1 {
            chips.push(charts::info_chip(IconName::Flame, format!("連續 {} 天", usage.streak), cx));
        }
        if let Some(best) = usage.best_day {
            chips.push(charts::info_chip(
                IconName::Star,
                format!("最多的一天 {}・{} 字", data::month_day_long(best.date), grouped(best.characters)),
                cx,
            ));
        }
        if let Some(right) = usage.first_time_right {
            chips.push(charts::info_chip(IconName::BadgeCheck, format!("一次就對 {}%", (right * 100.).round()), cx));
        }
        if c.characters > 0 {
            let rate = c.corrections() as f64 * 100. / c.characters as f64;
            chips.push(charts::info_chip(IconName::Pencil, format!("每 100 字改 {rate:.1} 次"), cx));
        }
        if !counting {
            chips.push(charts::info_chip(IconName::CirclePause, "已暫停記錄", cx));
        }
        let caption = if self.period == Period::Month { "過去 30 天，你打了" } else { "開始記錄以來，你打了" };
        charts::recap_card(caption, grouped(c.characters), "個字", chips, cx).into_any_element()
    }

    fn daily_chart(&self, usage: &Usage, cx: &mut Context<Self>) -> AnyElement {
        let days = usage.days.clone();
        let values: Vec<f64> = days.iter().map(|d| d.characters as f64).collect();
        let labels: Vec<(usize, SharedString)> = (0..days.len())
            .rev()
            .step_by(7)
            .map(|index| (index, SharedString::from(data::month_day(days[index].date))))
            .collect();
        let best = days.iter().enumerate().max_by_key(|(_, d)| d.characters).filter(|(_, d)| d.characters > 0);
        let entity = cx.entity();
        let tips = days.clone();
        let mut chart = charts::bar_chart("daily", values, 150.).labels(labels).hover(
            self.hovered_day,
            move |index| {
                let day = tips[index];
                format!("{}・{} 字", data::month_day(day.date), grouped(day.characters)).into()
            },
            move |hovered, _, cx| {
                entity.update(cx, |page, cx| {
                    if page.hovered_day != hovered {
                        page.hovered_day = hovered;
                        cx.notify();
                    }
                });
            },
        );
        if let Some((index, day)) = best {
            chart = chart.annotate(index, grouped(day.characters));
        }
        div().w_full().p(px(12.)).child(chart).into_any_element()
    }

    fn rhythm(&self, usage: &Usage, cx: &App) -> Vec<AnyElement> {
        let c = &usage.counters;
        let r = &usage.rhythm;
        let tiles = charts::tile_grid(
            3,
            110.,
            [
                charts::stat_tile(IconName::Gauge, "打字速度（字/分鐘）", format!("{}", usage.active_speed.unwrap_or(0.).round()), cx),
                charts::stat_tile(
                    IconName::BadgeCheck,
                    "一次就對（%）",
                    format!("{}", (usage.first_time_right.unwrap_or(0.) * 100.).round()),
                    cx,
                ),
                charts::stat_tile(IconName::ArrowRightToLine, "最長連續（字）", grouped(r.longest_run), cx),
            ],
        );
        let p = theme::palette(cx);
        let rows = ui::section("")
            .row(
                ui::row("rhythm-time", "打字時間・思考停頓")
                    .subtitle(format!("停頓超過 {:.0} 秒算思考，超過 1 分鐘不計", r.pause_threshold))
                    .value(format!("{}・{}", duration(c.typing_milliseconds), duration(c.thinking_milliseconds)))
                    .trailing(charts::split_bar(c.typing_milliseconds, c.thinking_milliseconds, cx)),
            )
            .row(ui::row("rhythm-clean", "一次不改字最長").value(format!("{} 字", grouped(r.longest_clean_run))))
            .row(ui::row("rhythm-keys", "每個字按幾個鍵").value(if c.characters > 0 {
                format!("{:.1}", c.keys as f64 / c.characters as f64)
            } else {
                "—".into()
            }))
            .row(ui::row("rhythm-pause", "思考停頓的中位數").value(r.pause_median.map_or("—".into(), |s| format!("{s:.1} 秒"))))
            .row(
                v().w_full()
                    .px(size::ROW_PADDING_X)
                    .py(size::ROW_PADDING_Y)
                    .gap(px(8.))
                    .child(div().text_color(p.text).child("什麼時候打字"))
                    .child(charts::heat_map(&r.hours, cx)),
            )
            .footer("打字速度只算連續打字的時間；一次就對是沒有選字、⌫ 或 Esc 就送出的組字。");
        vec![
            ui::section("打字節奏").bare().row(tiles).into_any_element(),
            rows.into_any_element(),
        ]
    }

    fn performance(&self, usage: &Usage, cx: &App) -> impl IntoElement {
        let latency = &usage.latency;
        let metrics: Option<ProcessMetrics> =
            if app::service(cx).is_some() { Some(zt_core::stats::process_metrics()) } else { data::local_metrics() };
        let mut section = ui::section("");
        if latency.total > 0 {
            section = section.row(
                ui::row("perf-latency", "按鍵反應")
                    .subtitle(format!("從按鍵到畫面更新；{} 個按鍵", grouped(latency.total)))
                    .value(format!("p50 {}・p95 {}・最慢 {}", ms(latency.p50), ms(latency.p95), ms(latency.maximum))),
            );
            let labels: Vec<String> = latency
                .bounds
                .iter()
                .map(|bound| if *bound < 1000. { format!("{bound:.0}") } else { "1s".into() })
                .chain(std::iter::once("更慢".to_owned()))
                .collect();
            let shown: Vec<(String, i64)> = labels
                .into_iter()
                .zip(latency.counts.iter().copied())
                .enumerate()
                .filter(|(offset, (_, count))| *count > 0 || *offset < 12)
                .map(|(_, pair)| pair)
                .collect();
            let chart = charts::bar_chart("latency", shown.iter().map(|(_, count)| *count as f64).collect(), 130.)
                .bar_ratio(0.7)
                .labels(shown.iter().enumerate().map(|(index, (label, _))| (index, label.clone().into())).collect())
                .caption("毫秒以內");
            section = section.row(div().w_full().p(px(12.)).child(chart));
        } else {
            section = section.row(ui::row("perf-latency", "按鍵反應").value("還沒有資料"));
        }
        let cpu = metrics.as_ref().and_then(|m| m.cpu_seconds);
        section = section.row(
            ui::row("perf-cpu", "CPU 時間")
                .subtitle_opt(metrics.as_ref().map(|m| format!("自 {} 啟動以來", clock(m.launched))))
                .value(cpu.map_or("無法取得".into(), |s| format!("{s:.1} 秒"))),
        );
        let memory: Vec<String> = metrics
            .as_ref()
            .map(|m| {
                [m.memory_bytes.map(|b| format!("目前 {}", bytes(b))), m.peak_memory_bytes.map(|b| format!("最高 {}", bytes(b)))]
                    .into_iter()
                    .flatten()
                    .collect()
            })
            .unwrap_or_default();
        section = section.row(
            ui::row("perf-memory", "記憶體").value(if memory.is_empty() { "無法取得".into() } else { memory.join("・") }),
        );
        let load = app::service(cx).and_then(|service| service.load_ms());
        section.row(ui::row("perf-load", "載入時間").subtitle("詞庫與語言模型").value(ms(load)))
    }

    fn main_page(&mut self, usage: &Usage, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let counting = app::settings(cx).bool("recordsUsageStats");
        let counting_marked = app::settings(cx).is_customised("recordsUsageStats");
        let c = usage.counters;
        let service = app::service(cx).is_some();
        let entity = cx.entity();
        let period = ui::segmented(
            "analytics-period",
            vec![("過去 30 天".into(), Period::Month), ("全部".into(), Period::All)],
            self.period,
            move |period, _, cx| entity.update(cx, |page, cx| page.set_period(period, cx)),
            cx,
        )
        .into_any_element();
        let daily = (c.characters > 0).then(|| self.daily_chart(usage, cx));
        let breakdown = |rows: &[(&str, i64)]| {
            let top = rows.iter().map(|(_, v)| *v).max().unwrap_or(0).max(1);
            rows.iter()
                .map(|(title, value)| {
                    ui::row(SharedString::from(format!("breakdown-{title}")), *title)
                        .value(grouped(*value))
                        .trailing(charts::meter(*value, top, cx))
                        .into_any_element()
                })
                .collect::<Vec<_>>()
        };
        let since = data::long_date(usage.since);
        let counting_subtitle = match (counting, service || sample_requested()) {
            (false, _) => "關閉時不計數".to_owned(),
            (true, true) => format!("從 {since} 開始記錄"),
            (true, false) => "輸入法服務沒有執行，目前沒有計數".to_owned(),
        };
        let p95 = if usage.latency.total > 0 { format!("p95 {}", ms(usage.latency.p95)) } else { "還沒有資料".into() };
        let words = self.words;
        let recap = self.recap(usage, counting, cx);
        charts::fill_column()
            .gap(size::SECTION_GAP)
            .child(v().gap(px(12.)).child(div().flex().child(period)).child(recap))
            .when_some(daily, |this, daily| this.child(ui::section("每天送出的字").row(daily)))
            .children(self.rhythm(usage, cx))
            .child(ui::section("打字").bare().row(charts::tile_grid(
                3,
                110.,
                [
                    charts::stat_tile(IconName::Type, "送出的字", grouped(c.characters), cx),
                    charts::stat_tile(IconName::TextCursor, "組字", grouped(c.compositions), cx),
                    charts::stat_tile(IconName::Keyboard, "按鍵", grouped(c.keys), cx),
                    charts::stat_tile(IconName::List, "從清單選字", grouped(c.picks()), cx),
                    charts::stat_tile(IconName::Pencil, "改字", grouped(c.corrections()), cx),
                    charts::stat_tile(IconName::CopyPlus, "加入自訂詞", grouped(c.phrases_added), cx),
                ],
            )))
            .child(ui::section("從清單選字").rows(breakdown(&[
                ("一般清單", c.picks_normal),
                ("修正清單", c.picks_corrections),
                ("單字清單", c.picks_single),
                ("標點", c.picks_punctuation),
            ])))
            .child(
                ui::section("改字")
                    .rows(breakdown(&[
                        ("⌫ 刪掉重打", c.backspaces),
                        ("Esc 清除", c.escapes),
                        ("送出後刪掉重打", c.delete_after_commit),
                        ("打出注音本身", c.literal_zhuyin),
                        ("恢復成原本的字", c.undo_literal),
                    ]))
                    .row(
                        ui::row("auto-corrections", "自動修正")
                            .subtitle("整句中有按錯修正的字：送出的組字 / 出現過的組字")
                            .value(format!("{} / {}", grouped(c.corrected_committed), grouped(c.corrected_shown))),
                    ),
            )
            .child(
                ui::section("你的字詞")
                    .row(ui::row("words-phrases", "自訂詞").value(words.map_or("—".into(), |w| format!("{} 個", grouped(w.0 as i64)))))
                    .row(ui::row("words-learned", "記住的字詞").value(words.map_or("—".into(), |w| format!("{} 個", grouped(w.1 as i64))))),
            )
            .child(
                ui::section("效能").row(
                    ui::row("performance", "效能")
                        .subtitle("按鍵反應、CPU、記憶體與載入時間")
                        .value(p95)
                        .on_click(|_, _, cx| app::go(Place::subpage(Page::Analytics, "performance"), cx)),
                ),
            )
            .child(
                ui::section("統計")
                    .row(
                        ui::row("records-usage", "記錄使用統計")
                            .subtitle(counting_subtitle)
                            .anchor("recordsUsageStats")
                            .marked(counting_marked)
                            .trailing(ui::switch("switch-recordsUsageStats", counting, |on, _, cx| {
                                app::set_setting("recordsUsageStats", Value::Bool(on), cx);
                                if let Some(service) = app::service(cx) {
                                    service.stats().set_enabled(on);
                                }
                            }))
                            .info("使用統計", |_, cx| usage_info(cx)),
                    )
                    .row(
                        ui::row("clear-stats", "清除統計").subtitle("刪除所有計數與每天的紀錄").trailing(
                            Button::new("clear-stats-button")
                                .small()
                                .outline()
                                .label("清除…")
                                .disabled(!service)
                                .on_click(|_, window, cx| confirm_clear(window, cx)),
                        ),
                    )
                    .footer("只記錄次數與時間，不記錄打的字、讀音與 App；只存在這台電腦。統計只在記憶體中累加，每 10 分鐘最多寫入一次，不影響打字。"),
            )
            .when(!usage.valid, |this| {
                this.child(div().px(size::ROW_PADDING_X).text_color(p.danger).child("無法讀取使用統計；按「清除…」重新開始計數。"))
            })
            .into_any_element()
    }
}

fn confirm_clear(window: &mut Window, cx: &mut App) {
    window.open_alert_dialog(cx, |alert, _, _| {
        alert
            .title("要清除使用統計嗎？")
            .description("所有計數與每天的紀錄都會刪除，無法復原。")
            .show_cancel(true)
            .ok_text("清除")
            .ok_variant(ButtonVariant::Danger)
            .cancel_text("取消")
            .on_ok(|_, window, cx| {
                if let Some(service) = app::service(cx)
                    && let Err(error) = service.stats().clear()
                {
                    window.push_notification(error.to_string(), cx);
                }
                cx.refresh_windows();
                true
            })
    });
}

/// What 使用統計 counts (docs/product/DESIGN.md §2.8).
fn usage_info(cx: &mut App) -> AnyElement {
    charts::fact_list(
        &[
            (IconName::Hash, "送出的字數、組字次數、按鍵數"),
            (IconName::List, "從各種清單選字的次數、各種改字的次數、加入自訂詞的次數"),
            (IconName::WandSparkles, "整句有按錯修正的組字數"),
            (IconName::Timer, "每個按鍵的反應時間，只記分布與最慢的一次"),
            (IconName::Calendar, "每天一筆，保留 60 天"),
            (IconName::EyeOff, "不記錄打的字、讀音、按了哪些鍵與 App"),
            (IconName::Lock, "密碼欄位中不計數"),
            (IconName::HardDrive, "存在這台電腦的 stats.json"),
        ],
        cx,
    )
}

/// 「14 小時 26 分」 or 「35 分」.
fn duration(milliseconds: i64) -> String {
    let minutes = milliseconds / 60_000;
    if minutes >= 60 { format!("{} 小時 {} 分", minutes / 60, minutes % 60) } else { format!("{minutes} 分") }
}

/// 「9.6 ms」, 「87 ms」, 「—」.
fn ms(value: Option<f64>) -> String {
    match value {
        None => "—".into(),
        Some(value) if value < 10. => format!("{value:.1} ms"),
        Some(value) => format!("{value:.0} ms"),
    }
}

/// 「245.3 MB」, 「6.47 GB」.
fn bytes(value: f64) -> String {
    const MB: f64 = 1024. * 1024.;
    if value >= 1024. * MB { format!("{:.2} GB", value / (1024. * MB)) } else { format!("{:.1} MB", value / MB) }
}

/// 「凌晨 3:27」, 「下午 2:05」.
fn clock(time: chrono::DateTime<chrono::Local>) -> String {
    use chrono::Timelike;
    let hour = time.hour();
    let part = match hour {
        0..=5 => "凌晨",
        6..=11 => "上午",
        12..=17 => "下午",
        _ => "晚上",
    };
    let twelve = if hour % 12 == 0 { 12 } else { hour % 12 };
    format!("{part} {twelve}:{:02}", time.minute())
}

impl Render for AnalyticsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let usage = self.usage(cx);
        if app::nav(cx).subpage.as_deref() == Some("performance") {
            return v().gap(size::SECTION_GAP).child(self.performance(&usage, cx)).into_any_element();
        }
        self.main_page(&usage, cx)
    }
}

/// The title of sub-page `id` (shown in the bar above it), or None if this page has no such sub-page.
pub fn subpage_title(id: &str) -> Option<&'static str> {
    (id == "performance").then_some("效能")
}
