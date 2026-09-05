//! The drawings are the specification.
//!
//! Every frame here is stored beside this file. A change that moves a character
//! fails one of these and leaves a `.snap.new` next to the `.snap`: read both,
//! and accept the new one by moving it into place, quoting the frame in the
//! commit. There is no accept-everything switch on purpose.

#![allow(clippy::unwrap_used)]

use orthodag::{Crossing, Graph, Node, Options};

fn nodes(g: &mut Graph, names: &[&str]) -> Vec<orthodag::NodeId> {
    names.iter().map(|n| g.add_node(Node::new(*n))).collect()
}

#[test]
fn a_chain_draws() {
    let mut g = Graph::new();
    let ids = nodes(&mut g, &["in", "cat", "out"]);
    g.add_edge(ids[0], ids[1]);
    g.add_edge(ids[1], ids[2]);
    insta::assert_snapshot!("chain", orthodag::draw(&g));
}

#[test]
fn a_fan_out_draws() {
    let mut g = Graph::new();
    let ids = nodes(&mut g, &["source", "even", "odd", "late"]);
    for target in &ids[1..] {
        g.add_edge(ids[0], *target);
    }
    insta::assert_snapshot!("fan_out", orthodag::draw(&g));
}

#[test]
fn a_fan_in_draws() {
    let mut g = Graph::new();
    let ids = nodes(&mut g, &["even", "odd", "late", "sink"]);
    for source in &ids[..3] {
        g.add_edge(*source, ids[3]);
    }
    insta::assert_snapshot!("fan_in", orthodag::draw(&g));
}

#[test]
fn a_diamond_with_a_skip_draws() {
    let mut g = Graph::new();
    let ids = nodes(&mut g, &["in", "split", "even", "odd"]);
    let sink = g.add_node(Node::new("sink").line("2 parts"));
    g.add_edge(ids[0], ids[1]);
    g.add_edge(ids[1], ids[2]);
    g.add_edge(ids[1], ids[3]);
    g.add_edge(ids[2], sink);
    g.add_edge(ids[3], sink);
    g.add_edge(ids[0], sink);
    insta::assert_snapshot!("diamond_with_a_skip", orthodag::draw(&g));
}

#[test]
fn a_cycle_draws_its_back_edge_in_a_lane() {
    let mut g = Graph::new();
    let ids = nodes(&mut g, &["in", "work", "retry"]);
    g.add_edge(ids[0], ids[1]);
    g.add_edge(ids[1], ids[2]);
    g.add_edge(ids[2], ids[1]);
    insta::assert_snapshot!("cycle", orthodag::draw(&g));
}

#[test]
fn a_lone_node_draws() {
    let mut g = Graph::new();
    g.add_node(Node::new("alone").line("no edges"));
    insta::assert_snapshot!("lone_node", orthodag::draw(&g));
}

#[test]
fn tags_draw_on_their_edges_when_asked_for() {
    let mut g = Graph::new();
    let ids = nodes(
        &mut g,
        &["in", "route", "even-sink", "odd-sink", "number-sink"],
    );
    g.add_edge(ids[0], ids[1]);
    g.add_tagged_edge(ids[1], ids[2], ["even-tag"]);
    g.add_tagged_edge(ids[1], ids[3], ["odd-tag"]);
    g.add_tagged_edge(ids[1], ids[4], ["even-tag", "odd-tag"]);
    insta::assert_snapshot!(
        "labels",
        orthodag::draw_with(&g, Options::new().labels(true))
    );
}

#[test]
fn labels_are_off_unless_asked_for() {
    let mut g = Graph::new();
    let ids = nodes(&mut g, &["a", "b"]);
    g.add_tagged_edge(ids[0], ids[1], ["a-tag"]);
    assert!(!orthodag::draw(&g).contains("a-tag"));
    assert!(orthodag::draw_with(&g, Options::new().labels(true)).contains("a-tag"));
}

#[test]
fn a_crossing_can_read_as_a_bridge() {
    let mut g = Graph::new();
    // Two chains with rungs between them and a skip over the top: the skip
    // forces a track that the straight edges have to cross.
    let ids = nodes(&mut g, &["a0", "b0", "a1", "b1", "a2", "b2", "end"]);
    for &(from, to) in &[
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
    ] {
        g.add_edge(ids[from], ids[to]);
    }
    insta::assert_snapshot!(
        "bridge",
        orthodag::draw_with(&g, Options::new().crossings(Crossing::Bridge))
    );
}

#[test]
fn an_empty_graph_draws_nothing() {
    assert_eq!(orthodag::draw(&Graph::new()), "");
}
