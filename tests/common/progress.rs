//! How long each graph took, said while it is happening.
//!
//! Laying a graph out is a search, so the cost is not proportional to anything
//! a reader can see: a graph twice the size can be fifty times the work, and the
//! only way to know which one is expensive is to time each one. Waiting several
//! minutes for a page of drawings with nothing on the terminal is also
//! unpleasant, and the same measurement fixes both.
//!
//! Everything here goes to **stderr**, so a caller can still pipe the real
//! output somewhere without this in it. There is no carriage return and nothing
//! is rewritten in place: one line per graph, which survives being captured to a
//! file, and which is what a bar redrawing itself does not.
//!
//! Times are wall clock and therefore not reproducible, so nothing here may
//! reach a recorded baseline — see `docs/quality.md`, where the ratchet compares
//! stored numbers and would fail on the noise.

use std::fmt::Write as _;
use std::io::Write as _;
use std::time::{Duration, Instant};

/// A run of graphs, counted and timed.
pub struct Progress {
    total: usize,
    at: usize,
    quiet: bool,
    started: Instant,
    times: Vec<(String, Duration)>,
}

/// How wide the bar is drawn, in cells.
const BAR: usize = 24;

impl Progress {
    /// A run of `total` graphs. Quiet prints nothing and still keeps the times.
    pub fn new(total: usize, quiet: bool) -> Self {
        Self {
            total,
            at: 0,
            quiet,
            started: Instant::now(),
            times: Vec::new(),
        }
    }

    /// Runs one graph's work, times it, and says so.
    pub fn graph<T>(&mut self, name: &str, work: impl FnOnce() -> T) -> T {
        let began = Instant::now();
        let out = work();
        let took = began.elapsed();

        self.at += 1;
        self.times.push((name.to_owned(), took));
        if !self.quiet {
            let mut line = String::new();
            let filled = self.at * BAR / self.total.max(1);
            let _ = write!(
                line,
                "[{:>3}/{}] {}{} {:<28} {:>7}",
                self.at,
                self.total,
                "█".repeat(filled),
                "·".repeat(BAR.saturating_sub(filled)),
                clip(name, 28),
                ms(took),
            );
            let _ = writeln!(std::io::stderr(), "{line}");
        }
        out
    }

    /// The total, and the graphs worth knowing about.
    ///
    /// A mean over a set where one graph is a chain and another is fifty nodes
    /// across a dozen columns says nothing, so this reports the median for the
    /// shape of the run and the slowest few for where the time actually went.
    pub fn finish(mut self) {
        if self.quiet || self.times.is_empty() {
            return;
        }
        let wall = self.started.elapsed();
        self.times.sort_by_key(|(_, took)| *took);

        let median = self
            .times
            .get(self.times.len() / 2)
            .map_or(Duration::ZERO, |(_, took)| *took);
        let spent: Duration = self.times.iter().map(|(_, took)| *took).sum();

        let mut out = std::io::stderr();
        let _ = writeln!(
            out,
            "\n{} graphs in {}, {} laying them out, median {}",
            self.times.len(),
            ms(wall),
            ms(spent),
            ms(median),
        );
        for (name, took) in self.times.iter().rev().take(5) {
            let _ = writeln!(out, "  {:<28} {:>7}", clip(name, 28), ms(*took));
        }
    }
}

/// Milliseconds, or seconds once it is long enough that milliseconds are noise.
fn ms(took: Duration) -> String {
    if took.as_secs() >= 10 {
        format!("{:.1}s", took.as_secs_f64())
    } else {
        format!("{}ms", took.as_millis())
    }
}

fn clip(text: &str, room: usize) -> String {
    if text.chars().count() <= room {
        return text.to_owned();
    }
    text.chars()
        .take(room.saturating_sub(1))
        .chain(std::iter::once('…'))
        .collect()
}
