//! The graphs everything is measured on.
//!
//! Small enough to read, one per shape worth having an opinion about, and named
//! so a commit message can say which one moved. Shared by the tests and by
//! `examples/score.rs`, which includes this file by path.

#![allow(dead_code, clippy::unwrap_used)]

use orthodag::{Graph, Node};

/// Every fixture, in a fixed order.
pub fn fixtures() -> Vec<(&'static str, Graph)> {
    vec![
        ("chain", chain()),
        ("fan-out", fan_out()),
        ("fan-in", fan_in()),
        ("diamond", diamond()),
        ("skip", skip()),
        ("cycle", cycle()),
        ("wide", wide()),
        ("two-tags", two_tags()),
        ("ladder", ladder()),
    ]
}

/// A pseudo-random graph that is the same graph every time.
///
/// Nothing in the library is random. A generator is only here to reach shapes
/// nobody would sit down and write — several columns, heavy fan-out, skips and a
/// few cycles at once — and to reach a lot of them cheaply.
pub fn seeded(seed: u64, nodes: usize, edges: usize) -> Graph {
    let mut state = seed | 1;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as usize
    };

    let mut g = Graph::new();
    let ids: Vec<_> = (0..nodes.max(1))
        .map(|i| g.add_node(Node::new(format!("n{i}"))))
        .collect();
    for _ in 0..edges {
        let (a, b) = (next() % ids.len(), next() % ids.len());
        if a == b {
            continue;
        }
        // A tenth point backwards, so cycles are in the mix.
        let (from, to) = if next() % 10 == 0 {
            (b, a)
        } else {
            (a.min(b), a.max(b))
        };
        match next() % 3 {
            0 => g.add_edge(ids[from], ids[to]),
            n => g.add_tagged_edge(ids[from], ids[to], [format!("t{n}")]),
        };
    }
    g
}

/// A spread of generated graphs, small to large.
pub fn generated(count: usize) -> Vec<(String, Graph)> {
    (0..count)
        .map(|at| {
            let seed = 0x5EED + at as u64;
            let nodes = 6 + at * 4;
            (format!("seed-{at:02}"), seeded(seed, nodes, nodes * 2))
        })
        .collect()
}

/// Graphs from `testdata/private`, if there are any.
///
/// The directory is git-ignored and usually absent. A library like this is only
/// honest against graphs somebody actually has, and those are rarely ours to
/// publish, so every caller of this degrades to doing nothing when the corpus is
/// not there — the suite runs identically with and without it, and CI never sees
/// one.
#[cfg(feature = "mermaid")]
pub fn corpus() -> Vec<(String, Graph)> {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/private");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut paths: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "mmd"))
        .collect();
    paths.sort();

    let mut found = Vec::new();
    for path in paths {
        let (Ok(source), Some(stem)) = (
            std::fs::read_to_string(&path),
            path.file_stem().and_then(|s| s.to_str()),
        ) else {
            continue;
        };
        let Ok(read) = orthodag::io::mermaid_in::from_mermaid(&source) else {
            println!("corpus: {} does not read, skipping", path.display());
            continue;
        };
        for (at, (title, g)) in read.into_iter().enumerate() {
            let name = if title.is_empty() {
                format!("{stem}/{at}")
            } else {
                format!("{stem}/{title}")
            };
            found.push((name, g));
        }
    }
    found
}

/// Without the reader there is no way to load a corpus from disk.
#[cfg(not(feature = "mermaid"))]
pub fn corpus() -> Vec<(String, Graph)> {
    Vec::new()
}

fn build(names: &[&str], edges: &[(usize, usize)]) -> Graph {
    let mut g = Graph::new();
    let ids: Vec<_> = names.iter().map(|n| g.add_node(Node::new(*n))).collect();
    for &(a, b) in edges {
        g.add_edge(ids[a], ids[b]);
    }
    g
}

/// Three boxes in a row. The drawing with nothing to get wrong.
fn chain() -> Graph {
    build(&["in", "cat", "out"], &[(0, 1), (1, 2)])
}

/// One box into three. The trunk-and-branches case.
fn fan_out() -> Graph {
    build(
        &["source", "even", "odd", "late"],
        &[(0, 1), (0, 2), (0, 3)],
    )
}

/// Three boxes into one.
fn fan_in() -> Graph {
    build(&["even", "odd", "late", "sink"], &[(0, 3), (1, 3), (2, 3)])
}

/// A split that rejoins.
fn diamond() -> Graph {
    build(
        &["in", "even", "odd", "sink"],
        &[(0, 1), (0, 2), (1, 3), (2, 3)],
    )
}

/// A chain with an edge over the top of it.
fn skip() -> Graph {
    build(&["in", "a", "b", "out"], &[(0, 1), (1, 2), (2, 3), (0, 3)])
}

/// A loop back to an earlier box.
fn cycle() -> Graph {
    build(
        &["in", "work", "retry", "out"],
        &[(0, 1), (1, 2), (2, 1), (2, 3)],
    )
}

/// Six boxes in one column, which is where placement has the least room.
fn wide() -> Graph {
    let names = ["in", "a", "b", "c", "d", "e", "f", "out"];
    let mut edges: Vec<(usize, usize)> = (1..=6).map(|i| (0, i)).collect();
    edges.extend((1..=6).map(|i| (i, 7)));
    build(&names, &edges)
}

/// Two flows through the same boxes, told apart only by their tags.
fn two_tags() -> Graph {
    let mut g = Graph::new();
    let source = g.add_node(Node::new("source"));
    let split = g.add_node(Node::new("split").line("2 partitions"));
    let even = g.add_node(Node::new("even"));
    let odd = g.add_node(Node::new("odd"));
    g.add_edge(source, split);
    g.add_tagged_edge(split, even, ["even"]);
    g.add_tagged_edge(split, odd, ["odd"]);
    g.add_tagged_edge(source, even, ["even", "direct"]);
    g
}

/// Two parallel chains with rungs between them, so long edges have company.
fn ladder() -> Graph {
    let names = ["a0", "b0", "a1", "b1", "a2", "b2", "end"];
    build(
        &names,
        &[
            (0, 2),
            (1, 3),
            (2, 4),
            (3, 5),
            (0, 3),
            (1, 2),
            (2, 5),
            (3, 4),
            (4, 6),
            (5, 6),
            (0, 6),
        ],
    )
}
