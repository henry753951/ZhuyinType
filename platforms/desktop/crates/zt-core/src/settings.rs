//! The settings: their descriptors (`settings.json`, shared with every page and the wizard) and the stored values.
//!
//! Descriptors say what a setting is (title, page, group, kind, default, options, range); `Settings` validates,
//! stores and notifies.  Values live in one JSON file in the user's config directory, written atomically.  A file that
//! cannot be read (cut short by a crash, broken in an editor) is set aside as `settings.unreadable.json` before the
//! first change replaces it, so the user's settings and keys are never silently lost.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use anyhow::{Context as _, Result};
use parking_lot::RwLock;
use serde::Deserialize;
use serde_json::Value;

/// The descriptors, built into the binary.
const REGISTRY: &str = include_str!("../../../settings.json");

/// One option of a choice setting.
#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    /// The stored value.
    pub value: Value,
    /// What the UI shows.
    pub label: String,
}

/// What kind of control a setting has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// On or off.
    Toggle,
    /// One of `options`.
    Choice,
    /// A number in `min…max`.
    Number,
    /// Free text (selection keys).
    Text,
}

/// One setting's descriptor, as in `settings.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    /// The key the value is stored under.
    pub key: String,
    /// The row's title.
    pub title: String,
    /// The page the row is on (`typing`, `choosing`, …).
    #[serde(default)]
    pub page: String,
    /// The section on that page.
    #[serde(default)]
    pub group: String,
    /// The control.
    pub kind: Kind,
    /// The value when the user has not changed it.
    pub default: Value,
    /// A choice's options.
    #[serde(default)]
    pub options: Vec<Choice>,
    /// The subtitle when no per-value detail applies.
    #[serde(default)]
    pub detail: Option<String>,
    /// The subtitle for each value of a choice.
    #[serde(default)]
    pub details: BTreeMap<String, String>,
    /// A number's range and step.
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub step: Option<f64>,
    /// The id links and searches scroll to.
    #[serde(default)]
    pub anchor: Option<String>,
    /// Every other field of the descriptor, for pages that need more than the common ones.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Descriptor {
    /// The subtitle for `value`: its own detail when the descriptor has one, else the general detail.
    pub fn subtitle(&self, value: &Value) -> Option<&str> {
        let key = match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        };
        self.details.get(&key).or(self.detail.as_ref()).map(String::as_str)
    }

    /// The label of `value` among the options.
    pub fn label(&self, value: &Value) -> Option<&str> {
        self.options.iter().find(|choice| &choice.value == value).map(|choice| choice.label.as_str())
    }

    /// Whether `value` is valid for this setting.
    pub fn accepts(&self, value: &Value) -> bool {
        match self.kind {
            Kind::Toggle => value.is_boolean(),
            Kind::Choice => self.options.iter().any(|choice| &choice.value == value),
            // Every number setting is whole (font size, candidates per page).
            Kind::Number => value.as_f64().is_some_and(|number| {
                number.is_finite()
                    && number.fract() == 0.0
                    && self.min.is_none_or(|min| number >= min)
                    && self.max.is_none_or(|max| number <= max)
            }),
            Kind::Text => value.is_string(),
        }
    }
}

/// Every descriptor, in `settings.json` order.
pub fn descriptors() -> &'static [Descriptor] {
    static LIST: std::sync::OnceLock<Vec<Descriptor>> = std::sync::OnceLock::new();
    LIST.get_or_init(|| serde_json::from_str(REGISTRY).expect("settings.json is valid"))
}

/// The descriptor of `key`.
pub fn descriptor(key: &str) -> Option<&'static Descriptor> {
    descriptors().iter().find(|descriptor| descriptor.key == key)
}

type Listener = Box<dyn Fn(&str) + Send + Sync>;

/// The stored values.  Cheap to clone; every clone shares the same values and listeners.
#[derive(Clone)]
pub struct Settings {
    inner: Arc<Inner>,
}

struct Inner {
    path: Option<PathBuf>,
    values: RwLock<BTreeMap<String, Value>>,
    listeners: RwLock<Vec<Listener>>,
    revision: AtomicU64,
    /// The file is there but could not be read: set it aside before writing over it.
    unreadable: AtomicBool,
}

