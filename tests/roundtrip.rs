//! A graph written as Mermaid and read back is the same graph.
//!
//! Which matters twice over. It is the only check that the reader and the
//! writer agree about the dialect, and it is what lets a corpus live on disk as
//! text rather than as Rust nobody wants to read.

#![cfg(feature = "mermaid")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use orthodag::io::mermaid_in::from_mermaid;
use orthodag::io::mermaid_out::to_mermaid;
use orthodag::{Graph, Node};

/// The single graph in a document, which is what the writer always produces.
fn read(source: &str) -> Graph {
    let read = from_mermaid(source).expect("the writer's own output reads");
    assert_eq!(read.len(), 1, "one graph in, one graph out");
    read.into_iter().next().unwrap().1
}

#[test]
fn every_fixture_survives_the_trip_out_and_back() {
    for (name, g) in common::fixtures() {
        let there_and_back = read(&to_mermaid(&g));
        assert_eq!(there_and_back, g, "{name} changed on the way");
    }
}

#[test]
fn a_second_trip_changes_nothing() {
    for (name, g) in common::fixtures() {
        let once = to_mermaid(&g);
        assert_eq!(
            to_mermaid(&read(&once)),
            once,
            "{name} is not settled after one trip"
        );
    }
}

#[test]
fn labels_and_tags_with_mermaid_syntax_survive_the_trip() {
    let mut graph = Graph::new();
    let source = graph
        .add_node(Node::new("say \"R&D\" <br> [α](β){γ} > now").line("line & <literal> \"終\""));
    let sink = graph.add_node(Node::new("出口 | --> [](){}"));
    graph
        .add_tagged_edge(source, sink, ["x & y", "say \"hi\"", "<br>"])
        .unwrap();

    let written = to_mermaid(&graph);
    assert!(
        written.contains("&quot;R&amp;D&quot; &lt;br&gt;"),
        "{written}"
    );
    assert!(
        written.contains("&lt;literal&gt; &quot;終&quot;"),
        "{written}"
    );
    assert_eq!(read(&written), graph);
}

#[test]
fn the_drawing_survives_too() {
    for (name, g) in common::fixtures() {
        assert_eq!(
            orthodag::draw(&read(&to_mermaid(&g))),
            orthodag::draw(&g),
            "{name}"
        );
    }
}

#[test]
fn the_corpus_on_disk_is_the_corpus_in_code() {
    // testdata/graphs is written from the fixtures. If they drift apart, the
    // files stop being a fair thing to score against.
    for (name, g) in common::fixtures() {
        let path = format!("{}/testdata/graphs/{name}.mmd", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("{path} is missing; regenerate the corpus"));
        assert_eq!(read(&source), g, "{name} on disk is not {name} in code");
    }
}
