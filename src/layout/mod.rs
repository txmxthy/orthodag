//! Turning a graph into a drawing, one phase at a time.
//!
//! The phases are the classical layered ones — make it acyclic, assign layers,
//! order them, place them, route between them — plus the ones the classical
//! presentation leaves to the renderer. Each is a plain function over the phase
//! before it; nothing is shared through a context object.

mod acyclic;

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
