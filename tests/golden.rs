//! The drawings are the specification.
//!
//! Every frame here is stored beside this file. A change that moves a character
//! fails one of these and leaves a `.snap.new` next to the `.snap`: read both,
//! and accept the new one by moving it into place, quoting the frame in the
//! commit. There is no accept-everything switch on purpose.

#![allow(clippy::unwrap_used)]

use orthodag::{Graph, Node};

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
fn an_empty_graph_draws_nothing() {
    assert_eq!(orthodag::draw(&Graph::new()), "");
}
