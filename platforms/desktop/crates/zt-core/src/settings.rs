//! The settings: their descriptors (`settings.json`, shared with every page and the wizard) and the stored values.
//!
//! Descriptors say what a setting is (title, page, group, kind, default, options, range); `Settings` validates,
//! stores and notifies.  Values live in one JSON file in the user's config directory, written atomically.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
            Kind::Number => value.as_f64().is_some_and(|number| {
                self.min.is_none_or(|min| number >= min) && self.max.is_none_or(|max| number <= max)
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
}

impl Settings {
    /// The settings in `path` (missing or unreadable: all defaults).  `None` keeps them in memory only (previews,
    /// tests).
    pub fn open(path: Option<PathBuf>) -> Self {
        let values = path.as_deref().and_then(|path| read(path).ok()).unwrap_or_default();
        Self { inner: Arc::new(Inner { path, values: RwLock::new(values), listeners: RwLock::default() }) }
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
        self.inner.values.write().insert(key.to_owned(), value);
        self.save()?;
        self.notify(key);
        Ok(())
    }

    /// Back to the default.
    pub fn reset(&self, key: &str) -> Result<()> {
        if self.inner.values.write().remove(key).is_some() {
            self.save()?;
            self.notify(key);
        }
        Ok(())
    }

    /// Every setting back to its default.
    pub fn reset_all(&self) -> Result<()> {
        let keys: Vec<String> = std::mem::take(&mut *self.inner.values.write()).into_keys().collect();
        self.save()?;
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
        for listener in self.inner.listeners.read().iter() {
            listener(key);
        }
    }

    fn save(&self) -> Result<()> {
        let Some(path) = &self.inner.path else { return Ok(()) };
        let text = serde_json::to_string_pretty(&*self.inner.values.read())?;
        write_atomically(path, text.as_bytes())
    }
}

fn read(path: &Path) -> Result<BTreeMap<String, Value>> {
    let text = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

/// Writes `bytes` to a temporary file beside `path`, then renames it over `path`.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes).with_context(|| format!("writing {}", temporary.display()))?;
    std::fs::rename(&temporary, path).with_context(|| format!("replacing {}", path.display()))?;
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
