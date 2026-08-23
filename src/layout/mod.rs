//! Turning a graph into a drawing, one phase at a time.
//!
//! The phases are the classical layered ones — make it acyclic, assign layers,
//! order them, place them, route between them — plus the ones the classical
//! presentation leaves to the renderer. Each is a plain function over the phase
//! before it; nothing is shared through a context object.

mod acyclic;
mod layer;
mod order;
mod rank;

use crate::graph::{EdgeId, Graph, NodeId};

/// Out-edges per node, built once and read by every phase.
///
/// An edge whose endpoints do not resolve against this graph — only reachable
/// by mixing ids from two graphs, see `docs/adr/0001-ids-are-indices.md` — is
/// absent here, and so is absent from the drawing.
pub(crate) struct Adjacency {
    out: Vec<Vec<EdgeId>>,
}

impl Adjacency {
    pub(crate) fn of(g: &Graph) -> Self {
        let mut out = vec![Vec::new(); g.nodes().len()];
        for id in g.edge_ids() {
            let Some(edge) = g.edge(id) else { continue };
            let (from, to) = (edge.from(), edge.to());
            if g.node(from).is_none() || g.node(to).is_none() {
                continue;
            }
            out[from.index()].push(id);
        }
        Self { out }
    }

    pub(crate) fn out(&self, node: NodeId) -> &[EdgeId] {
        self.out.get(node.index()).map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::Adjacency;
    use super::{acyclic::back_edges, layer::layer, order, rank::rank};
    use crate::graph::{Graph, Node};

    /// A pseudo-random graph that is the same graph every time.
    ///
    /// Nothing in the library is random, so a generator is only here to reach
    /// shapes nobody would write by hand — several columns, fan-out, skips and
    /// a few cycles at once.
    fn seeded(seed: u64, nodes: usize, edges: usize) -> Graph {
        let mut state = seed | 1;
        let mut next = move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as usize
        };

        let mut g = Graph::new();
        let ids: Vec<_> = (0..nodes)
            .map(|i| g.add_node(Node::new(format!("n{i}"))))
            .collect();
        for _ in 0..edges {
            let (a, b) = (next() % nodes, next() % nodes);
            if a == b {
                continue;
            }
            // A tenth of the edges point backwards, so cycles are in the mix.
            let (from, to) = if next() % 10 == 0 {
                (b, a)
            } else {
                (a.min(b), a.max(b))
            };
            g.add_tagged_edge(ids[from], ids[to], [format!("t{}", next() % 4)]);
        }
        g
    }

    /// Everything there is so far: acyclic, ranked, layered, ordered.
    fn pipeline(g: &Graph) -> Vec<order::Columns> {
        let adj = Adjacency::of(g);
        let acyclic = back_edges(g, &adj);
        let ranked = rank(g, &adj, &acyclic);
        order::orderings(&layer(g, &adj, &acyclic, &ranked))
    }

    #[test]
    fn the_same_graph_orders_the_same_way_every_run() {
        let g = seeded(0x5EED, 60, 140);
        let once = pipeline(&g);
        assert!(once.len() > 1, "the graph is too tame to be worth checking");
        for run in 0..16 {
            assert_eq!(pipeline(&g), once, "run {run} differed");
        }
    }

    #[test]
    fn different_graphs_are_not_accidentally_the_same() {
        assert_ne!(pipeline(&seeded(1, 40, 90)), pipeline(&seeded(2, 40, 90)));
    }

    #[test]
    fn the_candidate_count_stays_inside_its_budget() {
        // Nine: the initial order, plus at most one per sweep.
        for seed in 1..24u64 {
            assert!(
                pipeline(&seeded(seed, 120, 300)).len() <= 9,
                "seed {seed} ran long"
            );
        }
    }

    #[test]
    fn a_graph_with_no_edges_still_pipelines() {
        let mut g = Graph::new();
        for i in 0..5 {
            g.add_node(Node::new(format!("n{i}")));
        }
        assert_eq!(pipeline(&g).len(), 1);
    }
}