impl Settings {
    /// The settings in `path` (missing or unreadable: all defaults).  `None` keeps them in memory only (previews,
    /// tests).
    pub fn open(path: Option<PathBuf>) -> Self {
        let mut unreadable = false;
        let values = match path.as_deref().map(|path| (path, read(path))) {
            Some((_, Ok(values))) => values,
            Some((path, Err(error))) => {
                unreadable = path.exists();
                if unreadable {
                    log::warn!("settings: {} cannot be read ({error:#}); using the defaults", path.display());
                }
                BTreeMap::new()
            }
            None => BTreeMap::new(),
        };
        Self {
            inner: Arc::new(Inner {
                path,
                values: RwLock::new(values),
                listeners: RwLock::default(),
                revision: AtomicU64::new(0),
                unreadable: AtomicBool::new(unreadable),
            }),
        }
    }

    /// The file is there but could not be read, so the defaults are in use (until the first change sets it aside).
    pub fn unreadable(&self) -> bool {
        self.inner.unreadable.load(Ordering::Acquire)
    }

    /// The file the settings are kept in (None: in memory only).
    pub fn path(&self) -> Option<&Path> {
        self.inner.path.as_deref()
    }

    /// The value of `key`: the stored one when valid, else the default.
    pub fn get(&self, key: &str) -> Value {
        let stored = self.inner.values.read().get(key).cloned();
        match (stored, descriptor(key)) {
            (Some(value), Some(descriptor)) if descriptor.accepts(&value) => value,
            (_, Some(descriptor)) => descriptor.default.clone(),
            (Some(value), None) => value,
            (None, None) => Value::Null,
        }
    }

    /// `key` as a bool (false when it is not one).
    pub fn bool(&self, key: &str) -> bool {
        self.get(key).as_bool().unwrap_or(false)
    }

    /// `key` as a string (empty when it is not one).
    pub fn string(&self, key: &str) -> String {
        self.get(key).as_str().unwrap_or_default().to_owned()
    }

    /// `key` as a number (0 when it is not one).
    pub fn number(&self, key: &str) -> f64 {
        self.get(key).as_f64().unwrap_or(0.0)
    }

    /// Whether `key` differs from its default.
    pub fn is_customised(&self, key: &str) -> bool {
        descriptor(key).is_some_and(|descriptor| self.get(key) != descriptor.default)
    }

    /// Sets `key`; invalid values are refused.  Saves and notifies when the value changed.
    pub fn set(&self, key: &str, value: Value) -> Result<()> {
        if let Some(descriptor) = descriptor(key) {
            anyhow::ensure!(descriptor.accepts(&value), "invalid value {value} for {key}");
        }
        if self.get(key) == value {
            return Ok(());
        }
        let previous = self.inner.values.write().insert(key.to_owned(), value);
        if let Err(error) = self.save() {
            let mut values = self.inner.values.write();
            match previous {
                Some(previous) => values.insert(key.to_owned(), previous),
                None => values.remove(key),
            };
            return Err(error);
        }
        self.notify(key);
        Ok(())
    }

    /// Sets several values at once, all or none (the wizard's copy and 套用).  Unknown keys are stored as they are.
    ///
    /// # Errors
    /// When a value is invalid (nothing changes) or the file cannot be written (nothing changes).
    pub fn assign(&self, values: &BTreeMap<String, Value>) -> Result<()> {
        for (key, value) in values {
            if let Some(descriptor) = descriptor(key) {
                anyhow::ensure!(descriptor.accepts(value), "「{}」的值無效。", descriptor.title);
            }
        }
        let changed: Vec<String> = values.iter().filter(|(k, v)| self.get(k) != **v).map(|(k, _)| k.clone()).collect();
        if changed.is_empty() {
            return Ok(());
        }
        let before = self.inner.values.read().clone();
        self.inner.values.write().extend(values.iter().map(|(k, v)| (k.clone(), v.clone())));
        if let Err(error) = self.save() {
            *self.inner.values.write() = before;
            return Err(error);
        }
        for key in changed {
            self.notify(&key);
        }
        Ok(())
    }

    /// The stored values (only what differs from the defaults, plus keys without a descriptor).
    #[must_use]
    pub fn stored(&self) -> BTreeMap<String, Value> {
        self.inner.values.read().clone()
    }

    /// An in-memory copy of these settings (the wizard's and the practice field's sandbox): changes to it never
    /// reach this store or its file.
    #[must_use]
    pub fn sandbox(&self) -> Settings {
        let copy = Settings::open(None);
        *copy.inner.values.write() = self.stored();
        copy
    }

