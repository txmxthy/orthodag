//! Which column each node sits in.
//!
//! Longest path in Kahn order: every vertex sits one column past its latest
//! predecessor. One pass, tight — no vertex is further right than it has to
//! be — and it takes no parameters.
//!
//! It has a known weakness. A vertex with one early predecessor and one late
//! one hugs the early one and stretches the other edge across the drawing;
//! network simplex would pull it right and delete the long edge. Longest path
//! first, measure, and revisit: the phase boundary keeps the better algorithm a
//! drop-in replacement.

use std::collections::VecDeque;

use super::{Adjacency, acyclic::Acyclic};
use crate::graph::{Edge, Graph, NodeId};

/// Every node's column, and the nodes of each column in id order.
#[derive(Clone, Debug, Default)]
pub(crate) struct Ranked {
    rank: Vec<usize>,
    columns: Vec<Vec<NodeId>>,
}

impl Ranked {
    /// The column this node sits in.
    pub(crate) fn rank(&self, node: NodeId) -> usize {
        self.rank.get(node.index()).copied().unwrap_or(0)
    }

    /// How many columns the drawing has.
    pub(crate) fn columns(&self) -> usize {
        self.columns.len()
    }

    /// The nodes of one column, in id order.
    pub(crate) fn column(&self, index: usize) -> &[NodeId] {
        self.columns.get(index).map_or(&[], Vec::as_slice)
    }
}

/// Puts every node one column past its latest predecessor.
///
/// Back edges take no part: they are the edges that would make this impossible.
/// A node that the sweep never reaches — which needs a cycle the back-edge pass
/// missed, so it should not happen — stays in column zero rather than being
/// left without one.
pub(crate) fn rank(g: &Graph, adj: &Adjacency, acyclic: &Acyclic) -> Ranked {
    let n = g.nodes().len();
    let mut rank = vec![0usize; n];
    let mut waiting = vec![0usize; n];

    for node in g.node_ids() {
        for to in forward_targets(g, adj, acyclic, node) {
            if let Some(slot) = waiting.get_mut(to.index()) {
                *slot += 1;
            }
        }
    }

    let mut queue: VecDeque<NodeId> = g.node_ids().filter(|n| waiting[n.index()] == 0).collect();
    while let Some(node) = queue.pop_front() {
        let next = rank[node.index()] + 1;
        for to in forward_targets(g, adj, acyclic, node) {
            let Some(slot) = rank.get_mut(to.index()) else {
                continue;
            };
            *slot = (*slot).max(next);
            waiting[to.index()] -= 1;
            if waiting[to.index()] == 0 {
                queue.push_back(to);
            }
        }
    }

    let width = rank.iter().copied().max().map_or(0, |m| m + 1);
    let mut columns = vec![Vec::new(); if n == 0 { 0 } else { width }];
    for node in g.node_ids() {
        if let Some(column) = columns.get_mut(rank[node.index()]) {
            column.push(node);
        }
    }

    Ranked { rank, columns }
}

/// The targets of this node's forward edges, in the order they were added.
fn forward_targets<'a>(
    g: &'a Graph,
    adj: &'a Adjacency,
    acyclic: &'a Acyclic,
    node: NodeId,
) -> impl Iterator<Item = NodeId> + 'a {
    adj.out(node)
        .iter()
        .filter(|e| !acyclic.is_back(**e))
        .filter_map(|e| g.edge(*e).map(Edge::to))
}

#[cfg(test)]
mod tests {
    use super::super::acyclic::back_edges;
    use super::*;
    use crate::graph::Node;

    fn ranks(nodes: usize, edges: &[(usize, usize)]) -> Vec<usize> {
        let mut g = Graph::new();
        let ids: Vec<_> = (0..nodes)
            .map(|i| g.add_node(Node::new(format!("n{i}"))))
            .collect();
        for &(a, b) in edges {
            g.add_edge(ids[a], ids[b]).unwrap();
        }
        let adj = Adjacency::of(&g);
        let ranked = rank(&g, &adj, &back_edges(&g, &adj));
        ids.iter().map(|n| ranked.rank(*n)).collect()
    }

    #[test]
    fn a_chain_is_one_node_per_column() {
        assert_eq!(ranks(4, &[(0, 1), (1, 2), (2, 3)]), [0, 1, 2, 3]);
    }

    #[test]
    fn a_fan_out_shares_a_column() {
        assert_eq!(ranks(4, &[(0, 1), (0, 2), (0, 3)]), [0, 1, 1, 1]);
    }

    #[test]
    fn a_node_sits_past_its_latest_predecessor_not_its_earliest() {
        // 0 -> 1 -> 2 -> 3 and 0 -> 3: the long edge stretches, 3 does not move left.
        assert_eq!(ranks(4, &[(0, 1), (1, 2), (2, 3), (0, 3)]), [0, 1, 2, 3]);
    }

    #[test]
    fn a_back_edge_does_not_push_anything_right() {
        // The cycle 1 -> 2 -> 1 would otherwise have no ranking at all.
        assert_eq!(ranks(3, &[(0, 1), (1, 2), (2, 1)]), [0, 1, 2]);
    }

    #[test]
    fn a_disconnected_node_starts_at_the_left() {
        assert_eq!(ranks(3, &[(0, 1)]), [0, 1, 0]);
    }

    #[test]
    fn columns_hold_their_nodes_in_id_order() {
        let mut g = Graph::new();
        let ids: Vec<_> = (0..4)
            .map(|i| g.add_node(Node::new(format!("n{i}"))))
            .collect();
        for target in &ids[1..] {
            g.add_edge(ids[0], *target).unwrap();
        }
        let adj = Adjacency::of(&g);
        let ranked = rank(&g, &adj, &back_edges(&g, &adj));

        assert_eq!(ranked.columns(), 2);
        assert_eq!(ranked.column(0), [ids[0]]);
        assert_eq!(ranked.column(1), &ids[1..]);
        assert_eq!(ranked.column(9), []);
    }

    #[test]
    fn an_empty_graph_has_no_columns() {
        assert_eq!(
            rank(
                &Graph::new(),
                &Adjacency::of(&Graph::new()),
                &Acyclic::default()
            )
            .columns(),
            0
        );
    }
}
