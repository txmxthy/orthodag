//! Draw a graph and say what is wrong with it.
//!
//! ```text
//! cargo run --features mermaid --example why -- graph.mmd
//! cargo run --features mermaid --example why           # reads stdin
//! ```
//!
//! The frame, the numbers, and then every defect with the pair responsible,
//! grouped so a pair that shares eight cells reads as one decision gone wrong
//! rather than eight. The blame column says which phase could have prevented
//! it: a fan crossing itself is attach rows in the wrong order, two strangers
//! crossing is the column order.

use std::collections::BTreeMap;
use std::io::Read;

use orthodag::{Graph, Node};

fn main() {
    let mut source = String::new();
    match std::env::args().nth(1) {
        Some(path) => source = std::fs::read_to_string(path).unwrap_or_default(),
        None => {
            let _ = std::io::stdin().read_to_string(&mut source);
        }
    }

    let Ok(read) = orthodag::io::mermaid_in::from_mermaid(&source) else {
        eprintln!("that is not a flowchart this can read");
        return;
    };

    for (title, g) in read {
        println!("──── {title} ────");
        print!("{}", orthodag::draw(&g));
        println!("\n{}\n", orthodag::score(&g));
        report(&g);
    }
}

fn report(g: &Graph) {
    let label = |id| {
        g.edge(id).map_or_else(
            || "?".to_owned(),
            |e| {
                let name = |n| g.node(n).map_or("?", Node::label);
                format!(
                    "{} -> {} [{}]",
                    name(e.from()),
                    name(e.to()),
                    e.tags().join("+")
                )
            },
        )
    };

    let mut pairs: BTreeMap<(String, String, String), usize> = BTreeMap::new();
    for defect in orthodag::defects(g) {
        let blame = if defect.same_source(g).is_some() {
            "a fan out of one box crosses itself"
        } else if defect.same_target(g).is_some() {
            "a fan into one box crosses itself"
        } else {
            "two unrelated edges"
        };
        *pairs
            .entry((
                format!("{:?} — {blame}", defect.fault),
                label(defect.edges.0),
                label(defect.edges.1),
            ))
            .or_default() += 1;
    }

    if pairs.is_empty() {
        println!("nothing wrong with it");
        return;
    }

    let mut rows: Vec<_> = pairs.into_iter().collect();
    rows.sort_by_key(|(_, cells)| std::cmp::Reverse(*cells));
    for ((fault, a, b), cells) in rows {
        println!("{cells:>3} cells  {fault}\n            {a}\n            {b}");
    }
}
