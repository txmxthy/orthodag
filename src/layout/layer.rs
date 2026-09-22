//! Cutting long edges into single-hop segments.
//!
//! An edge spanning several columns is a problem for every phase after this
//! one: crossings between two columns are easy to count, crossings between a
//! column and one four along are not. So an edge that skips columns gets a
//! placeholder in each column it passes through, and becomes a chain of hops
//! between neighbours.
//!
//! Everything downstream then deals with adjacent columns only, and the
//! placeholders turn back into a straight run across the middle of the edge.

use super::{Adjacency, acyclic::Acyclic, rank::Ranked};
use crate::graph::{Edge, EdgeId, Graph, NodeId};

/// A place in a column.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) enum Slot {
    /// A vertex of the graph, drawn as a box.
    Node(NodeId),
    /// One column of an edge on its way past, drawn as a line.
    Pass(EdgeId),
}

/// One hop of an edge, between two neighbouring columns.
///
/// `gap` is the space between column `gap` and column `gap + 1`, which is where
/// this hop is eventually drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Segment {
    pub(crate) edge: EdgeId,
    pub(crate) gap: usize,
    pub(crate) from: Slot,
    pub(crate) to: Slot,
}

/// The columns, with placeholders, and the hops between them.
#[derive(Clone, Debug, Default)]
pub(crate) struct Layered {
    columns: Vec<Vec<Slot>>,
    segments: Vec<Segment>,
}

impl Layered {
    /// How many columns there are.
    pub(crate) fn columns(&self) -> usize {
        self.columns.len()
    }

    /// One column, top to bottom.
    pub(crate) fn column(&self, index: usize) -> &[Slot] {
        self.columns.get(index).map_or(&[], Vec::as_slice)
    }

    /// Every column, top to bottom.
    pub(crate) fn all(&self) -> &[Vec<Slot>] {
        &self.columns
    }

    /// Replaces the columns, keeping the segments — what a reordering does.
    ///
    /// Reordering moves slots within a column and never adds or removes one, so
    /// the hops are unaffected.
    pub(crate) fn reorder(&mut self, columns: Vec<Vec<Slot>>) {
        self.columns = columns;
    }

    /// Every hop, in edge order then source-to-target order along each edge.
    pub(crate) fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// The hops crossing one gap.
    pub(crate) fn segments_in(&self, gap: usize) -> impl Iterator<Item = &Segment> {
        self.segments.iter().filter(move |s| s.gap == gap)
    }

    /// Where a slot sits in its column, or `None` if it is not in that column.
    pub(crate) fn position(&self, column: usize, slot: Slot) -> Option<usize> {
        self.column(column).iter().position(|s| *s == slot)
    }
}

/// Builds the layered graph, inserting a placeholder per column an edge skips.
///
/// Back edges take no part; they are drawn later through a lane of their own,
/// and giving them placeholders here would make them compete for space with
/// edges that actually flow left to right.
pub(crate) fn layer(g: &Graph, adj: &Adjacency, acyclic: &Acyclic, ranked: &Ranked) -> Layered {
    let mut columns: Vec<Vec<Slot>> = (0..ranked.columns())
        .map(|c| ranked.column(c).iter().copied().map(Slot::Node).collect())
        .collect();
    let mut segments = Vec::new();

    for from in g.node_ids() {
        for &id in adj.out(from) {
            if acyclic.is_back(id) {
                continue;
            }
            let Some(to) = g.edge(id).map(Edge::to) else {
                continue;
            };
            let (start, end) = (ranked.rank(from), ranked.rank(to));
            // Forward edges always climb by at least one; anything else means
            // the ranking never reached one of the ends, and the edge is left
            // out rather than given a hop that runs backwards or nowhere.
            if end <= start {
                continue;
            }

            let mut previous = Slot::Node(from);
            for gap in start..end {
                let next = if gap + 1 == end {
                    Slot::Node(to)
                } else {
                    let pass = Slot::Pass(id);
                    if let Some(column) = columns.get_mut(gap + 1) {
                        column.push(pass);
                    }
                    pass
                };
                segments.push(Segment {
                    edge: id,
                    gap,
                    from: previous,
                    to: next,
                });
                previous = next;
            }
        }
    }

    Layered { columns, segments }
}

