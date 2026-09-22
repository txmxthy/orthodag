//! The graph a caller hands in.
//!
//! Vertices carry a headline and a few lines of text; edges carry an optional
//! set of tags. Nothing here is parsed and nothing is laid out — this is the
//! whole of the input model.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Provenance(u64);

impl Provenance {
    fn fresh() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        match NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            next.checked_add(1)
        }) {
            Ok(provenance) => Self(provenance),
            Err(_) => std::process::abort(),
        }
    }
}

/// Identifies a node within the [`Graph`] that produced it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId {
    provenance: Provenance,
    index: u32,
}

/// Identifies an edge within the [`Graph`] that produced it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeId {
    provenance: Provenance,
    index: u32,
}

impl NodeId {
    /// The position this id names in its graph's node list.
    ///
    /// Ids are indices (ADR 0001): the nth node added has index n, so a caller
    /// keeping a table per node can key it on this.
    pub fn index(self) -> usize {
        self.index as usize
    }

    /// The id at a position, for a test standing one phase up on its own.
    ///
    /// Test-only: outside a test the only way to get an id is to add the thing
    /// it names, which is the point of the newtype.
    #[cfg(test)]
    pub(crate) fn from_index(at: usize) -> Self {
        Self::for_graph(Provenance(0), at)
    }

    #[cfg(test)]
    fn for_graph(provenance: Provenance, at: usize) -> Self {
        Self {
            provenance,
            index: u32::try_from(at).unwrap_or(u32::MAX),
        }
    }
}

impl EdgeId {
    /// The position this id names in its graph's edge list. See [`NodeId::index`].
    pub fn index(self) -> usize {
        self.index as usize
    }

    /// The id at a position. See [`NodeId::from_index`].
    #[cfg(test)]
    pub(crate) fn from_index(at: usize) -> Self {
        Self::for_graph(Provenance(0), at)
    }

    #[cfg(test)]
    fn for_graph(provenance: Provenance, at: usize) -> Self {
        Self {
            provenance,
            index: u32::try_from(at).unwrap_or(u32::MAX),
        }
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("NodeId").field(&self.index).finish()
    }
}

impl fmt::Debug for EdgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EdgeId").field(&self.index).finish()
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "n{}", self.index)
    }
}

impl fmt::Display for EdgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "e{}", self.index)
    }
}

/// Why a graph mutation or validation could not use an id.
#[non_exhaustive]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GraphError {
    /// A node id was issued by another graph.
    ForeignNode(NodeId),
    /// A node id belongs to this graph but does not name a stored node.
    NodeOutOfRange(NodeId),
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignNode(id) => write!(f, "node {id} belongs to another graph"),
            Self::NodeOutOfRange(id) => write!(f, "node {id} is not in this graph"),
        }
    }
}

