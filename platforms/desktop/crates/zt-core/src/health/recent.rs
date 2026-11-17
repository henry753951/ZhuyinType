//! The app's own recent warnings and errors, in memory only, for 回報問題.  The app's logger hands every warning and
//! error to [`record`]; nothing is written to disk.  Log messages never carry typed text (AGENTS.md, 隱私), and the
//! report replaces the home directory with `~`.

use std::collections::VecDeque;
use std::sync::Mutex;

use chrono::{DateTime, Local};

/// How many lines are kept.
const KEPT: usize = 40;
/// Longer messages are cut (a long path or a chained error).
const LONGEST: usize = 300;

static LINES: Mutex<VecDeque<(DateTime<Local>, String)>> = Mutex::new(VecDeque::new());

/// Keeps `message` (one line).
pub fn record(message: &str) {
    let mut line: String = message.split(['\n', '\r']).map(str::trim).collect::<Vec<_>>().join(" ");
    if let Some((cut, _)) = line.char_indices().nth(LONGEST) {
        line.truncate(cut);
        line.push('…');
    }
    // A poisoned lock only means another thread panicked while logging; the lines are still usable.
    let mut lines = LINES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if lines.len() == KEPT {
        lines.pop_front();
    }
    lines.push_back((Local::now(), line));
}

/// The kept lines, oldest first, as `HH:MM:SS message`.
#[must_use]
pub fn lines() -> Vec<String> {
    let lines = LINES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    lines.iter().map(|(time, line)| format!("{} {line}", time.format("%H:%M:%S"))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_one_line_each_and_bounded() {
        record("first\nsecond");
        record(&"長".repeat(LONGEST + 10));
        for index in 0..KEPT {
            record(&format!("line {index}"));
        }
        let kept = lines();
        assert_eq!(kept.len(), KEPT);
        assert!(kept.iter().any(|line| line.ends_with(&format!("line {}", KEPT - 1))));
        assert!(kept.iter().all(|line| !line.contains('\n')));
    }

    #[test]
    fn a_long_message_is_cut() {
        record(&"字".repeat(LONGEST * 2));
        let line = lines().into_iter().rev().find(|line| line.contains('字')).unwrap();
        assert!(line.ends_with('…'));
        assert!(line.chars().count() < LONGEST + 20);
    }
}
