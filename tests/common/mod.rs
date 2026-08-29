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
