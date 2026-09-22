//! Writing a graph back out as Mermaid flowchart source.

use std::fmt::Write as _;

use crate::graph::Graph;

/// Writes a graph as a Mermaid flowchart, left to right.
///
/// Ids are positions, so the output is the same every run and says nothing
/// about where the graph came from. A node's further lines are joined with
/// `<br>`, which is what [`from_mermaid`] splits them on, so a graph survives
/// the trip out and back.
///
/// [`from_mermaid`]: super::mermaid_in::from_mermaid
pub fn to_mermaid(g: &Graph) -> String {
    let mut out = String::from("graph LR\n");

    for (at, node) in g.nodes().iter().enumerate() {
        let text = std::iter::once(node.label())
            .chain(node.lines().iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join("<br>");
        let _ = writeln!(out, "  n{at}[\"{}\"]", escape(&text));
    }

    for edge in g.edges() {
        let (from, to) = (edge.from().index(), edge.to().index());
        if edge.tags().is_empty() {
            let _ = writeln!(out, "  n{from} --> n{to}");
        } else {
            let _ = writeln!(
                out,
                "  n{from} -->|{}| n{to}",
                escape(&edge.tags().join(", "))
            );
        }
    }

    out
}

/// Mermaid has no escape for a quote inside a quoted label, so it becomes an
/// HTML entity, which Mermaid does read.
fn escape(text: &str) -> String {
    text.replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::super::mermaid_in::from_mermaid;
    use super::*;
    use crate::graph::Node;

    #[test]
    fn a_chain_writes() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("in"));
        let b = g.add_node(Node::new("out"));
        g.add_edge(a, b).unwrap();
        assert_eq!(
            to_mermaid(&g),
            "graph LR\n  n0[\"in\"]\n  n1[\"out\"]\n  n0 --> n1\n"
        );
    }

    #[test]
    fn tags_go_between_the_pipes() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        g.add_tagged_edge(a, b, ["odd", "late"]).unwrap();
        assert!(to_mermaid(&g).contains("n0 -->|late, odd| n1"));
    }

    #[test]
    fn further_lines_join_with_a_break_the_reader_understands() {
        let mut g = Graph::new();
        g.add_node(Node::new("box").line("two").line("three"));
        let written = to_mermaid(&g);
        assert!(written.contains("n0[\"box<br>two<br>three\"]"), "{written}");

        let read = from_mermaid(&written).expect("it reads back");
        assert_eq!(read[0].1.nodes()[0].lines(), ["two", "three"]);
    }

    #[test]
    fn a_quote_in_a_label_survives() {
        let mut g = Graph::new();
        g.add_node(Node::new("say \"hello\""));
        assert!(to_mermaid(&g).contains("&quot;hello&quot;"));
    }

    #[test]
    fn an_empty_graph_writes_a_header_and_nothing_else() {
        assert_eq!(to_mermaid(&Graph::new()), "graph LR\n");
    }
}
