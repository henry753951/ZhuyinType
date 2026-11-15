//! The broker (`IPC.md`): newline-delimited JSON between the native adapters (Fcitx5 addon, TSF text service) and
//! this process, over a per-user Unix socket (Linux) or named pipe (Windows).  Each connection has a thread of its
//! own; requests are answered in order, one reply per request, and no message is sent unasked.  A malformed frame,
//! a frame over 256 KiB or a request id that does not increase closes the connection.

pub mod host;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;

use std::io::{BufRead as _, BufReader, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use anyhow::Result;
use serde_json::{Map, Value};

pub use host::ConnectionId;

/// The largest frame, newline included.
pub const MAX_FRAME: usize = 256 * 1024;
/// The largest request id (JSON numbers are exact up to here).
const MAX_ID: f64 = 9_007_199_254_740_991.0;

/// What serves the requests.
pub trait Handler: Send + Sync + 'static {
    /// One request; the reply without `v`, `id` and `context` (the broker adds them).
    fn request(&self, connection: ConnectionId, request: &Map<String, Value>) -> Map<String, Value>;
    /// The connection closed.
    fn closed(&self, connection: ConnectionId);
}

/// The endpoint for this user: `ZT_IME_SOCKET` when set; Linux `$XDG_RUNTIME_DIR/zhuyintype/ime-v1` (or
/// `/tmp/zhuyintype-<uid>/ime-v1` when that is unset or not absolute); Windows `\\.\pipe\ZhuyinType-ime-v1-<SID>`.
#[must_use]
pub fn endpoint() -> Option<String> {
    if let Some(value) = std::env::var_os("ZT_IME_SOCKET").filter(|v| !v.is_empty()) {
        return value.into_string().ok();
    }
    platform::default_endpoint()
}

/// Why the endpoint could not be reserved.
#[derive(Debug)]
pub enum ReserveError {
    /// Another instance serves it.
    AlreadyRunning,
    /// Anything else, in the user's words.
    Failed(anyhow::Error),
}

impl std::fmt::Display for ReserveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRunning => f.write_str("輸入法服務已在執行。"),
            Self::Failed(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ReserveError {}

/// The endpoint, reserved for this process (the single-instance lock is held) but not yet accepting connections:
/// the service loads the engine first, so the first key never reaches an engine that is not ready.
pub struct Reservation {
    endpoint: String,
    inner: platform::Reservation,
}

impl Reservation {
    /// The endpoint.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Starts accepting connections; they are served by `handler` until the returned broker is dropped.
    ///
    /// # Errors
    /// When the endpoint cannot be created.
    pub fn listen(self, handler: Arc<dyn Handler>) -> Result<Broker> {
        let stop = Arc::new(AtomicBool::new(false));
        let inner = self.inner.listen(handler, stop.clone())?;
        Ok(Broker { endpoint: self.endpoint, stop, _inner: inner })
    }
}

/// Reserves `endpoint` (the single-instance lock, the directory checks).
///
/// # Errors
/// [`ReserveError::AlreadyRunning`] when another instance serves it; otherwise why it cannot be used.
pub fn reserve(endpoint: &str) -> std::result::Result<Reservation, ReserveError> {
    let inner = platform::Reservation::new(endpoint)?;
    Ok(Reservation { endpoint: endpoint.to_owned(), inner })
}

/// The running broker.  Dropping it stops accepting connections.
pub struct Broker {
    endpoint: String,
    stop: Arc<AtomicBool>,
    _inner: platform::Listener,
}

impl Broker {
    /// The endpoint served.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

impl Drop for Broker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accepting thread so it sees the flag.
        let _ = platform::connect(&self.endpoint);
    }
}

static NEXT_CONNECTION: AtomicU64 = AtomicU64::new(1);

/// Serves one connection until it closes or breaks the protocol.
pub(crate) fn serve(handler: &Arc<dyn Handler>, reader: impl Read, mut writer: impl Write) {
    let connection = NEXT_CONNECTION.fetch_add(1, Ordering::Relaxed);
    let mut reader = BufReader::with_capacity(64 * 1024, reader);
    let mut last_id = 0.0;
    loop {
        let mut line = Vec::new();
        match (&mut reader).take(MAX_FRAME as u64 + 1).read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if line.last() != Some(&b'\n') || line.len() > MAX_FRAME {
            break;
        }
        let Ok(Value::Object(request)) = serde_json::from_slice::<Value>(&line) else { break };
        let Some(id) = request.get("id").and_then(Value::as_f64).filter(|id| {
            id.is_finite() && id.fract() == 0.0 && (1.0..=MAX_ID).contains(id) && *id > last_id
        }) else {
            break;
        };
        last_id = id;
        let mut reply = handler.request(connection, &request);
        reply.insert("v".into(), 1.into());
        reply.insert("id".into(), request["id"].clone());
        if let Some(context) = request.get("context") {
            reply.insert("context".into(), context.clone());
        }
        let mut output = Value::Object(reply).to_string();
        output.push('\n');
        if output.len() > MAX_FRAME || writer.write_all(output.as_bytes()).and_then(|()| writer.flush()).is_err() {
            break;
        }
    }
    handler.closed(connection);
}

/// A client of a broker (another start of the app asking for the settings window; tests).
pub struct Client {
    reader: BufReader<platform::Stream>,
    writer: platform::Stream,
    next_id: u64,
}

impl Client {
    /// Connects to `endpoint`.
    ///
    /// # Errors
    /// When nothing serves it.
    pub fn connect(endpoint: &str) -> std::io::Result<Self> {
        let stream = platform::connect(endpoint)?;
        let writer = platform::clone_stream(&stream)?;
        Ok(Self { reader: BufReader::new(stream), writer, next_id: 0 })
    }

    /// Sends `op` with `fields` (and `v`, the next `id`, and `context` unless given) and reads the reply.
    ///
    /// # Errors
    /// When the connection breaks or the reply is not JSON.
    pub fn request(&mut self, op: &str, fields: Value) -> std::io::Result<Value> {
        self.next_id += 1;
        let mut request = match fields {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        request.insert("v".into(), 1.into());
        request.insert("id".into(), self.next_id.into());
        request.insert("op".into(), op.into());
        request.entry("context").or_insert_with(|| "field".into());
        self.send_raw(&Value::Object(request).to_string())?;
        self.read_reply()
    }

    /// Writes `line` and a newline as they are (tests of malformed frames).
    ///
    /// # Errors
    /// When the connection breaks.
    pub fn send_raw(&mut self, line: &str) -> std::io::Result<()> {
        self.writer.write_all(line.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()
    }

    /// Reads one reply.
    ///
    /// # Errors
    /// When the connection closed or the reply is not JSON.
    pub fn read_reply(&mut self) -> std::io::Result<Value> {
        let mut line = String::new();
        if self.reader.read_line(&mut line)? == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        serde_json::from_str(&line).map_err(std::io::Error::other)
    }
}

/// Asks the instance serving `endpoint` to show its settings window.  False when no instance answers.
#[must_use]
pub fn show_settings(endpoint: &str) -> bool {
    let Ok(mut client) = Client::connect(endpoint) else { return false };
    client.request("show-settings", serde_json::json!({"context": "settings"})).is_ok()
}
