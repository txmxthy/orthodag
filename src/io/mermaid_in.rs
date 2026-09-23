//! Reading a graph out of Mermaid flowchart source.
//!
//! Not a Mermaid implementation. It reads the subset that describes a directed
//! graph — nodes, edges, tags on edges, subgraphs — and refuses everything else
//! rather than guessing. A parser that silently ignores what it does not
//! understand produces a drawing that is quietly missing half the graph, which
//! is the failure this library can least afford.
//!
//! The supported subset is a `graph` or `flowchart` header with one of Mermaid's
//! five directions; node declarations with box, round or brace shapes; the five
//! arrows recognised by `split_edge`, with optional comma-separated tags; and
//! one level of `subgraph`. Styling and direction lines are accepted and ignored.
//! Quoted text recognises `&amp;`, `&quot;`, `&lt;` and `&gt;`; other
//! well-formed entities stay literal. Malformed declarations and entities are
//! errors.
//!
//! Node shapes are read and dropped. A rounded box and a doubled box mean
//! something to the tool that wrote them, and nothing here: this draws a box.

use std::fmt;

use crate::graph::{Graph, Node, NodeId};

/// Why a document could not be read.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ImportError {
    /// The document does not have a supported `graph` or `flowchart` header.
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
    if !valid_header(header) {
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
            let title =
                title(rest.trim()).ok_or_else(|| ImportError::Unreadable(at, line.to_owned()))?;
            sheets.push(Sheet::new(title));
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

/// The complete header grammar supported by this line-oriented reader.
fn valid_header(header: &str) -> bool {
    let mut words = header.split_ascii_whitespace();
    matches!(words.next(), Some("graph" | "flowchart"))
        && matches!(words.next(), Some("TB" | "TD" | "BT" | "RL" | "LR"))
        && words.next().is_none()
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
    fn node(&mut self, id: &str, label: Option<&str>) -> Option<NodeId> {
        if let Some((_, held)) = self.named.iter().find(|(name, _)| name == id) {
            let held = *held;
            if let Some(label) = label {
                self.graph.relabel(held, decode_entities(label)?).ok()?;
            }
            return Some(held);
        }
        // A label written with breaks in it comes back as a headline and the
        // lines under it, which is how it went out.
        let text = label.unwrap_or(id);
        let mut parts = text.split("<br>").map(str::trim);
        let mut node = Node::new(decode_entities(parts.next().unwrap_or(text))?);
        for line in parts {
            node = node.line(decode_entities(line)?);
        }
        let node = self.graph.add_node(node);
        self.named.push((id.to_owned(), node));
        Some(node)
    }
}

/// Lines that are Mermaid but say nothing about the graph.
fn is_noise(line: &str) -> bool {
    ["style ", "classDef ", "class ", "linkStyle ", "direction "]
        .iter()
        .any(|prefix| line.starts_with(prefix))
}

/// The text of a `subgraph` header: its label if it has one, else its id.
fn title(rest: &str) -> Option<String> {
    let label = if rest.contains(['[', '(', '{']) {
        declaration(rest)?.1?
    } else {
        rest
    };
    decode_entities(label)
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
        let (from_id, from_label) = declaration(from)?;
        let (to_id, to_label) = declaration(to)?;
        let source = sheet.node(from_id, from_label)?;
        let target = sheet.node(to_id, to_label)?;
        sheet
            .graph
            .add_tagged_edge(source, target, tags(arrow)?)
            .ok()?;
        return Some(());
    }

    // A bare declaration: an id, optionally with a shape and a label.
    let (id, label) = declaration(line)?;
    if id.is_empty() || id.contains(char::is_whitespace) {
        return None;
    }
    sheet.node(id, label)?;
    Some(())
}

/// Splits `a -->|tag| b` into its three parts.
fn split_edge(line: &str) -> Option<(&str, &str, &str)> {
    // Longest first, so `-.->` is not read as `-` then `.->`.
    let arrows = ["-.->", "==>", "-->", "--x", "--o"];
    let mut quoted = false;
    let mut depth = 0usize;
    let mut found = None;
    for (at, ch) in line.char_indices() {
        if ch == '"' {
            quoted = !quoted;
        } else if !quoted && matches!(ch, '[' | '(' | '{') {
            depth += 1;
        } else if !quoted && matches!(ch, ']' | ')' | '}') {
            depth = depth.saturating_sub(1);
        }
        if !quoted
            && depth == 0
            && let Some(arrow) = arrows.iter().find(|arrow| line[at..].starts_with(**arrow))
        {
            found = Some((at, *arrow));
            break;
        }
    }
    let (at, arrow) = found?;

    let from = line.get(..at)?.trim();
    let rest = line.get(at + arrow.len()..)?;
    // Tags ride between pipes on the far side of the arrow.
    let (tags, to) = match rest.strip_prefix('|') {
        Some(tagged) => tagged.split_once('|')?,
        None => ("", rest),
    };
    (!from.is_empty() && !to.trim().is_empty()).then_some((from, tags, to.trim()))
}

