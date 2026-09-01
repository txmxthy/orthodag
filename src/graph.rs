//! The graph a caller hands in.
//!
//! Vertices carry a headline and a few lines of text; edges carry an optional
//! set of tags. Nothing here is parsed and nothing is laid out — this is the
//! whole of the input model.

use std::fmt;

/// Identifies a node within the [`Graph`] that produced it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct NodeId(u32);

/// Identifies an edge within the [`Graph`] that produced it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct EdgeId(u32);

impl NodeId {
    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }

    /// The id at a position, for a test standing one phase up on its own.
    ///
    /// Test-only: outside a test the only way to get an id is to add the thing
    /// it names, which is the point of the newtype.
    #[cfg(test)]
    pub(crate) fn from_index(at: usize) -> Self {
        Self(u32::try_from(at).unwrap_or(u32::MAX))
    }
}

impl EdgeId {
    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }

    /// The id at a position. See [`NodeId::from_index`].
    #[cfg(test)]
    pub(crate) fn from_index(at: usize) -> Self {
        Self(u32::try_from(at).unwrap_or(u32::MAX))
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "n{}", self.0)
    }
}

impl fmt::Display for EdgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "e{}", self.0)
    }
}

/// A vertex: one headline, and any number of further lines drawn under it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Node {
    label: String,
    lines: Vec<String>,
}

impl Node {
    /// A node with a headline and nothing else.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            lines: Vec::new(),
        }
    }

    /// Adds one more line inside the box.
    #[must_use]
    pub fn line(mut self, line: impl Into<String>) -> Self {
        self.lines.push(line.into());
        self
    }

    /// The headline, drawn first and emphasised.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The further lines, in the order they were added.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }
}

/// A directed edge, optionally tagged.
///
/// Tags do a lot of work later: two edges carrying the same tags and ending at
/// the same vertex are one line as far as a reader is concerned, and the tag
/// set is what keys the palette. So the set is canonical — sorted and
/// deduplicated on construction — and two edges carry "the same tags" exactly
/// when [`Edge::tags`] compares equal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    from: NodeId,
    to: NodeId,
    tags: Vec<String>,
}

impl Edge {
    /// The source.
    pub fn from(&self) -> NodeId {
        self.from
    }

    /// The target.
    pub fn to(&self) -> NodeId {
        self.to
    }

    /// The tag set: sorted, deduplicated, possibly empty.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }
}

/// A directed graph, built by the caller.
///
/// Ids are dense indices into the two vectors, so they are only meaningful
/// against the graph that issued them. Nothing checks that, because nothing
/// can: an id from another graph that happens to be in range is
/// indistinguishable from a local one. See `docs/adr/0001-ids-are-indices.md`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

impl Graph {
    /// An empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a node and returns its id.
    pub fn add_node(&mut self, node: Node) -> NodeId {
        let id = NodeId(u32::try_from(self.nodes.len()).unwrap_or(u32::MAX));
        self.nodes.push(node);
        id
    }

    /// Adds an untagged edge between two nodes of this graph.
    pub fn add_edge(&mut self, from: NodeId, to: NodeId) -> EdgeId {
        self.add_tagged_edge(from, to, Vec::<String>::new())
    }

    /// Adds an edge carrying a tag set.
    ///
    /// The tags are canonicalised here, so `["odd", "late"]` and
    /// `["late", "odd", "odd"]` produce edges that compare equal.
    pub fn add_tagged_edge(
        &mut self,
        from: NodeId,
        to: NodeId,
        tags: impl IntoIterator<Item = impl Into<String>>,
    ) -> EdgeId {
        let mut tags: Vec<String> = tags.into_iter().map(Into::into).collect();
        tags.sort();
        tags.dedup();
        let id = EdgeId(u32::try_from(self.edges.len()).unwrap_or(u32::MAX));
        self.edges.push(Edge { from, to, tags });
        id
    }

    /// Every node, in insertion order.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Every edge, in insertion order.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// One node, or `None` if the id is out of range.
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.index())
    }

    /// One edge, or `None` if the id is out of range.
    pub fn edge(&self, id: EdgeId) -> Option<&Edge> {
        self.edges.get(id.index())
    }

    /// The ids of every node, in insertion order.
    pub fn node_ids(&self) -> impl Iterator<Item = NodeId> + use<> {
        (0..u32::try_from(self.nodes.len()).unwrap_or(u32::MAX)).map(NodeId)
    }

    /// The ids of every edge, in insertion order.
    pub fn edge_ids(&self) -> impl Iterator<Item = EdgeId> + use<> {
        (0..u32::try_from(self.edges.len()).unwrap_or(u32::MAX)).map(EdgeId)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_index_the_graph_that_issued_them() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b").line("two"));
        let e = g.add_edge(a, b);

        assert_eq!(g.node(a).map(Node::label), Some("a"));
        assert_eq!(g.node(b).map(Node::lines), Some(&["two".to_owned()][..]));
        assert_eq!(g.edge(e).map(Edge::from), Some(a));
        assert_eq!(g.edge(e).map(Edge::to), Some(b));
        assert_eq!(g.node_ids().collect::<Vec<_>>(), [a, b]);
        assert_eq!(g.edge_ids().collect::<Vec<_>>(), [e]);
    }

    #[test]
    fn an_id_out_of_range_reads_as_nothing_rather_than_panicking() {
        let g = Graph::new();
        assert!(g.node(NodeId(7)).is_none());
        assert!(g.edge(EdgeId(7)).is_none());
    }

    #[test]
    fn a_tag_set_is_canonical() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        let one = g.add_tagged_edge(a, b, ["late", "odd", "odd"]);
        let two = g.add_tagged_edge(a, b, ["odd", "late"]);

        assert_eq!(
            g.edge(one).map(Edge::tags),
            Some(&["late".to_owned(), "odd".to_owned()][..])
        );
        assert_eq!(g.edge(one), g.edge(two));
    }
}
