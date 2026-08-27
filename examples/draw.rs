//! Draws a few graphs, for looking at.
//!
//! `cargo run --example draw`

use orthodag::{Graph, Node};

fn main() {
    for (name, graph) in [
        ("chain", chain()),
        ("fan-out", fan_out()),
        ("diamond", diamond()),
    ] {
        println!("{name}\n{}", orthodag::draw(&graph));
    }
}

/// Three boxes in a row.
fn chain() -> Graph {
    let mut g = Graph::new();
    let a = g.add_node(Node::new("in"));
    let b = g.add_node(Node::new("cat"));
    let c = g.add_node(Node::new("out"));
    g.add_edge(a, b);
    g.add_edge(b, c);
    g
}

/// One box into three.
fn fan_out() -> Graph {
    let mut g = Graph::new();
    let source = g.add_node(Node::new("source"));
    for name in ["even", "odd", "late"] {
        let target = g.add_node(Node::new(name));
        g.add_tagged_edge(source, target, [name]);
    }
    g
}

/// A split that rejoins, with an edge skipping the middle.
fn diamond() -> Graph {
    let mut g = Graph::new();
    let source = g.add_node(Node::new("in"));
    let split = g.add_node(Node::new("split"));
    let even = g.add_node(Node::new("even"));
    let odd = g.add_node(Node::new("odd"));
    let sink = g.add_node(Node::new("sink").line("2 parts"));
    g.add_edge(source, split);
    g.add_edge(split, even);
    g.add_edge(split, odd);
    g.add_edge(even, sink);
    g.add_edge(odd, sink);
    g.add_edge(source, sink);
    g
}
