//! What the input method service has been doing, for 回報問題's 服務 section: how long it has run, when the last key
//! arrived and how many adapters (the TSF text service in each app, the Fcitx 5 addon) are connected.  Counts and
//! times only: never a key, a field's text or an app's name.
//!
//! The service keeps one [`Activity`] and answers the broker's `diagnostics` request with [`Diagnostics`] (IPC.md),
//! so `zhuyintype --health` reads the running copy's facts, and its recent warnings and errors, from another process.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use serde_json::{Map, Value, json};

use crate::broker::ConnectionId;

/// The service's activity.
#[derive(Debug)]
pub struct Activity {
    started: Instant,
    started_at: SystemTime,
    /// Milliseconds since `started` at the last key press, plus one (0: none yet).
    last_key: AtomicU64,
    connections: Mutex<HashSet<ConnectionId>>,
}

impl Default for Activity {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            started_at: SystemTime::now(),
            last_key: AtomicU64::new(0),
            connections: Mutex::default(),
        }
    }
}

impl Activity {
    /// A request arrived on `connection`.
    pub fn seen(&self, connection: ConnectionId) {
        self.connections.lock().insert(connection);
    }

    /// `connection` closed.
    pub fn closed(&self, connection: ConnectionId) {
        self.connections.lock().remove(&connection);
    }

    /// A key was pressed.
    pub fn key(&self) {
        let elapsed = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX - 1);
        self.last_key.store(elapsed + 1, Ordering::Relaxed);
    }

    /// The facts now.
    #[must_use]
    pub fn diagnostics(&self, version: &str) -> Diagnostics {
        let uptime = self.started.elapsed();
        let last_key = match self.last_key.load(Ordering::Relaxed) {
            0 => None,
            at => Some(uptime.saturating_sub(Duration::from_millis(at - 1))),
        };
        Diagnostics {
            version: version.to_owned(),
            started: self.started_at.duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs()),
            uptime: uptime.as_secs(),
            last_key: last_key.map(|ago| ago.as_secs()),
            connections: self.connections.lock().len(),
            recent: super::recent::lines(),
        }
    }
}

/// What the service says about itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    /// The serving copy's version.
    pub version: String,
    /// When it started, in seconds since the Unix epoch.
    pub started: u64,
    /// Seconds it has run.
    pub uptime: u64,
    /// Seconds since the last key press (None: no key yet).
    pub last_key: Option<u64>,
    /// Adapters connected now.
    pub connections: usize,
    /// Its recent warnings and errors ([`super::recent::lines`]).
    pub recent: Vec<String>,
}

impl Diagnostics {
    /// The broker reply (IPC.md `diagnostics`).
    #[must_use]
    pub fn reply(&self) -> Map<String, Value> {
        let value = json!({
            "ok": true,
            "version": self.version,
            "started": self.started,
            "uptime": self.uptime,
            "lastKey": self.last_key,
            "connections": self.connections,
            "recent": self.recent,
        });
        match value {
            Value::Object(map) => map,
            _ => Map::new(),
        }
    }

    /// Reads a reply; None when it is not one (an older copy answers `diagnostics` with a failure).
    #[must_use]
    pub fn from_reply(reply: &Value) -> Option<Self> {
        reply.get("uptime")?.as_u64()?;
        Some(Self {
            version: reply.get("version").and_then(Value::as_str).unwrap_or_default().to_owned(),
            started: reply.get("started").and_then(Value::as_u64).unwrap_or_default(),
            uptime: reply.get("uptime").and_then(Value::as_u64).unwrap_or_default(),
            last_key: reply.get("lastKey").and_then(Value::as_u64),
            connections: reply
                .get("connections")
                .and_then(Value::as_u64)
                .and_then(|count| usize::try_from(count).ok())
                .unwrap_or_default(),
            recent: reply
                .get("recent")
                .and_then(Value::as_array)
                .map(|lines| lines.iter().filter_map(|line| line.as_str().map(str::to_owned)).collect())
                .unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connections_keys_and_the_reply() {
        let activity = Activity::default();
        assert_eq!(activity.diagnostics("0.7.1").last_key, None);
        activity.seen(3);
        activity.seen(3);
        activity.seen(4);
        activity.closed(4);
        activity.key();
        let facts = activity.diagnostics("0.7.1");
        assert_eq!((facts.connections, facts.last_key), (1, Some(0)));
        let read = Diagnostics::from_reply(&Value::Object(facts.reply())).unwrap();
        assert_eq!(read, facts);
        assert_eq!(Diagnostics::from_reply(&json!({ "ok": false, "error": "未知的操作" })), None);
    }
}
