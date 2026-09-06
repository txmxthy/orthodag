//! Writing a graph out as Graphviz DOT.
//!
//! For handing a graph to something that lays out better than this does — a
//! poster, a paper, anything with pixels. Nothing reads DOT back in: this
//! library is not in the business of parsing, and DOT is a language rather than
//! a format.

use std::fmt::Write as _;

use crate::graph::Graph;

/// Writes a graph as a DOT digraph, left to right.
pub fn to_dot(g: &Graph) -> String {
    let mut out = String::from("digraph {\n  rankdir=LR;\n  node [shape=box];\n");

    for (at, node) in g.nodes().iter().enumerate() {
        // Escape each line before joining, or the escaper doubles the
        // backslash in the break and DOT prints a literal \n.
        let text = std::iter::once(node.label())
            .chain(node.lines().iter().map(String::as_str))
            .map(escape)
            .collect::<Vec<_>>()
            .join("\\n");
        let _ = writeln!(out, "  n{at} [label=\"{text}\"];");
    }

    for edge in g.edges() {
        let (from, to) = (edge.from().index(), edge.to().index());
        if edge.tags().is_empty() {
            let _ = writeln!(out, "  n{from} -> n{to};");
        } else {
            let _ = writeln!(
                out,
                "  n{from} -> n{to} [label=\"{}\"];",
                escape(&edge.tags().join(", "))
            );
        }
    }

    out.push_str("}\n");
    out
}

/// A quote or a backslash inside a DOT string is escaped with a backslash.
fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Node;

    #[test]
    fn a_chain_writes() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("in"));
        let b = g.add_node(Node::new("out"));
        g.add_edge(a, b);
        let written = to_dot(&g);
        assert!(written.starts_with("digraph {\n  rankdir=LR;"), "{written}");
        assert!(written.contains("n0 [label=\"in\"];"));
        assert!(written.contains("n0 -> n1;"));
        assert!(written.ends_with("}\n"));
    }

    #[test]
    fn tags_become_an_edge_label() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        g.add_tagged_edge(a, b, ["odd", "late"]);
        assert!(to_dot(&g).contains("n0 -> n1 [label=\"late, odd\"];"));
    }

    #[test]
    fn a_quote_or_a_backslash_is_escaped() {
        let mut g = Graph::new();
        g.add_node(Node::new("say \"hi\" \\ so"));
        assert!(to_dot(&g).contains(r#"label="say \"hi\" \\ so""#));
    }

    #[test]
    fn further_lines_become_line_breaks() {
        let mut g = Graph::new();
        g.add_node(Node::new("box").line("two"));
        // DOT's own line break, which is a backslash and an n in the string.
        let written = to_dot(&g);
        assert!(written.contains(r#"label="box\ntwo""#), "{written}");
    }
}
