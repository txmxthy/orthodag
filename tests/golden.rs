//! The drawings are the specification.
//!
//! Every frame here is stored beside this file. A change that moves a character
//! fails one of these and leaves a `.snap.new` next to the `.snap`: read both,
//! and accept the new one by moving it into place, quoting the frame in the
//! commit. There is no accept-everything switch on purpose.

#![allow(clippy::unwrap_used)]

use orthodag::{Crossing, Graph, Node, Options};
use unicode_width::UnicodeWidthStr;

fn nodes(g: &mut Graph, names: &[&str]) -> Vec<orthodag::NodeId> {
    names.iter().map(|n| g.add_node(Node::new(*n))).collect()
}

#[test]
fn a_chain_draws() {
    let mut g = Graph::new();
    let ids = nodes(&mut g, &["in", "cat", "out"]);
    g.add_edge(ids[0], ids[1]).unwrap();
    g.add_edge(ids[1], ids[2]).unwrap();
    insta::assert_snapshot!("chain", orthodag::draw(&g));
}

#[test]
fn a_fan_out_draws() {
    let mut g = Graph::new();
    let ids = nodes(&mut g, &["source", "even", "odd", "late"]);
    for target in &ids[1..] {
        g.add_edge(ids[0], *target).unwrap();
    }
    insta::assert_snapshot!("fan_out", orthodag::draw(&g));
}

#[test]
fn a_fan_in_draws() {
    let mut g = Graph::new();
    let ids = nodes(&mut g, &["even", "odd", "late", "sink"]);
    for source in &ids[..3] {
        g.add_edge(*source, ids[3]).unwrap();
    }
    insta::assert_snapshot!("fan_in", orthodag::draw(&g));
}

#[test]
fn a_diamond_with_a_skip_draws() {
    let mut g = Graph::new();
    let ids = nodes(&mut g, &["in", "split", "even", "odd"]);
    let sink = g.add_node(Node::new("sink").line("2 parts"));
    g.add_edge(ids[0], ids[1]).unwrap();
    g.add_edge(ids[1], ids[2]).unwrap();
    g.add_edge(ids[1], ids[3]).unwrap();
    g.add_edge(ids[2], sink).unwrap();
    g.add_edge(ids[3], sink).unwrap();
    g.add_edge(ids[0], sink).unwrap();
    insta::assert_snapshot!("diamond_with_a_skip", orthodag::draw(&g));
}

#[test]
fn a_cycle_draws_its_back_edge_in_a_lane() {
    let mut g = Graph::new();
    let ids = nodes(&mut g, &["in", "work", "retry"]);
    g.add_edge(ids[0], ids[1]).unwrap();
    g.add_edge(ids[1], ids[2]).unwrap();
    g.add_edge(ids[2], ids[1]).unwrap();
    insta::assert_snapshot!("cycle", orthodag::draw(&g));
}

#[test]
fn a_self_loop_leaves_and_returns_under_its_box() {
    let mut g = Graph::new();
    let ids = nodes(&mut g, &["enrich", "sink"]);
    g.add_edge(ids[0], ids[0]).unwrap();
    g.add_edge(ids[0], ids[1]).unwrap();
    insta::assert_snapshot!("self_loop", orthodag::draw(&g));
}

#[test]
fn a_lone_node_draws() {
    let mut g = Graph::new();
    g.add_node(Node::new("alone").line("no edges"));
    insta::assert_snapshot!("lone_node", orthodag::draw(&g));
}

#[test]
fn unicode_labels_are_measured_in_terminal_cells() {
    let mut g = Graph::new();
    let wide = g.add_node(Node::new("界界界"));
    let combining = g.add_node(Node::new("e\u{301}e\u{301}e\u{301}"));
    let emoji = g.add_node(Node::new("👩‍💻👩‍💻👩‍💻"));
    g.add_edge(wide, combining).unwrap();
    g.add_edge(combining, emoji).unwrap();

    let drawing = orthodag::layout(&g, Options::default());

    assert_eq!(drawing.boxed_at(wide).map(|b| b.rect.w), Some(10));
    assert_eq!(drawing.boxed_at(combining).map(|b| b.rect.w), Some(7));
    assert_eq!(drawing.boxed_at(emoji).map(|b| b.rect.w), Some(10));
}

