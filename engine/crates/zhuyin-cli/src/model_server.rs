//! `--model-server PATH`: the models run by `zt`'s model server over a private JSON-lines Unix socket (Swift's
//! `ModelServer`).  A server error aborts the evaluation instead of silently dropping model scores.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use zhuyin_models::{ModelError, RerankerModel, SyllableInput, SyllableModel};

use crate::fail;

/// One connection, shared by the reranker and the character model.
pub(crate) struct ModelServer {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl ModelServer {
    /// Connects to the socket at `path`, with Swift's 120-second send and receive timeouts.
    pub(crate) fn connect(path: &str) -> Result<Arc<Mutex<ModelServer>>, String> {
        let stream = UnixStream::connect(path).map_err(|e| e.to_string())?;
        let timeout = Some(Duration::from_secs(120));
        stream.set_read_timeout(timeout).map_err(|e| e.to_string())?;
        stream.set_write_timeout(timeout).map_err(|e| e.to_string())?;
        let writer = stream.try_clone().map_err(|e| e.to_string())?;
        Ok(Arc::new(Mutex::new(ModelServer { reader: BufReader::new(stream), writer })))
    }

    fn predict(&mut self, request: &Value) -> Value {
        let result = (|| -> Result<Value, String> {
            let mut data = serde_json::to_vec(request).map_err(|e| e.to_string())?;
            data.push(b'\n');
            self.writer.write_all(&data).map_err(|e| e.to_string())?;
            let mut line = Vec::new();
            let read = Read::by_ref(&mut self.reader)
                .take(4_194_305)
                .read_until(b'\n', &mut line)
                .map_err(|e| e.to_string())?;
            if read == 0 {
                return Err("Broken pipe".to_owned());
            }
            if line.last() != Some(&b'\n') {
                return Err("Message too long".to_owned());
            }
            let mut response: Value = serde_json::from_slice(&line).map_err(|e| e.to_string())?;
            if let Some(error) = response.get("error").and_then(Value::as_str) {
                fail(&format!("model server: {error}"));
            }
            response.get_mut("output").map(Value::take).ok_or_else(|| "Protocol error".to_owned())
        })();
        result.unwrap_or_else(|error| fail(&format!("model server: {error}")))
    }
}

fn doubles(value: &Value) -> Option<Vec<f64>> {
    value.as_array()?.iter().map(Value::as_f64).collect()
}

/// The reranker through the server.
pub(crate) struct ServerReranker(pub(crate) Arc<Mutex<ModelServer>>);

impl RerankerModel for ServerReranker {
    fn logprob(&mut self, ids: &[i32], targets: &[i32], rows: usize, length: usize) -> Result<Vec<f64>, ModelError> {
        let request = json!({"op": "rerank", "ids": ids, "targets": targets, "batch": rows, "length": length});
        let output = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).predict(&request);
        match doubles(&output) {
            Some(values) if values.len() == ids.len() && values.iter().all(|v| v.is_finite()) => Ok(values),
            _ => fail("Invalid reranker response"),
        }
    }
}

/// The character model through the server.
pub(crate) struct ServerSyllableModel(pub(crate) Arc<Mutex<ModelServer>>);

impl SyllableModel for ServerSyllableModel {
    fn predict(&mut self, input: &SyllableInput<'_>) -> Result<Vec<Vec<f64>>, ModelError> {
        let request = json!({
            "op": "syllable", "context": input.context_ids, "ids": input.syllable_ids, "tones": input.tones,
            "costs": input.costs, "boundary": input.boundaries, "candidates": input.candidates,
        });
        let output = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).predict(&request);
        let rows: Option<Vec<Vec<f64>>> = output.as_array().and_then(|rows| rows.iter().map(doubles).collect());
        match rows {
            Some(rows)
                if rows.len() == input.candidates.len()
                    && rows
                        .iter()
                        .zip(input.candidates)
                        .all(|(row, list)| row.len() == list.len() && row.iter().all(|v| v.is_finite())) =>
            {
                Ok(rows)
            }
            _ => fail("Invalid syllable response"),
        }
    }
}
