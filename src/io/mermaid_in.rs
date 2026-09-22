//! Reading a graph out of Mermaid flowchart source.
//!
//! Not a Mermaid implementation. It reads the subset that describes a directed
//! graph — nodes, edges, tags on edges, subgraphs — and refuses everything else
//! rather than guessing. A parser that silently ignores what it does not
//! understand produces a drawing that is quietly missing half the graph, which
//! is the failure this library can least afford.
//!
//! Node shapes are read and dropped. A rounded box and a doubled box mean
//! something to the tool that wrote them, and nothing here: this draws a box.

use std::fmt;

use crate::graph::{Graph, GraphError, Node, NodeId};

/// Why a document could not be read.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ImportError {
    /// The document does not start with `graph` or `flowchart`.
    NotAFlowchart,
    /// A line that is not in the subset, with the line number and the text.
    Unreadable(usize, String),
    /// A `subgraph` with no matching `end`, or an `end` with no `subgraph`.
    Unbalanced(usize),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAFlowchart => write!(f, "not a flowchart"),
            Self::Unreadable(line, text) => write!(f, "line {line}: cannot read {text:?}"),
            Self::Unbalanced(line) => write!(f, "line {line}: subgraph and end do not match"),
        }
    }
}

impl std::error::Error for ImportError {}

/// Reads every graph in a Mermaid flowchart.
///
/// One graph per `subgraph`, named by its label; a document with no subgraphs is
/// one graph named by the empty string. Edges that cross a subgraph boundary are
/// dropped, because the two ends are in different graphs and an edge to a node
/// that is not there cannot be drawn.
///
/// # Errors
///
/// [`ImportError`] if the document is not a flowchart, has a line outside the
/// subset, or has a `subgraph` and `end` that do not match up.
pub fn from_mermaid(source: &str) -> Result<Vec<(String, Graph)>, ImportError> {
    let mut lines = source.lines().enumerate().filter_map(|(at, line)| {
        let line = line.trim();
        let line = line
            .split_once("%%")
            .map_or(line, |(before, _)| before.trim());
        (!line.is_empty()).then_some((at + 1, line))
    });

    let Some((_, header)) = lines.next() else {
        return Err(ImportError::NotAFlowchart);
    };
    if !(header.starts_with("graph") || header.starts_with("flowchart")) {
        return Err(ImportError::NotAFlowchart);
    }

    let mut sheets: Vec<Sheet> = vec![Sheet::new(String::new())];
    let mut open: Option<usize> = None;

    for (at, line) in lines {
        if is_noise(line) {
            continue;
        }
        if let Some(rest) = line.strip_prefix("subgraph ") {
            if open.is_some() {
                return Err(ImportError::Unbalanced(at));
            }
            sheets.push(Sheet::new(title(rest.trim())));
            open = Some(sheets.len() - 1);
            continue;
        }
        if line == "end" {
            open.take().ok_or(ImportError::Unbalanced(at))?;
            continue;
        }

        let Some(sheet) = sheets.get_mut(open.unwrap_or(0)) else {
            continue;
        };
        read(sheet, line).ok_or_else(|| ImportError::Unreadable(at, line.to_owned()))?;
    }

    if open.is_some() {
        return Err(ImportError::Unbalanced(source.lines().count()));
    }

    Ok(sheets
        .into_iter()
        .filter(|sheet| !sheet.graph.nodes().is_empty())
        .map(|sheet| (sheet.title, sheet.graph))
        .collect())
}

/// One graph under construction, and the ids of the nodes seen so far.
struct Sheet {
    title: String,
    graph: Graph,
    named: Vec<(String, NodeId)>,
}

impl Sheet {
    fn new(title: String) -> Self {
        Self {
            title,
            graph: Graph::new(),
            named: Vec::new(),
        }
    }

    /// The node with this id, added with `label` if it is new.
    ///
    /// Mermaid lets a node be introduced by an edge and labelled later, so a
    /// later label replaces a placeholder one.
    fn node(&mut self, id: &str, label: Option<&str>) -> Result<NodeId, GraphError> {
        if let Some((_, held)) = self.named.iter().find(|(name, _)| name == id) {
            let held = *held;
            if let Some(label) = label {
                self.graph.relabel(held, label)?;
            }
            return Ok(held);
        }
        // A label written with breaks in it comes back as a headline and the
        // lines under it, which is how it went out.
        let text = label.unwrap_or(id);
        let mut parts = text.split("<br>").map(str::trim);
        let mut node = Node::new(parts.next().unwrap_or(text));
        for line in parts {
            node = node.line(line);
        }
        let node = self.graph.add_node(node);
        self.named.push((id.to_owned(), node));
        Ok(node)
    }
}

/// Lines that are Mermaid but say nothing about the graph.
fn is_noise(line: &str) -> bool {
    ["style ", "classDef ", "class ", "linkStyle ", "direction "]
        .iter()
        .any(|prefix| line.starts_with(prefix))
}

/// The text of a `subgraph` header: its label if it has one, else its id.
fn title(rest: &str) -> String {
    label_of(rest).map_or_else(|| rest.to_owned(), str::to_owned)
}

/// The quoted or bracketed text of a declaration, if there is any.
fn label_of(text: &str) -> Option<&str> {
    let open = text.find(['[', '(', '{'])?;
    let close = text.rfind([']', ')', '}'])?;
    let inner = text.get(open..=close)?;
    let inner = inner.trim_matches(['[', ']', '(', ')', '{', '}']);
    Some(inner.trim().trim_matches('"'))
}

