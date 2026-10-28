//! Background persistence for personal data.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::personal::PersonalLanguageModel;

/// The texts of the personal data files.
pub(super) struct Snapshot {
    pub(super) phrases: String,
    pub(super) learned: String,
    pub(super) excluded: String,
    pub(super) typos: String,
    pub(super) personal: String,
}

enum Job {
    Write(PathBuf, Snapshot),
    Flush(mpsc::Sender<std::io::Result<()>>),
}

/// Writes personal data on its own thread, 150 ms after the last change.
pub(super) struct Writer {
    sender: Option<mpsc::Sender<Job>>,
    thread: Option<JoinHandle<()>>,
}

impl Writer {
    pub(super) fn new() -> Self {
        let (sender, receiver) = mpsc::channel::<Job>();
        let thread = std::thread::Builder::new()
            .name("zhuyin-personal-data".into())
            .spawn(move || {
                let mut pending: Option<(PathBuf, Snapshot)> = None;
                let mut deadline: Option<Instant> = None;
                loop {
                    let job = if let Some(at) = deadline {
                        match receiver.recv_timeout(at.saturating_duration_since(Instant::now())) {
                            Ok(job) => Some(job),
                            Err(mpsc::RecvTimeoutError::Timeout) => None,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    } else {
                        match receiver.recv() {
                            Ok(job) => Some(job),
                            Err(_) => break,
                        }
                    };
                    match job {
                        Some(Job::Write(directory, snapshot)) => {
                            pending = Some((directory, snapshot));
                            deadline = Some(Instant::now() + Duration::from_millis(150));
                        }
                        Some(Job::Flush(done)) => {
                            let result = pending.as_ref().map_or(Ok(()), |(dir, data)| write_files(dir, data));
                            if result.is_ok() {
                                pending = None;
                            }
                            deadline = None;
                            let _ = done.send(result);
                        }
                        None => {
                            if let Some((dir, data)) = &pending {
                                match write_files(dir, data) {
                                    Ok(()) => pending = None,
                                    Err(error) => eprintln!("ZhuyinCore: could not save personal data: {error}"),
                                }
                            }
                            deadline = None;
                        }
                    }
                }
                if let Some((dir, data)) = &pending
                    && let Err(error) = write_files(dir, data)
                {
                    eprintln!("ZhuyinCore: could not save personal data: {error}");
                }
            })
            .expect("personal data thread");
        Self { sender: Some(sender), thread: Some(thread) }
    }

    pub(super) fn write(&self, directory: PathBuf, snapshot: Snapshot) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(Job::Write(directory, snapshot));
        }
    }

    pub(super) fn flush(&self) -> std::io::Result<()> {
        let (done, wait) = mpsc::channel();
        self.sender
            .as_ref()
            .ok_or_else(|| std::io::Error::other("personal writer stopped"))?
            .send(Job::Flush(done))
            .map_err(|_| std::io::Error::other("personal writer stopped"))?;
        wait.recv().map_err(|_| std::io::Error::other("personal writer stopped"))?
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.sender = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Replaces `path` with `contents` atomically (a temporary file renamed over it).
fn write_atomically(path: &Path, contents: &str) -> std::io::Result<()> {
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temporary, contents)?;
    std::fs::rename(&temporary, path)
}

fn write_files(directory: &Path, snapshot: &Snapshot) -> std::io::Result<()> {
    std::fs::create_dir_all(directory)?;
    write_atomically(&directory.join("user_phrases.tsv"), &snapshot.phrases)?;
    write_atomically(&directory.join("learned.tsv"), &snapshot.learned)?;
    write_atomically(&directory.join("excluded.tsv"), &snapshot.excluded)?;
    write_atomically(&directory.join("typos.tsv"), &snapshot.typos)?;
    write_atomically(&directory.join(PersonalLanguageModel::FILE_NAME), &snapshot.personal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_flush_can_be_retried() {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/personal-writer-retry-test");
        let _ = std::fs::remove_dir_all(&directory);
        let _ = std::fs::remove_file(&directory);
        std::fs::write(&directory, "block directory").unwrap();
        let writer = Writer::new();
        writer.write(
            directory.clone(),
            Snapshot {
                phrases: "phrase".into(),
                learned: String::new(),
                excluded: String::new(),
                typos: String::new(),
                personal: String::new(),
            },
        );
        assert!(writer.flush().is_err());
        std::fs::remove_file(&directory).unwrap();
        writer.flush().unwrap();
        assert_eq!(std::fs::read_to_string(directory.join("user_phrases.tsv")).unwrap(), "phrase");
        drop(writer);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