impl std::error::Error for GraphError {}

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
/// Ids carry this graph's process-local identity as well as a dense index. A
/// clone deliberately preserves that identity, while graph equality compares
/// only structure. See `docs/adr/0001-ids-are-indices.md`.
#[derive(Clone)]
pub struct Graph {
    provenance: Provenance,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

impl Default for Graph {
    fn default() -> Self {
        Self {
            provenance: Provenance::fresh(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}

impl fmt::Debug for Graph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Graph")
            .field("nodes", &self.nodes)
            .field("edges", &self.edges)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Graph {
    fn eq(&self, other: &Self) -> bool {
        self.nodes == other.nodes
            && self.edges.len() == other.edges.len()
            && self.edges.iter().zip(&other.edges).all(|(one, other)| {
                one.from.index == other.from.index
                    && one.to.index == other.to.index
                    && one.tags == other.tags
            })
    }
}

impl Eq for Graph {}

impl Graph {
    /// An empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a node and returns its id.
    pub fn add_node(&mut self, node: Node) -> NodeId {
        let id = NodeId {
            provenance: self.provenance,
            index: u32::try_from(self.nodes.len()).unwrap_or(u32::MAX),
        };
        self.nodes.push(node);
        id
    }

    /// Adds an untagged edge between two nodes of this graph.
    ///
    /// # Errors
    ///
    /// [`GraphError`] if either id is foreign or out of range.
    pub fn add_edge(&mut self, from: NodeId, to: NodeId) -> Result<EdgeId, GraphError> {
        self.add_tagged_edge(from, to, Vec::<String>::new())
    }

    /// Adds an edge carrying a tag set.
    ///
    /// The tags are canonicalised here, so `["odd", "late"]` and
    /// `["late", "odd", "odd"]` produce edges that compare equal.
    ///
    /// # Errors
    ///
    /// [`GraphError`] if either id is foreign or out of range.
    pub fn add_tagged_edge(
        &mut self,
        from: NodeId,
        to: NodeId,
        tags: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<EdgeId, GraphError> {
        self.check_node(from)?;
        self.check_node(to)?;
        let mut tags: Vec<String> = tags.into_iter().map(Into::into).collect();
        tags.sort();
        tags.dedup();
        let id = EdgeId {
            provenance: self.provenance,
            index: u32::try_from(self.edges.len()).unwrap_or(u32::MAX),
        };
        self.edges.push(Edge { from, to, tags });
        Ok(id)
    }

    /// Replaces a node's headline.
    ///
    /// For a reader building a graph from text, where a node can be mentioned
    /// by an edge before the line that names it.
    ///
    /// # Errors
    ///
    /// [`GraphError`] if the id is foreign or out of range.
    pub fn relabel(&mut self, id: NodeId, label: impl Into<String>) -> Result<(), GraphError> {
        self.check_node(id)?;
        if let Some(node) = self.nodes.get_mut(id.index()) {
            node.label = label.into();
        }
        Ok(())
    }

    /// Every node, in insertion order.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Every edge, in insertion order.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// One node, or `None` if the id is foreign or out of range.
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        if id.provenance != self.provenance {
            return None;
        }
        self.nodes.get(id.index())
    }

    /// One edge, or `None` if the id is foreign or out of range.
    pub fn edge(&self, id: EdgeId) -> Option<&Edge> {
        if id.provenance != self.provenance {
            return None;
        }
        self.edges.get(id.index())
    }

    /// Checks that every stored edge endpoint belongs to this graph and names a node.
    ///
    /// # Errors
    ///
    /// [`GraphError`] for the first invalid endpoint.
    pub fn validate(&self) -> Result<(), GraphError> {
        for edge in &self.edges {
            self.check_node(edge.from)?;
            self.check_node(edge.to)?;
        }
        Ok(())
    }

    /// The ids of every node, in insertion order.
    pub fn node_ids(&self) -> impl Iterator<Item = NodeId> + use<> {
        let provenance = self.provenance;
        (0..u32::try_from(self.nodes.len()).unwrap_or(u32::MAX))
            .map(move |index| NodeId { provenance, index })
    }

    /// The ids of every edge, in insertion order.
    pub fn edge_ids(&self) -> impl Iterator<Item = EdgeId> + use<> {
        let provenance = self.provenance;
        (0..u32::try_from(self.edges.len()).unwrap_or(u32::MAX))
            .map(move |index| EdgeId { provenance, index })
    }

    pub(crate) fn node_id_at(&self, index: usize) -> Option<NodeId> {
        (index < self.nodes.len()).then(|| NodeId {
            provenance: self.provenance,
            index: u32::try_from(index).unwrap_or(u32::MAX),
        })
    }

    pub(crate) fn edge_id_at(&self, index: usize) -> Option<EdgeId> {
        (index < self.edges.len()).then(|| EdgeId {
            provenance: self.provenance,
            index: u32::try_from(index).unwrap_or(u32::MAX),
        })
    }

    fn check_node(&self, id: NodeId) -> Result<(), GraphError> {
        if id.provenance != self.provenance {
            Err(GraphError::ForeignNode(id))
        } else if id.index() >= self.nodes.len() {
            Err(GraphError::NodeOutOfRange(id))
        } else {
            Ok(())
        }
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
        let e = g.add_edge(a, b).unwrap();

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
        assert!(g.node(NodeId::for_graph(g.provenance, 7)).is_none());
        assert!(g.edge(EdgeId::for_graph(g.provenance, 7)).is_none());
    }

    #[test]
    fn a_tag_set_is_canonical() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        let one = g.add_tagged_edge(a, b, ["late", "odd", "odd"]).unwrap();
        let two = g.add_tagged_edge(a, b, ["odd", "late"]).unwrap();

        assert_eq!(
            g.edge(one).map(Edge::tags),
            Some(&["late".to_owned(), "odd".to_owned()][..])
        );
        assert_eq!(g.edge(one), g.edge(two));
    }

    #[test]
    fn foreign_ids_are_rejected() {
        let mut one = Graph::new();
        let one_node = one.add_node(Node::new("one"));
        let one_edge = one.add_edge(one_node, one_node).unwrap();
        let mut other = Graph::new();
        let other_node = other.add_node(Node::new("other"));
        let other_edge = other.add_edge(other_node, other_node).unwrap();

        assert_ne!(one_node, other_node);
        assert_ne!(one_edge, other_edge);
        assert!(other.node(one_node).is_none());
        assert!(other.edge(one_edge).is_none());
        assert_eq!(
            other.add_edge(one_node, other_node),
            Err(GraphError::ForeignNode(one_node))
        );
        assert_eq!(
            other.add_tagged_edge(other_node, one_node, ["tag"]),
            Err(GraphError::ForeignNode(one_node))
        );
        assert_eq!(
            other.relabel(one_node, "changed"),
            Err(GraphError::ForeignNode(one_node))
        );
    }

    #[test]
    fn local_ids_out_of_range_are_rejected() {
        let mut g = Graph::new();
        let missing = NodeId::for_graph(g.provenance, 7);

        assert!(g.node(missing).is_none());
        assert_eq!(
            g.add_edge(missing, missing),
            Err(GraphError::NodeOutOfRange(missing))
        );
        assert_eq!(
            g.relabel(missing, "changed"),
            Err(GraphError::NodeOutOfRange(missing))
        );
    }

    #[test]
    fn a_clone_preserves_identity_and_structural_equality_ignores_it() {
        let mut original = Graph::new();
        let node = original.add_node(Node::new("node"));
        let edge = original.add_edge(node, node).unwrap();
        let clone = original.clone();
        let mut same_shape = Graph::new();
        let other_node = same_shape.add_node(Node::new("node"));
        same_shape.add_edge(other_node, other_node).unwrap();

        assert_eq!(clone.node(node).map(Node::label), Some("node"));
        assert_eq!(clone.edge(edge).map(Edge::from), Some(node));
        assert_eq!(original, clone);
        assert_eq!(original, same_shape);
        assert_ne!(node, other_node);
    }

    #[test]
    fn validation_checks_stored_endpoint_identity_and_range() {
        let mut g = Graph::new();
        let node = g.add_node(Node::new("node"));
        g.add_edge(node, node).unwrap();
        assert_eq!(g.validate(), Ok(()));

        let foreign = Graph::new();
        g.edges[0].to = NodeId::for_graph(foreign.provenance, 0);
        assert_eq!(g.validate(), Err(GraphError::ForeignNode(g.edges[0].to)));

        g.edges[0].to = NodeId::for_graph(g.provenance, 7);
        assert_eq!(g.validate(), Err(GraphError::NodeOutOfRange(g.edges[0].to)));
    }
}
