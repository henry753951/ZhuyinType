//! 回報問題 from a populated profile: a service with custom phrases, learned words, an app list and changed settings,
//! in a home directory named after the user, run through every check and section the app runs.  The report has
//! every section and none of the personal data.

mod common;

use std::collections::BTreeMap;

use common::{CONTROLLER_LEXICON, TempDir, key_for, write_lexicon};
use serde_json::json;
use zt_core::health::common::{DataFacts, ServiceFacts, SettingsFacts, UpdateFacts};
use zt_core::health::details::{AppFacts, DisplayFacts, ModelFacts};
use zt_core::health::report::{Report, System, text};
use zt_core::health::{self, Inputs};
use zt_core::keys::{Key, code};
use zt_core::service::{Service, ServiceConfig};

#[test]
fn a_populated_profile_leaves_no_personal_data_in_the_report() {
    let dir = TempDir::new();
    let home = dir.join("home/amy");
    let data = home.join("zhuyintype-data");
    std::fs::create_dir_all(&data).unwrap();
    write_lexicon(&data, CONTROLLER_LEXICON);
    let settings_file = home.join(".config/zhuyintype/settings.json");
    let config = ServiceConfig {
        data_dir: Some(data.clone()),
        user_dir: Some(home.join(".local/share/zhuyintype")),
        settings_file: Some(settings_file.clone()),
        app_version: "0.7.1".into(),
        ..ServiceConfig::default()
    };
    let service = Service::start(config).unwrap().0;

    // The profile: a custom phrase, a learned word, an app list, changed settings.
    service.add_phrase("書蘇", "ㄕㄨ ㄙㄨ").unwrap();
    service.with_controller(|input| {
        for c in "gjzj6".chars() {
            input.press(&key_for(c));
        }
        input.press(&Key::new(code::RETURN, 0));
    });
    assert!(!service.user_phrases().is_empty(), "the profile has a custom phrase");
    let settings = service.settings();
    settings.set("popupCompositionApps", json!(["secret-notes.exe", "diary-app"])).unwrap();
    settings.set("spaceAction", json!("candidates")).unwrap();
    settings.set("candidateFontSize", json!(22)).unwrap();
    settings.set("dataDirectory", json!(data.display().to_string())).unwrap();
    let learned: Vec<String> = service.learned_entries().into_iter().map(|phrase| phrase.word).collect();

    let (reranker_ms, character_ms) = service.model_load_ms();
    let inputs = Inputs {
        version: "0.7.1".into(),
        data: DataFacts::probe(service.data_files(), service.engine_error()),
        settings: SettingsFacts::probe(settings.path().map(Into::into), settings.unreadable()),
        service: ServiceFacts { running: true, error: None, reachable: None },
        endpoint: Some(home.join(".run/ime-v1").display().to_string()),
        updates: UpdateFacts::Current,
        fcitx_helper: None,
        about: AppFacts {
            diagnostics: Some(service.diagnostics()),
            models: ModelFacts { engine_ms: service.load_ms(), reranker_ms, character_ms, error: None },
            stored_settings: settings.stored(),
            user_data: Some(home.join(".local/share/zhuyintype")),
            executable: Some(home.join("bin/zhuyintype")),
            display: DisplayFacts { scale: Some(1.5), screen_reader: Some(false), ..DisplayFacts::default() },
            automatic_updates: true,
            last_update_check: Some("2026-11-18 13:00".into()),
            ..AppFacts::default()
        },
    };
    let outcome = health::run(&inputs);
    let errors = vec![format!("14:03:02 WARN settings: {} cannot be read", settings_file.display())];
    let system = System { os: "Linux".into(), desktop: String::new(), arch: "x86_64".into() };
    let report = text(&Report {
        version: "0.7.1",
        generated: "2026-11-18 14:03",
        system: &system,
        checks: &outcome.checks,
        sections: &outcome.sections,
        errors: Some(&errors),
        home: Some(&home),
        user: Some("amy"),
    });

    for heading in ["## ZhuyinType 問題回報", "### 檢查結果", "### 版本與資料", "### 安裝", "### 服務", "### 更新"]
    {
        assert!(report.contains(&format!("{heading}\n")), "{heading} missing:\n{report}");
    }
    assert!(report.contains("### 改過的設定\n\n"), "{report}");
    assert!(report.contains("- spaceAction：candidates\n"), "{report}");
    assert!(report.contains("- popupCompositionApps：（已設定）\n"), "{report}");
    assert!(report.contains("- dataDirectory：（已設定）\n"), "{report}");
    assert!(report.contains("- 資料夾：~/zhuyintype-data\n"), "{report}");
    assert!(report.contains("  - 改過的設定：4 項\n"), "{report}");
    assert!(
        report.contains("```text\n14:03:02 WARN settings: ~/.config/zhuyintype/settings.json cannot be read\n```\n")
    );

    let mut personal = vec!["amy", "書蘇", "ㄕㄨ ㄙㄨ", "secret-notes", "diary-app"];
    personal.extend(learned.iter().map(String::as_str));
    for secret in personal {
        assert!(!report.contains(secret), "{secret:?} is in the report:\n{report}");
    }
    // Nothing from the stored values beyond the closed-set ones.
    let stored: BTreeMap<String, serde_json::Value> = settings.stored();
    assert_eq!(stored.len(), 4);
}
