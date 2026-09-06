//! A graph written as Mermaid and read back is the same graph.
//!
//! Which matters twice over. It is the only check that the reader and the
//! writer agree about the dialect, and it is what lets a corpus live on disk as
//! text rather than as Rust nobody wants to read.

#![cfg(feature = "mermaid")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use orthodag::Graph;
use orthodag::io::mermaid_in::from_mermaid;
use orthodag::io::mermaid_out::to_mermaid;

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