/// An id and, if the text carries one, its label.
fn declaration(text: &str) -> Option<(&str, Option<&str>)> {
    let text = text.trim();
    let Some(at) = text.find(['[', '(', '{']) else {
        return (!text.is_empty()
            && !text.contains(char::is_whitespace)
            && !text.contains([']', ')', '}', '"']))
        .then_some((text, None));
    };
    let id = text.get(..at)?.trim();
    if id.is_empty() || id.contains(char::is_whitespace) {
        return None;
    }

    let mut stack = Vec::new();
    let mut quoted = false;
    let mut end = None;
    for (offset, ch) in text.get(at..)?.char_indices() {
        if ch == '"' {
            quoted = !quoted;
        } else if !quoted && matches!(ch, '[' | '(' | '{') {
            stack.push(ch);
        } else if !quoted && matches!(ch, ']' | ')' | '}') {
            let want = match ch {
                ']' => '[',
                ')' => '(',
                '}' => '{',
                _ => return None,
            };
            if stack.pop() != Some(want) {
                return None;
            }
            if stack.is_empty() {
                end = Some(at + offset + ch.len_utf8());
                break;
            }
        }
    }
    let end = end?;
    if quoted || !text.get(end..)?.trim().is_empty() {
        return None;
    }
    Some((id, label_of(text.get(at..end)?)))
}

/// The tag set written between the pipes.
fn tags(text: &str) -> Option<Vec<String>> {
    text.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(decode_entities)
        .collect()
}

/// Decode the entities this subset writes. Other syntactically valid entities
/// stay literal; rejecting them would require pretending to implement HTML.
fn decode_entities(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(rest.get(..at)?);
        rest = rest.get(at + 1..)?;
        let end = rest.find(';')?;
        let entity = rest.get(..end)?;
        let valid = entity
            .strip_prefix("#x")
            .or_else(|| entity.strip_prefix("#X"))
            .is_some_and(|digits| {
                !digits.is_empty() && digits.chars().all(|c| c.is_ascii_hexdigit())
            })
            || entity.strip_prefix('#').is_some_and(|digits| {
                !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
            })
            || entity
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphabetic)
                && entity.bytes().all(|byte| byte.is_ascii_alphanumeric());
        if !valid {
            return None;
        }
        match entity {
            "amp" => out.push('&'),
            "quot" => out.push('"'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            _ => {
                out.push('&');
                out.push_str(entity);
                out.push(';');
            }
        }
        rest = rest.get(end + 1..)?;
    }
    out.push_str(rest);
    Some(out)
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
    fn the_header_is_an_exact_keyword_and_direction() {
        for header in [
            "graph LR",
            "graph\tRL",
            "flowchart TB",
            "flowchart TD",
            "flowchart BT",
        ] {
            assert!(
                from_mermaid(&format!("{header}\n  a\n")).is_ok(),
                "{header}"
            );
        }
        for header in [
            "graphical LR",
            "graphLR",
            "flowcharting TD",
            "flowchart sideways",
            "graph",
            "graph LR extra",
        ] {
            assert_eq!(
                from_mermaid(&format!("{header}\n  a\n")),
                Err(ImportError::NotAFlowchart),
                "{header}"
            );
        }
    }

    #[test]
    fn entities_are_decoded_or_preserved_without_guessing() {
        let g = one(
            "graph LR\n  a[\"&amp; &quot;quoted&quot; &lt;br&gt; &gt; &copy; &#169; &#xA9;\"]\n",
        );
        assert_eq!(labels(&g), ["& \"quoted\" <br> > &copy; &#169; &#xA9;"]);

        for source in [
            "graph LR\n  a[\"broken &entity\"]\n",
            "graph LR\n  a -->|broken &#x;| b\n",
            "graph LR\n  subgraph s[\"broken &;\"]\n  end\n",
        ] {
            assert!(
                matches!(from_mermaid(source), Err(ImportError::Unreadable(2, _))),
                "{source:?}"
            );
        }
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
    fn malformed_declarations_are_refused() {
        for line in [
            "a[\"unterminated]",
            "a[\"unterminated\"",
            "a[\"mismatched\")",
            "a -->|unterminated b",
        ] {
            let read = from_mermaid(&format!("graph LR\n  {line}\n"));
            assert!(
                matches!(read, Err(ImportError::Unreadable(2, _))),
                "{line:?}: {read:?}"
            );
        }
    }

    #[test]
    fn an_unclosed_subgraph_is_refused() {
        let read = from_mermaid("graph LR\n  subgraph one\n    a --> b\n");
        assert!(matches!(read, Err(ImportError::Unbalanced(_))), "{read:?}");
    }
}
