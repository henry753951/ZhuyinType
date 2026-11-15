//! With the real engine data (`tools/zt data fetch engine && tools/zt data compile`, found the way the app finds it):
//! loading, typing, the reading channel, practice sessions on the shared data, and latency.  Skipped, saying so, when
//! the data is not there.

mod common;

use std::time::Instant;

use common::TempDir;
use zt_core::engine::{DataFiles, EngineData, EngineSession};
use zt_core::keys::{Key, code};
use zt_core::service::{Service, ServiceConfig};

fn files() -> Option<DataFiles> {
    let files = DataFiles::find(None);
    if files.is_none() {
        eprintln!("real engine data not found ({:?}); skipped", DataFiles::search_path(None));
    }
    files
}

#[test]
fn loads_and_types_with_the_real_lexicon() {
    let Some(files) = files() else { return };
    assert!(files.compiled, "lexicon.bin");
    assert!(files.language_model.is_some(), "lm/");
    let data = EngineData::new(files.clone());
    let dir = TempDir::new();
    let mut session = EngineSession::new();
    session.open(&data, Some(&dir.join("personal"))).unwrap();
    println!(
        "engine load {:.1} ms (lm_chat: {}, channel: {})",
        session.load_ms().unwrap(),
        files.chat_model.is_some(),
        files.reading_channel.is_some()
    );
    assert_eq!(session.presentation().reading_channel_active, files.reading_channel.is_some());
    let mut latencies = Vec::new();
    for c in "su3cl3".chars() {
        let outcome = session.key(&common::key_for(c)).unwrap();
        latencies.push(outcome.elapsed_ms.unwrap_or_default());
    }
    assert_eq!(session.presentation().composition, "你好");
    // A longer sentence: 我們今天去看電影 (ㄨㄛˇ ㄇㄣˊ ㄐㄧㄣ ㄊㄧㄢ ㄑㄩˋ ㄎㄢˋ ㄉㄧㄢˋ ㄧㄥˇ).
    session.reset().unwrap();
    for c in "ji3ap6rup wu0 fm4d042u04u/3".chars() {
        let outcome = session.key(&common::key_for(c)).unwrap();
        latencies.push(outcome.elapsed_ms.unwrap_or_default());
    }
    let sentence = session.presentation().composition.clone();
    println!("sentence: {sentence}");
    assert_eq!(sentence.chars().count(), 8);
    assert!(sentence.starts_with("我們今天"), "{sentence}");
    session.key(&Key::new(code::DOWN, 0)).unwrap();
    assert!(session.presentation().count > 1);
    latencies.sort_by(f64::total_cmp);
    println!("key latency ms p50={:.3} max={:.3}", latencies[latencies.len() / 2], latencies[latencies.len() - 1]);
}

#[test]
fn service_and_practice_share_the_data() {
    if files().is_none() {
        return;
    }
    let dir = TempDir::new();
    let config = ServiceConfig { in_memory: true, ..ServiceConfig::default() };
    let started = Instant::now();
    let (service, _events) = Service::start(config).unwrap();
    println!("service start {:.1} ms", started.elapsed().as_secs_f64() * 1000.0);
    assert!(service.engine_error().is_none(), "{:?}", service.engine_error());
    let started = Instant::now();
    let mut first = service.practice().unwrap();
    println!("first practice field {:.1} ms", started.elapsed().as_secs_f64() * 1000.0);
    let started = Instant::now();
    let second = service.practice().unwrap();
    println!("second practice field (shared engine) {:.1} ms", started.elapsed().as_secs_f64() * 1000.0);
    for c in "su3cl3".chars() {
        first.key_down(&common::key_for(c));
    }
    assert_eq!(first.presentation().composition, "你好");
    first.key_down(&Key::new(code::RETURN, 0));
    assert_eq!(first.text(), "你好");
    assert!(second.presentation().is_empty());
    drop(dir);
}
