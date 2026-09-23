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
//! The recorded form is what `tests/quality.rs` compares against. `just baseline`
//! writes the accepted public-corpus baseline after a change has been reviewed.

#[path = "../tests/common/mod.rs"]
mod common;

use std::time::Duration;

/// Where the time goes, summed over every graph.
///
/// Per graph it is noise — most of them lay out in well under a millisecond —
/// and over the set it says which phase a budget is actually spent on.
fn phases(names: &[String]) {
    let mut graphs: Vec<(String, orthodag::Graph)> = common::fixtures()
        .into_iter()
        .map(|(name, g)| (name.to_owned(), g))
        .collect();
    graphs.extend(common::generated(12));
    graphs.extend(common::corpus());
    graphs.retain(|(name, _)| names.contains(name));

    let mut sum = [Duration::ZERO; 7];
    let (mut total, mut drawings) = (Duration::ZERO, 0);
    for (_, g) in &graphs {
        let p = orthodag::phases(g);
        for (at, took) in [
            p.acyclic, p.rank, p.layer, p.order, p.place, p.route, p.score,
        ]
        .into_iter()
        .enumerate()
        {
            sum[at] += took;
        }
        total += p.total;
        drawings += p.drawings;
    }

    let labels = [
        "acyclic", "rank", "layer", "order", "place", "route", "score",
    ];
    eprintln!(
        "\nby phase, over {} graphs, {drawings} drawings made",
        graphs.len()
    );
    for (name, took) in labels.iter().zip(sum) {
        let share = if total.is_zero() {
            0.0
        } else {
            took.as_secs_f64() / total.as_secs_f64() * 100.0
        };
        eprintln!(
            "  {name:<12}{:>8.1}ms{share:>7.1}%",
            took.as_secs_f64() * 1000.0
        );
    }
    eprintln!("  {:<12}{:>8.1}ms", "total", total.as_secs_f64() * 1000.0);
}

fn main() {
    let record = std::env::args().any(|arg| arg == "--record");
    let wide = std::env::args().any(|arg| arg == "--wide");
    // `--fit N`: lay every graph out for a terminal N columns across, which is
    // the path anything with a window takes and a different cost entirely — the
    // compaction ladder is a whole layout per rung.
    let fit = std::env::args()
        .skip_while(|arg| arg != "--fit")
        .nth(1)
        .and_then(|n| n.parse::<usize>().ok());

    let mut graphs: Vec<(String, orthodag::Graph)> = common::fixtures()
        .into_iter()
        .map(|(name, g)| (name.to_owned(), g))
        .collect();
    if wide {
        graphs.extend(common::generated(12));
        graphs.extend(common::corpus());
    }

    // Recording is the machine path: its output is compared against a stored
    // baseline, and a time is different every run.
    let names: Vec<_> = graphs.iter().map(|(n, _)| n.clone()).collect();
    let mut watch = common::progress::Progress::new(graphs.len(), record);
    let mut scored: Vec<_> = graphs
        .into_iter()
        .map(|(name, g)| {
            let options = fit.map_or_else(orthodag::Options::new, |width| {
                orthodag::Options::new().width(width)
            });
            let score = watch.graph(&name, || orthodag::score_with(&g, options));
            (name, score)
        })
        .collect();
    watch.finish();

    if std::env::args().any(|arg| arg == "--phases") {
        phases(&names);
    }
    if record {
        scored.sort_by(|a, b| a.0.cmp(&b.0));
    } else {
        scored.sort_by(|a, b| (b.1.total, &a.0).cmp(&(a.1.total, &b.0)));
    }
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
