//! Helpers shared by the integration tests: temporary directories, small lexicons and keys.
#![allow(dead_code, unreachable_pub)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use zt_core::controller::{ControllerEvent, InputController};
use zt_core::engine::{EngineData, EngineSession};
use zt_core::keys::{Key, code};
use zt_core::settings::Settings;
use zt_core::shortcuts::Shortcuts;

/// A directory of its own (mode 0700), removed when dropped.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!(
            "zt-core-test-{}-{}-{nanos}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The lexicon of the Qt EngineSessionTest and ImeHostTest.
pub const LEXICON: &str = "ㄋㄧˇ\t你\t-3\nㄋㄧˇ\t妳\t-4\nㄋㄧˇ\t擬\t-5\nㄋㄧˇ\t旎\t-6\n\
ㄏㄠˇ\t好\t-3\nㄏㄠˇ\t郝\t-6\nㄋㄧˇ ㄏㄠˇ\t你好\t-2\n";

/// The lexicon of the Qt InputControllerTest.
pub const CONTROLLER_LEXICON: &str = "ㄋㄧˇ\t你\t-3\nㄋㄧˇ\t妳\t-4\nㄏㄠˇ\t好\t-3\n\
ㄋㄧˇ ㄏㄠˇ\t你好\t-2\nㄕㄨ\t書\t-3\nㄙㄨ\t蘇\t-4\nㄈㄨˊ\t服\t-3\nㄕㄨ ㄈㄨˊ\t舒服\t-2\n";

/// ㄊㄜˇ typed (`wk3`) reads 乙 through the tone correction: the exact 甲 is far rarer (the engine's
/// `correction_undo` test).
pub const CORRECTION_LEXICON: &str = "ㄊㄜˇ\t甲\t-9\nㄊㄜˋ\t乙\t0\nㄅㄚ\t八\t-2\n";

/// Writes `text` as `lexicon.tsv` in `dir`; returns its path.
pub fn write_lexicon(dir: &Path, text: &str) -> PathBuf {
    let path = dir.join("lexicon.tsv");
    std::fs::write(&path, text).unwrap();
    path
}

/// The engine data of a lexicon written into `dir`.
pub fn data(dir: &Path, text: &str) -> Arc<EngineData> {
    EngineData::from_lexicon(&write_lexicon(dir, text)).unwrap()
}

/// The key a test types for `c`, as the Qt tests did (`QChar::toUpper().unicode()`, no modifiers).
pub fn key_for(c: char) -> Key {
    if c == ' ' { Key::new(code::SPACE, 0) } else { Key::new(c.to_ascii_uppercase() as i32, 0) }
}

/// Types `keys` into a session.
pub fn type_session(session: &mut EngineSession, keys: &str) {
    for c in keys.chars() {
        session.key(&key_for(c)).unwrap();
    }
}

/// The input controller fixture of the Qt InputControllerTest: settings in a file, a small lexicon, personal data
/// in `personal`.
pub struct Fixture {
    pub dir: TempDir,
    pub settings: Settings,
    pub shortcuts: Shortcuts,
    pub input: InputController,
}

impl Fixture {
    pub fn new() -> Self {
        Self::with_lexicon(CONTROLLER_LEXICON)
    }

    pub fn with_lexicon(lexicon: &str) -> Self {
        let dir = TempDir::new();
        let settings = Settings::open(Some(dir.join("settings.json")));
        let shortcuts = Shortcuts::new(settings.clone());
        let mut session = EngineSession::new();
        session.apply(&settings).unwrap();
        session.open(&data(dir.path(), lexicon), Some(&dir.join("personal"))).unwrap();
        let input = InputController::new(session, settings.clone(), shortcuts.clone());
        Self { dir, settings, shortcuts, input }
    }

    pub fn type_keys(&mut self, keys: &str) {
        for c in keys.chars() {
            self.input.press(&key_for(c));
        }
    }

    pub fn press(&mut self, code: i32, modifiers: u32) -> bool {
        self.input.press(&Key::new(code, modifiers))
    }

    pub fn composition(&self) -> String {
        self.input.presentation().composition.clone()
    }

    /// The commits since the last call.
    pub fn commits(&mut self) -> Vec<String> {
        self.input
            .take_events()
            .into_iter()
            .filter_map(|event| match event {
                ControllerEvent::Commit(text) => Some(text),
                _ => None,
            })
            .collect()
    }
}
