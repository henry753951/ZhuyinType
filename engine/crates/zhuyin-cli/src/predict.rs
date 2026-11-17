//! `--predict N`: the next-word suggestions (聯想詞) for each line of standard input, one tab-separated line each,
//! as the composer would show them after committing that line.  With `--timing`, the latency percentiles go to
//! standard error.

use std::io::{BufRead, Write};
use std::time::Instant;

use zhuyin_core::engine::Engine;

/// Reads context lines until the end of input, prints their suggestions and exits.
pub(crate) fn run(engine: &Engine, count: usize, timing: bool) -> ! {
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    let mut times: Vec<f64> = Vec::new();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let context = line.trim_end_matches('\r');
        let start = Instant::now();
        let predictions = engine.predict_next(context, count, true);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        let words: Vec<&str> = predictions.iter().map(|p| p.text.as_str()).collect();
        let _ = writeln!(out, "{context}\t{}", words.join("\t"));
    }
    let _ = out.flush();
    if timing && !times.is_empty() {
        times.sort_by(f64::total_cmp);
        let at = |percent: usize| times[(times.len() - 1) * percent / 100];
        let _ = writeln!(
            std::io::stderr(),
            "predict: {} lines, p50 {:.3} ms, p95 {:.3} ms, p99 {:.3} ms, max {:.3} ms",
            times.len(),
            at(50),
            at(95),
            at(99),
            times[times.len() - 1]
        );
    }
    std::process::exit(0);
}
