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
    // A branch carrying two tags at once is the case worth a picture: it is one
    // edge, drawn once, captioned with both.
    let ids = nodes(&mut g, &["logs", "level", "warn", "error", "audit"]);
    g.add_edge(ids[0], ids[1]);
    g.add_tagged_edge(ids[1], ids[2], ["warn"]);
    g.add_tagged_edge(ids[1], ids[3], ["error"]);
    g.add_tagged_edge(ids[1], ids[4], ["warn", "error"]);
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

/// The widest line of a drawing.
fn columns(drawn: &str) -> usize {
    drawn
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0)
}

#[test]
fn a_drawing_shrinks_to_fit_a_width() {
    let mut g = Graph::new();
    let ids = nodes(
        &mut g,
        &[
            "a-very-long-source-name",
            "an-equally-long-middle",
            "and-a-long-sink-name-too",
        ],
    );
    g.add_edge(ids[0], ids[1]);
    g.add_edge(ids[1], ids[2]);

    let natural = orthodag::draw(&g);
    assert!(columns(&natural) > 60);

    let fitted = orthodag::draw_with(&g, Options::new().width(60));
    assert!(
        columns(&fitted) <= 60,
        "asked for 60, got {}",
        columns(&fitted)
    );
    assert!(fitted.contains('…'), "text was cut, so it should say so");
    insta::assert_snapshot!("fitted", fitted);
}

#[test]
fn a_width_nothing_can_reach_gives_the_narrowest_rather_than_a_clipped_drawing() {
    let mut g = Graph::new();
    let ids = nodes(
        &mut g,
        &["a-very-long-source-name", "and-a-long-sink-name-too"],
    );
    g.add_edge(ids[0], ids[1]);

    let floor = orthodag::draw_with(&g, Options::new().width(4));
    assert!(
        columns(&floor) > 4,
        "nothing can draw two boxes in four columns"
    );
    assert_eq!(
        columns(&floor),
        columns(&orthodag::draw_with(&g, Options::new().width(1))),
        "past the floor, asking for less changes nothing"
    );
    assert!(
        floor
            .lines()
            .all(|line| line.chars().count() <= columns(&floor))
    );
}

#[test]
fn asking_for_more_room_than_it_needs_changes_nothing() {
    let mut g = Graph::new();
    let ids = nodes(&mut g, &["in", "out"]);
    g.add_edge(ids[0], ids[1]);
    assert_eq!(
        orthodag::draw(&g),
        orthodag::draw_with(&g, Options::new().width(400))
    );
}

#[test]
fn an_empty_graph_draws_nothing() {
    assert_eq!(orthodag::draw(&Graph::new()), "");
}
