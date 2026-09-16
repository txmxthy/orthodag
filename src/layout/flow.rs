//! Which edges are one line.
//!
//! Two edges carrying the same tags into the same box are one flow as far as a
//! reader is concerned — `design.md` §3 — and every phase that treats them as
//! one has to agree on which of them speaks for the group.

use crate::graph::{EdgeId, Graph, NodeId};

/// The edge each edge answers to: the lowest id among the tagged edges that
/// share its target and its tag set, or itself when it has no company.
///
/// Untagged edges answer only to themselves. Two of them into one box are two
/// lines that happen to share a door, and nothing in the graph says otherwise.
pub(crate) fn representative(g: &Graph) -> Vec<EdgeId> {
    let mut keys: Vec<((NodeId, &[String]), EdgeId)> = Vec::new();
    g.edge_ids()
        .map(|id| {
            let Some(edge) = g.edge(id).filter(|e| !e.tags().is_empty()) else {
                return id;
            };
            let key = (edge.to(), edge.tags());
            if let Some((_, first)) = keys.iter().find(|(k, _)| *k == key) {
                return *first;
            }
            keys.push((key, id));
            id
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Node;

    #[test]
    fn same_tags_into_one_box_answer_to_the_first_of_them() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        let c = g.add_node(Node::new("c"));
        let first = g.add_tagged_edge(a, c, ["t"]);
        let plain = g.add_edge(a, b);
        let second = g.add_tagged_edge(b, c, ["t"]);
        let other = g.add_tagged_edge(b, c, ["u"]);

        assert_eq!(representative(&g), vec![first, plain, first, other]);
        assert_eq!(representative(&g)[second.index()], first);
    }
}
