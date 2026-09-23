//! Finding the edges that point backwards.
//!
//! Layering needs an acyclic graph. A depth-first traversal colours vertices
//! white, grey and black; an edge pointing at a grey vertex closes a cycle.
//!
//! Those edges are **removed, not reversed**. A reversed arrow in a terminal
//! reads as pointing the wrong way, which is worse than the layout cost of
//! taking the edge out of the optimiser. They get their own vocabulary later: a
//! lane row under the boxes.

use super::Adjacency;
use crate::graph::{EdgeId, Graph, NodeId};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Colour {
    /// Not reached yet.
    White,
    /// On the stack: reaching it again closes a cycle.
    Grey,
    /// Finished.
    Black,
}

/// Which edges close a cycle, and so are left out of the layering.
#[derive(Clone, Debug, Default)]
pub(crate) struct Acyclic {
    back: Vec<bool>,
}

impl Acyclic {
    /// Whether this edge points backwards.
    ///
    /// An id from another graph reads as `false`, which puts the edge in the
    /// layering, where it is bounds-checked again.
    pub(crate) fn is_back(&self, edge: EdgeId) -> bool {
        self.back.get(edge.index()).copied().unwrap_or(false)
    }

    /// How many edges were taken out.
    pub(crate) fn back_count(&self) -> usize {
        self.back.iter().filter(|b| **b).count()
    }
}

/// Finds the back edges of `g`.
///
/// Deterministic: roots are visited in id order, sources before the rest, and
/// each node's out-edges in the order they were added. The same graph produces
/// the same set every run, which matters because *which* edge of a cycle is
/// called the back edge decides what the drawing looks like.
pub(crate) fn back_edges(g: &Graph, adj: &Adjacency) -> Acyclic {
    let n = g.nodes().len();
    let mut colour = vec![Colour::White; n];
    let mut back = vec![false; g.edges().len()];
    // (node, how many of its out-edges have been taken)
    let mut stack: Vec<(NodeId, usize)> = Vec::new();

    for root in roots_first(g, adj) {
        if colour[root.index()] != Colour::White {
            continue;
        }
        colour[root.index()] = Colour::Grey;
        stack.push((root, 0));

        while let Some(&mut (node, ref mut taken)) = stack.last_mut() {
            let Some(&edge) = adj.out(node).get(*taken) else {
                colour[node.index()] = Colour::Black;
                stack.pop();
                continue;
            };
            *taken += 1;
            let Some(to) = g.edge(edge).map(crate::graph::Edge::to) else {
                continue;
            };
            let Some(seen) = colour.get(to.index()).copied() else {
                continue;
            };
            match seen {
                Colour::Grey => back[edge.index()] = true,
                Colour::White => {
                    colour[to.index()] = Colour::Grey;
                    stack.push((to, 0));
                }
                Colour::Black => {}
            }
        }
    }

    Acyclic { back }
}

/// Every node, sources first, each group in id order.
///
/// Starting at a source keeps the traversal going the way the reader will:
/// entering a cycle from outside makes the edge that closes it the back edge,
/// where starting inside the cycle would pick an arbitrary member.
fn roots_first(g: &Graph, adj: &Adjacency) -> Vec<NodeId> {
    let mut has_in = vec![false; g.nodes().len()];
    for node in g.node_ids() {
        for &edge in adj.out(node) {
            if let Some(to) = g.edge(edge).map(crate::graph::Edge::to)
                && let Some(slot) = has_in.get_mut(to.index())
            {
                *slot = true;
            }
        }
    }
    let (sources, rest): (Vec<_>, Vec<_>) = g.node_ids().partition(|n| !has_in[n.index()]);
    [sources, rest].concat()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Node;

    fn graph(nodes: usize, edges: &[(usize, usize)]) -> Graph {
        let mut g = Graph::new();
        let ids: Vec<_> = (0..nodes)
            .map(|i| g.add_node(Node::new(format!("n{i}"))))
            .collect();
        for &(a, b) in edges {
            g.add_edge(ids[a], ids[b]).unwrap();
        }
        g
    }

    fn back(g: &Graph) -> Vec<usize> {
        let acyclic = back_edges(g, &Adjacency::of(g));
        g.edge_ids()
            .filter(|e| acyclic.is_back(*e))
            .map(EdgeId::index)
            .collect()
    }

    #[test]
    fn a_chain_has_no_back_edges() {
        assert_eq!(back(&graph(3, &[(0, 1), (1, 2)])), Vec::<usize>::new());
    }

    #[test]
    fn a_diamond_has_no_back_edges() {
        let g = graph(4, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
        assert_eq!(back(&g), Vec::<usize>::new());
    }

    #[test]
    fn the_edge_that_closes_a_cycle_is_the_back_edge() {
        // 0 -> 1 -> 2 -> 1: entering at the source makes 2 -> 1 the one.
        assert_eq!(back(&graph(3, &[(0, 1), (1, 2), (2, 1)])), [2]);
    }

    #[test]
    fn a_self_loop_points_backwards() {
        assert_eq!(back(&graph(2, &[(0, 1), (1, 1)])), [1]);
    }

    #[test]
    fn a_cycle_with_no_source_still_terminates() {
        let g = graph(3, &[(0, 1), (1, 2), (2, 0)]);
        assert_eq!(back_edges(&g, &Adjacency::of(&g)).back_count(), 1);
    }

    #[test]
    fn a_deep_chain_does_not_overflow_the_stack() {
        let edges: Vec<_> = (0..20_000).map(|i| (i, i + 1)).collect();
        let g = graph(20_001, &edges);
        assert_eq!(back_edges(&g, &Adjacency::of(&g)).back_count(), 0);
    }
}