    /// Increases with every change; cheap to read on every key.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.inner.revision.load(Ordering::Acquire)
    }

    /// Back to the default.  When it cannot be saved, nothing changes.
    pub fn reset(&self, key: &str) -> Result<()> {
        let Some(previous) = self.inner.values.write().remove(key) else { return Ok(()) };
        if let Err(error) = self.save() {
            self.inner.values.write().insert(key.to_owned(), previous);
            return Err(error);
        }
        self.notify(key);
        Ok(())
    }

    /// Every setting of the registry back to its default; other stored values (keys, app lists) stay.  When it
    /// cannot be saved, nothing changes.
    pub fn reset_all(&self) -> Result<()> {
        let before = self.inner.values.read().clone();
        let keys: Vec<String> = {
            let mut values = self.inner.values.write();
            let keys: Vec<String> = values.keys().filter(|key| descriptor(key).is_some()).cloned().collect();
            for key in &keys {
                values.remove(key);
            }
            keys
        };
        if let Err(error) = self.save() {
            *self.inner.values.write() = before;
            return Err(error);
        }
        for key in keys {
            self.notify(&key);
        }
        Ok(())
    }

    /// Calls `listener` with the key of every change.
    pub fn observe(&self, listener: impl Fn(&str) + Send + Sync + 'static) {
        self.inner.listeners.write().push(Box::new(listener));
    }

    fn notify(&self, key: &str) {
        self.inner.revision.fetch_add(1, Ordering::AcqRel);
        for listener in self.inner.listeners.read().iter() {
            listener(key);
        }
    }

    fn save(&self) -> Result<()> {
        let Some(path) = &self.inner.path else { return Ok(()) };
        if self.inner.unreadable.load(Ordering::Acquire) {
            set_aside(path)?;
            self.inner.unreadable.store(false, Ordering::Release);
        }
        let text = serde_json::to_string_pretty(&*self.inner.values.read())?;
        write_atomically(path, text.as_bytes())
    }
}

/// Keeps the settings file that could not be read as `<name>.unreadable.json` (replacing an older one), so the
/// user can recover it; fails rather than let it be written over.
fn set_aside(path: &Path) -> Result<()> {
    let aside = path.with_extension("unreadable.json");
    if std::fs::rename(path, &aside).is_err() && path.exists() {
        std::fs::copy(path, &aside).with_context(|| format!("keeping {}", path.display()))?;
    }
    log::warn!("settings: the unreadable file was kept as {}", aside.display());
    Ok(())
}

fn read(path: &Path) -> Result<BTreeMap<String, Value>> {
    let text = std::fs::read_to_string(path)?;
    // Editors on Windows (Notepad, PowerShell) may start the file with a byte-order mark.
    Ok(serde_json::from_str(text.trim_start_matches('\u{feff}'))?)
}

/// Writes `bytes` to a temporary file of its own beside `path`, flushes it to the disk, then renames it over `path`:
/// after a crash or power loss `path` holds either the old contents or the new, never a part or zeros.  Writers at the
/// same time each leave a whole file; another file of the same stem is never touched.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write as _;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let mut name = path.file_name().map(std::ffi::OsStr::to_os_string).unwrap_or_default();
    name.push(format!(".{}-{}.tmp", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let temporary = path.with_file_name(name);
    let written = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)
    })();
    if let Err(error) = written {
        let _ = std::fs::remove_file(&temporary);
        return Err(error).with_context(|| format!("replacing {}", path.display()));
    }
    // The rename itself reaches the disk with the directory (Linux; NTFS journals it).
    #[cfg(unix)]
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty())
        && let Ok(directory) = std::fs::File::open(parent)
    {
        let _ = directory.sync_all();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_parses_and_defaults_are_valid() {
        assert!(descriptors().len() >= 40);
        for descriptor in descriptors() {
            assert!(descriptor.accepts(&descriptor.default), "{}", descriptor.key);
        }
    }

    #[test]
    fn a_byte_order_mark_is_ignored() {
        let dir = std::env::temp_dir().join(format!("zt-settings-bom-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(&path, "\u{feff}{\"panelAppearance\": \"dark\"}").unwrap();
        assert_eq!(Settings::open(Some(path)).string("panelAppearance"), "dark");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn invalid_values_are_refused() {
        let settings = Settings::open(None);
        assert!(settings.set("spaceAction", Value::from("nonsense")).is_err());
        settings.set("spaceAction", Value::from("insert")).unwrap();
        assert_eq!(settings.string("spaceAction"), "insert");
        assert!(settings.is_customised("spaceAction"));
        settings.reset("spaceAction").unwrap();
        assert!(!settings.is_customised("spaceAction"));
    }
}