#[test]
fn terminal_controls_are_replaced_before_rendering() {
    let mut g = Graph::new();
    let unsafe_node = g.add_node(Node::new(
        "a\tb\nc\u{1b}\u{85}\u{202e}\u{2066}\u{200e}\u{200b}\u{ad}\u{feff}",
    ));
    let safe_node = g.add_node(Node::new("safe"));
    g.add_tagged_edge(unsafe_node, safe_node, ["tag\t\u{202e}"])
        .unwrap();

    let drawn = orthodag::draw_with(&g, Options::new().labels(true));

    assert!(drawn.contains("a�b�c��������"), "{drawn:?}");
    assert!(drawn.contains("tag��"), "{drawn:?}");
    assert!(!drawn.contains([
        '\t', '\u{1b}', '\u{85}', '\u{202e}', '\u{2066}', '\u{200e}', '\u{200b}', '\u{ad}',
        '\u{feff}'
    ]));
}

#[test]
fn rendered_graphemes_occupy_the_cells_layout_reserved() {
    let label = "👩‍💻👩‍💻👩‍💻";
    let mut g = Graph::new();
    g.add_node(Node::new(label));

    let drawn = orthodag::draw(&g);

    assert!(drawn.contains(label), "{drawn}");
    assert!(drawn.lines().all(|line| line.width() == 10), "{drawn}");
}

#[test]
fn clipping_keeps_only_whole_graphemes() {
    let mut g = Graph::new();
    let emoji = g.add_node(Node::new("👩‍💻x"));
    let combining = g.add_node(Node::new("e\u{301}xy"));
    g.add_edge(emoji, combining).unwrap();

    let drawn = orthodag::draw_with(&g, Options::new().box_width(6));

    assert!(!drawn.contains(['👩', '💻']), "{drawn}");
    assert!(drawn.contains("e\u{301}…"), "{drawn}");
}

#[test]
fn tags_draw_on_their_edges_when_asked_for() {
    let mut g = Graph::new();
    // A branch carrying two tags at once is the case worth a picture: it is one
    // edge, drawn once, captioned with both.
    let ids = nodes(&mut g, &["logs", "level", "warn", "error", "audit"]);
    g.add_edge(ids[0], ids[1]).unwrap();
    g.add_tagged_edge(ids[1], ids[2], ["warn"]).unwrap();
    g.add_tagged_edge(ids[1], ids[3], ["error"]).unwrap();
    g.add_tagged_edge(ids[1], ids[4], ["warn", "error"])
        .unwrap();
    insta::assert_snapshot!(
        "labels",
        orthodag::draw_with(&g, Options::new().labels(true))
    );
}

#[test]
fn labels_are_off_unless_asked_for() {
    let mut g = Graph::new();
    let ids = nodes(&mut g, &["a", "b"]);
    g.add_tagged_edge(ids[0], ids[1], ["a-tag"]).unwrap();
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
        g.add_edge(ids[from], ids[to]).unwrap();
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
    g.add_edge(ids[0], ids[1]).unwrap();
    g.add_edge(ids[1], ids[2]).unwrap();

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
    g.add_edge(ids[0], ids[1]).unwrap();

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
    g.add_edge(ids[0], ids[1]).unwrap();
    assert_eq!(
        orthodag::draw(&g),
        orthodag::draw_with(&g, Options::new().width(400))
    );
}

#[test]
fn an_empty_graph_draws_nothing() {
    assert_eq!(orthodag::draw(&Graph::new()), "");
}