#[cfg(test)]
mod tests {
    use super::super::{acyclic::back_edges, rank::rank};
    use super::*;
    use crate::graph::Node;

    fn build(nodes: usize, edges: &[(usize, usize)]) -> (Graph, Vec<NodeId>, Layered) {
        let mut g = Graph::new();
        let ids: Vec<_> = (0..nodes)
            .map(|i| g.add_node(Node::new(format!("n{i}"))))
            .collect();
        for &(a, b) in edges {
            g.add_edge(ids[a], ids[b]).unwrap();
        }
        let adj = Adjacency::of(&g);
        let acyclic = back_edges(&g, &adj);
        let ranked = rank(&g, &adj, &acyclic);
        let layered = layer(&g, &adj, &acyclic, &ranked);
        (g, ids, layered)
    }

    #[test]
    fn a_neighbouring_edge_is_one_hop_and_no_placeholder() {
        let (_, ids, l) = build(2, &[(0, 1)]);
        assert_eq!(l.column(0), [Slot::Node(ids[0])]);
        assert_eq!(l.column(1), [Slot::Node(ids[1])]);
        assert_eq!(l.segments().len(), 1);
        assert_eq!(l.segments()[0].gap, 0);
    }

    #[test]
    fn an_edge_skipping_columns_gets_one_placeholder_per_column_passed() {
        // 0 -> 1 -> 2 -> 3 puts 3 in column three; 0 -> 3 then passes two columns.
        let (g, ids, l) = build(4, &[(0, 1), (1, 2), (2, 3), (0, 3)]);
        let long = g.edge_ids().last().expect("the graph has edges");

        assert_eq!(l.column(1), [Slot::Node(ids[1]), Slot::Pass(long)]);
        assert_eq!(l.column(2), [Slot::Node(ids[2]), Slot::Pass(long)]);

        let hops: Vec<_> = l.segments().iter().filter(|s| s.edge == long).collect();
        assert_eq!(hops.len(), 3);
        assert_eq!(hops[0].from, Slot::Node(ids[0]));
        assert_eq!(hops[0].to, Slot::Pass(long));
        assert_eq!(hops[2].to, Slot::Node(ids[3]));
        assert_eq!(hops.iter().map(|s| s.gap).collect::<Vec<_>>(), [0, 1, 2]);
    }

    #[test]
    fn every_hop_joins_neighbouring_columns() {
        let (_, _, l) = build(5, &[(0, 1), (1, 2), (2, 3), (3, 4), (0, 4), (1, 4)]);
        for gap in 0..l.columns().saturating_sub(1) {
            for hop in l.segments_in(gap) {
                assert!(
                    l.position(gap, hop.from).is_some(),
                    "{hop:?} leaves column {gap}"
                );
                assert!(
                    l.position(gap + 1, hop.to).is_some(),
                    "{hop:?} enters column {gap}+1"
                );
            }
        }
    }

    #[test]
    fn a_back_edge_gets_no_hops_at_all() {
        let (g, _, l) = build(3, &[(0, 1), (1, 2), (2, 1)]);
        let back = g.edge_ids().last().expect("the graph has edges");
        assert!(l.segments().iter().all(|s| s.edge != back));
        assert!(l.all().iter().all(|c| !c.contains(&Slot::Pass(back))));
    }

    #[test]
    fn an_empty_graph_layers_to_nothing() {
        let (_, _, l) = build(0, &[]);
        assert_eq!(l.columns(), 0);
        assert_eq!(l.segments(), []);
    }
}
