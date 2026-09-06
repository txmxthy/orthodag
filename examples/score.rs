//! Scores every fixture, worst first.
//!
//! ```text
//! cargo run --example score
//! cargo run --example score -- --record    # the form a baseline is stored in
//! cargo run --example score -- --wide      # generated graphs and any private corpus
//! ```
//!
//! `--wide` needs the `mermaid` feature to read a corpus from disk, and quietly
//! scores nothing extra when there is none.
//!
//! The recorded form is what `tests/quality.rs` compares against. Write it with
//! `cargo run --example score -- --record > target/quality/baseline.txt`, and
//! only after a change has been looked at and accepted.

#[path = "../tests/common/mod.rs"]
mod common;

fn main() {
    let record = std::env::args().any(|arg| arg == "--record");
    let wide = std::env::args().any(|arg| arg == "--wide");

    let mut graphs: Vec<(String, orthodag::Graph)> = common::fixtures()
        .into_iter()
        .map(|(name, g)| (name.to_owned(), g))
        .collect();
    if wide {
        graphs.extend(common::generated(12));
        graphs.extend(common::corpus());
    }

    let mut scored: Vec<_> = graphs
        .into_iter()
        .map(|(name, g)| (name, orthodag::score(&g)))
        .collect();
    scored.sort_by(|a, b| (b.1.total, &a.0).cmp(&(a.1.total, &b.0)));
    let widest = scored
        .iter()
        .map(|(name, _)| name.chars().count())
        .max()
        .unwrap_or(0);

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
            "\n{} graphs, {broken} outside the vocabulary, {total} in total",
            scored.len()
        );
    }
}
