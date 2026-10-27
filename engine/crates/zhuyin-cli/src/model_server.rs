//! `--model-server PATH`: the models run by `zt`'s model server (`ml/src/zhuyintype_ml/remote/model_server.py`) over a
//! private Unix socket.  A server error aborts the evaluation instead of silently dropping model scores.
//!
//! The server reads JSON lines and binary frames; this client sends binary frames, which neither side has to format
//! or parse as text.  Little-endian throughout.  A frame is an 8-byte header — marker 0, kind, version 1 (u16), payload
//! length in bytes (u32) — then the payload:
//!
//! - kind 1, reranker: i32 rows, length, `ids[rows × length]`, `targets[rows × length]`; the reply holds rows × length
//!   f32 log-probabilities (0 after a row's last target);
//! - kind 2, character model: i32 context length C, syllables N, width W, `context[C]`, `syllable_ids[N]`,
//!   `tones[N]`, `costs[N]`, `boundaries[N]`, `candidates[N × W]`; the reply holds N × W f32 log-probabilities.
//!
//! A reply of kind 1 carries an error message (UTF-8) instead.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use zhuyin_models::{ModelError, RerankerModel, SyllableInput, SyllableModel};

use crate::fail;

const RERANK: u8 = 1;
const SYLLABLE: u8 = 2;
const VERSION: u16 = 1;
const MAX_REPLY: usize = 16 << 20;

/// One connection, shared by the reranker and the character model.
pub(crate) struct ModelServer {
    stream: UnixStream,
}

impl ModelServer {
    /// Connects to the socket at `path`, with 120-second send and receive timeouts.
    pub(crate) fn connect(path: &str) -> Result<Arc<Mutex<ModelServer>>, String> {
        let stream = UnixStream::connect(path).map_err(|e| e.to_string())?;
        let timeout = Some(Duration::from_secs(120));
        stream.set_read_timeout(timeout).map_err(|e| e.to_string())?;
        stream.set_write_timeout(timeout).map_err(|e| e.to_string())?;
        Ok(Arc::new(Mutex::new(ModelServer { stream })))
    }

    /// Sends one request of `kind` and returns the reply's values.
    fn call(&mut self, kind: u8, values: &[i32]) -> Vec<f64> {
        let result = (|| -> Result<Vec<f64>, String> {
            let size = u32::try_from(values.len() * 4).map_err(|e| e.to_string())?;
            let mut frame = Vec::with_capacity(8 + values.len() * 4);
            frame.extend_from_slice(&[0, kind]);
            frame.extend_from_slice(&VERSION.to_le_bytes());
            frame.extend_from_slice(&size.to_le_bytes());
            for value in values {
                frame.extend_from_slice(&value.to_le_bytes());
            }
            self.stream.write_all(&frame).map_err(|e| e.to_string())?;
            let mut header = [0u8; 8];
            self.stream.read_exact(&mut header).map_err(|e| e.to_string())?;
            let size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
            if header[0] != 0 || u16::from_le_bytes([header[2], header[3]]) != VERSION || size > MAX_REPLY {
                return Err("Protocol error".to_owned());
            }
            let mut payload = vec![0u8; size];
            self.stream.read_exact(&mut payload).map_err(|e| e.to_string())?;
            match header[1] {
                0 if size.is_multiple_of(4) => {
                    Ok(payload.as_chunks::<4>().0.iter().map(|b| f64::from(f32::from_le_bytes(*b))).collect())
                }
                1 => fail(&format!("model server: {}", String::from_utf8_lossy(&payload))),
                _ => Err("Protocol error".to_owned()),
            }
        })();
        result.unwrap_or_else(|error| fail(&format!("model server: {error}")))
    }
}

fn int(value: usize) -> i32 {
    i32::try_from(value).unwrap_or_else(|_| fail("model server: request too large"))
}

/// The reranker through the server.
pub(crate) struct ServerReranker(pub(crate) Arc<Mutex<ModelServer>>);

impl RerankerModel for ServerReranker {
    fn logprob(&mut self, ids: &[i32], targets: &[i32], rows: usize, length: usize) -> Result<Vec<f64>, ModelError> {
        let mut values = vec![int(rows), int(length)];
        values.extend_from_slice(ids);
        values.extend_from_slice(targets);
        let output = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).call(RERANK, &values);
        if output.len() == ids.len() && output.iter().all(|v| v.is_finite()) {
            Ok(output)
        } else {
            fail("Invalid reranker response")
        }
    }
}

/// The character model through the server.
pub(crate) struct ServerSyllableModel(pub(crate) Arc<Mutex<ModelServer>>);

impl SyllableModel for ServerSyllableModel {
    fn predict(&mut self, input: &SyllableInput<'_>) -> Result<Vec<Vec<f64>>, ModelError> {
        let width = input.candidates.first().map_or(0, Vec::len);
        if width == 0 || input.candidates.iter().any(|row| row.len() != width) {
            fail("Invalid syllable request");
        }
        let mut values = vec![int(input.context_ids.len()), int(input.syllable_ids.len()), int(width)];
        for part in [input.context_ids, input.syllable_ids, input.tones, input.costs, input.boundaries] {
            values.extend_from_slice(part);
        }
        for row in input.candidates {
            values.extend_from_slice(row);
        }
        let output = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).call(SYLLABLE, &values);
        if output.len() != input.candidates.len() * width || !output.iter().all(|v| v.is_finite()) {
            fail("Invalid syllable response");
        }
        Ok(output.chunks_exact(width).map(<[f64]>::to_vec).collect())
    }
}