/// Reads one line into a sheet, or says it could not.
fn read(sheet: &mut Sheet, line: &str) -> Option<()> {
    if let Some((from, arrow, to)) = split_edge(line) {
        let (from_id, from_label) = declaration(from);
        let (to_id, to_label) = declaration(to);
        let source = sheet.node(from_id, from_label).ok()?;
        let target = sheet.node(to_id, to_label).ok()?;
        sheet
            .graph
            .add_tagged_edge(source, target, tags(arrow))
            .ok()?;
        return Some(());
    }

    // A bare declaration: an id, optionally with a shape and a label.
    let (id, label) = declaration(line);
    if id.is_empty() || id.contains(char::is_whitespace) {
        return None;
    }
    sheet.node(id, label).ok()?;
    Some(())
}

/// Splits `a -->|tag| b` into its three parts.
fn split_edge(line: &str) -> Option<(&str, &str, &str)> {
    // Longest first, so `-.->` is not read as `-` then `.->`.
    let arrows = ["-.->", "==>", "-->", "--x", "--o"];
    let (at, arrow) = arrows
        .iter()
        .filter_map(|arrow| line.find(arrow).map(|at| (at, *arrow)))
        .min_by_key(|(at, arrow)| (*at, std::cmp::Reverse(arrow.len())))?;

    let from = line.get(..at)?.trim();
    let rest = line.get(at + arrow.len()..)?;
    // Tags ride between pipes on the far side of the arrow.
    let (tags, to) = match rest.strip_prefix('|').and_then(|rest| rest.split_once('|')) {
        Some((tags, to)) => (tags, to),
        None => ("", rest),
    };
    (!from.is_empty() && !to.trim().is_empty()).then_some((from, tags, to.trim()))
}

/// An id and, if the text carries one, its label.
fn declaration(text: &str) -> (&str, Option<&str>) {
    let text = text.trim();
    match text.find(['[', '(', '{']) {
        Some(at) => (text.get(..at).unwrap_or(text).trim(), label_of(text)),
        None => (text, None),
    }
}

/// The tag set written between the pipes.
fn tags(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(source: &str) -> Graph {
        let read = from_mermaid(source).expect("the source reads");
        read.into_iter().next().expect("there is a graph").1
    }

    fn labels(g: &Graph) -> Vec<&str> {
        g.nodes().iter().map(Node::label).collect()
    }

    #[test]
    fn a_chain_reads() {
        let g = one("graph LR\n  a --> b\n  b --> c\n");
        assert_eq!(labels(&g), ["a", "b", "c"]);
        assert_eq!(g.edges().len(), 2);
    }

    #[test]
    fn a_label_replaces_the_id_wherever_it_appears() {
        let g = one("graph LR\n  a --> b\n  a[\"the source\"]\n");
        assert_eq!(labels(&g), ["the source", "b"]);
    }

    #[test]
    fn every_shape_reads_as_a_box() {
        let g = one(
            "graph LR\n  a([\"src\"]) --> b[[\"sink\"]]\n  c((\"ghost\")) --> d{{\"reduce\"}}\n",
        );
        assert_eq!(labels(&g), ["src", "sink", "ghost", "reduce"]);
    }

    #[test]
    fn tags_ride_between_the_pipes() {
        let g = one("graph LR\n  a -->|odd, late| b\n  a --> c\n");
        assert_eq!(g.edges()[0].tags(), ["late", "odd"]);
        assert_eq!(g.edges()[1].tags(), [] as [String; 0]);
    }

    #[test]
    fn a_dotted_arrow_is_still_an_arrow() {
        let g = one("graph LR\n  a -.->|drop| b\n");
        assert_eq!(g.edges().len(), 1);
        assert_eq!(g.edges()[0].tags(), ["drop"]);
    }

    #[test]
    fn a_subgraph_is_a_graph_of_its_own() {
        let read = from_mermaid(
            "graph LR\n  subgraph one[\"first\"]\n    a --> b\n  end\n\
             subgraph two[\"second\"]\n    c --> d\n  end\n",
        )
        .expect("the source reads");
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].0, "first");
        assert_eq!(read[1].0, "second");
        assert_eq!(labels(&read[0].1), ["a", "b"]);
    }

    #[test]
    fn comments_and_styling_are_skipped() {
        let g = one("graph LR\n  %% a note\n  style a fill:#fff\n  a --> b %% trailing\n");
        assert_eq!(labels(&g), ["a", "b"]);
    }

    #[test]
    fn something_that_is_not_a_flowchart_is_refused() {
        assert_eq!(
            from_mermaid("sequenceDiagram\n  a ->> b: hi\n"),
            Err(ImportError::NotAFlowchart)
        );
        assert_eq!(from_mermaid(""), Err(ImportError::NotAFlowchart));
    }

    #[test]
    fn a_line_outside_the_subset_is_refused_rather_than_ignored() {
        let read = from_mermaid("graph LR\n  a --> b\n  this is not mermaid\n");
        assert!(
            matches!(read, Err(ImportError::Unreadable(3, _))),
            "{read:?}"
        );
    }

    #[test]
    fn an_unclosed_subgraph_is_refused() {
        let read = from_mermaid("graph LR\n  subgraph one\n    a --> b\n");
        assert!(matches!(read, Err(ImportError::Unbalanced(_))), "{read:?}");
    }
}
