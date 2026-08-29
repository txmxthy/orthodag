//! Scores every fixture, worst first.
//!
//! ```text
//! cargo run --example score
//! cargo run --example score -- --record    # the form a baseline is stored in
//! ```
//!
//! The recorded form is what `tests/quality.rs` compares against. Write it with
//! `cargo run --example score -- --record > target/quality/baseline.txt`, and
//! only after a change has been looked at and accepted.

#[path = "../tests/common/mod.rs"]
mod common;

fn main() {
    let record = std::env::args().any(|arg| arg == "--record");

    let mut scored: Vec<_> = common::fixtures()
        .into_iter()
        .map(|(name, g)| (name, orthodag::score(&g)))
        .collect();
    scored.sort_by(|a, b| b.1.total.cmp(&a.1.total).then(a.0.cmp(b.0)));

    let widest = scored.iter().map(|(name, _)| name.len()).max().unwrap_or(0);
    for (name, score) in &scored {
        if record {
            println!("{name} {}", score.record());
        } else {
            println!("{name:widest$}  {score}");
        }
    }

    if !record {
        let broken = scored
            .iter()
            .filter(|(_, s)| s.vocabulary() != [0; 4])
            .count();
        let total: i64 = scored.iter().map(|(_, s)| s.total).sum();
        println!(
            "\n{} fixtures, {broken} outside the vocabulary, {total} in total",
            scored.len()
        );
    }
}
