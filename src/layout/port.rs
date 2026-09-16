//! Which row of a box each flow leaves on and arrives at.
//!
//! Where a box has several edges, they leave on different rows of its right
//! edge — one row per tag set — and arrive the same way on the left. A box is
//! made tall enough that no two tag sets have to share a row.
//!
//! That is a colour rule before it is a layout rule. A cell holds one character
//! and therefore one colour, so two flows sharing an attach row lose one of
//! them. It turns out to be a layout rule as well: two runs that leave a box on
//! the same row and go opposite ways box the track ordering in, because each
//! one's horizontal arrives on the row the other turns on whichever way round
//! they go.
//!
//! Rows are handed out by where each branch *first turns*, which for a long
//! edge is the row it will run across rather than the box at the far end. Sort
//! a fan by its destinations instead and any branch that detours on the way
//! will cut across its siblings on the way out.

use super::acyclic::Acyclic;
use super::layer::Slot;
use super::order::Columns;
use super::place::Placed;
use crate::graph::{EdgeId, Graph, NodeId};

/// The row each edge attaches on, at each of its ends.
#[derive(Clone, Debug, Default)]
pub(crate) struct Ports {
    exit: Vec<Option<i32>>,
    entry: Vec<Option<i32>>,
}

impl Ports {
    /// The row this edge leaves its source on.
    pub(crate) fn exit(&self, edge: EdgeId) -> Option<i32> {
        self.exit.get(edge.index()).copied().flatten()
    }

    /// The row this edge arrives at its target on.
    pub(crate) fn entry(&self, edge: EdgeId) -> Option<i32> {
        self.entry.get(edge.index()).copied().flatten()
    }
}

/// How many interior rows each box needs for its ports.
///
/// Computed from the graph alone, before anything is placed, because a box's
/// height has to be known before its column can be stacked.
pub(crate) fn interiors(g: &Graph, acyclic: &Acyclic) -> Vec<usize> {
    let mut out: Vec<Vec<&[String]>> = vec![Vec::new(); g.nodes().len()];
    let mut into: Vec<Vec<&[String]>> = vec![Vec::new(); g.nodes().len()];

    for id in g.edge_ids() {
        if acyclic.is_back(id) {
            continue;
        }
        let Some(edge) = g.edge(id) else { continue };
        for (side, node) in [(&mut out, edge.from()), (&mut into, edge.to())] {
            let Some(sets) = side.get_mut(node.index()) else {
                continue;
            };
            if !sets.contains(&edge.tags()) {
                sets.push(edge.tags());
            }
        }
    }

    out.iter()
        .zip(&into)
        .map(|(o, i)| o.len().max(i.len()))
        .collect()
}

/// Gives every edge a row at each end, once everything is placed.
pub(crate) fn rows(g: &Graph, acyclic: &Acyclic, columns: &Columns, placed: &Placed) -> Ports {
    let mut ports = Ports {
        exit: vec![None; g.edges().len()],
        entry: vec![None; g.edges().len()],
    };

    for node in g.node_ids() {
        let Some((column, at)) = find(columns, Slot::Node(node)) else {
            continue;
        };
        let (top, height) = (placed.top(column, at), placed.height_of(column, at));

        assign(
            &mut ports.exit,
            groups(g, acyclic, columns, placed, node, true),
            top,
            height,
        );
        assign(
            &mut ports.entry,
            groups(g, acyclic, columns, placed, node, false),
            top,
            height,
        );
    }

    ports
}

/// Where a slot sits: its column and its place in it.
fn find(columns: &Columns, slot: Slot) -> Option<(usize, usize)> {
    columns
        .iter()
        .enumerate()
        .find_map(|(column, slots)| Some((column, slots.iter().position(|s| *s == slot)?)))
}

/// One tag set's edges at one box, and the row they collectively turn towards.
struct Group {
    aim: i32,
    edges: Vec<EdgeId>,
}

/// The tag sets leaving (or arriving at) a box, ordered by where they turn.
fn groups(
    g: &Graph,
    acyclic: &Acyclic,
    columns: &Columns,
    placed: &Placed,
    node: NodeId,
    leaving: bool,
) -> Vec<Group> {
    let Some((column, _)) = find(columns, Slot::Node(node)) else {
        return Vec::new();
    };
    let mut sets: Vec<(&[String], Vec<EdgeId>, Vec<i32>)> = Vec::new();

    for id in g.edge_ids() {
        if acyclic.is_back(id) {
            continue;
        }
        let Some(edge) = g.edge(id) else { continue };
        let mine = if leaving { edge.from() } else { edge.to() };
        if mine != node {
            continue;
        }
        // The row this edge turns towards is the one it is on in the column
        // next door, which for a long edge is its placeholder and not its far
        // box.
        let next = if leaving {
            column.checked_add(1)
        } else {
            column.checked_sub(1)
        };
        let Some(next) = next else { continue };
        let far = if leaving { edge.to() } else { edge.from() };
        let slot = if find(columns, Slot::Node(far)).map(|(c, _)| c) == Some(next) {
            Slot::Node(far)
        } else {
            Slot::Pass(id)
        };
        // Looked for *in* that column, not anywhere. A long edge's placeholder
        // carries one id down every column it passes, so asking where the slot
        // is finds the first of them — which for an edge arriving from four
        // columns back is nowhere near the box it is arriving at. The answer
        // used to be discarded, and the edge with it: no entry row, and two
        // flows landing on one.
        let Some(aim) = columns
            .get(next)
            .and_then(|slots| slots.iter().position(|s| *s == slot))
            .map(|at| placed.middle(next, at))
        else {
            continue;
        };

        match sets.iter_mut().find(|(tags, _, _)| *tags == edge.tags()) {
            Some((_, edges, aims)) => {
                edges.push(id);
                aims.push(aim);
            }
            None => sets.push((edge.tags(), vec![id], vec![aim])),
        }
    }

    let mut groups: Vec<Group> = sets
        .into_iter()
        .map(|(_, edges, mut aims)| {
            aims.sort_unstable();
            let aim = aims.get(aims.len() / 2).copied().unwrap_or(0);
            Group { aim, edges }
        })
        .collect();
    groups.sort_by_key(|group| (group.aim, group.edges.first().copied()));
    groups
}

/// Spreads the groups over the box's interior rows, centred.
///
/// Centred rather than packed from the top so a fan stays symmetric about the
/// box: a box with two flows and four interior rows puts them on the middle two,
/// not the top two.
fn assign(rows: &mut [Option<i32>], groups: Vec<Group>, top: i32, height: i32) {
    let interior = (height - 2).max(1);
    let count = i32::try_from(groups.len()).unwrap_or(1).min(interior);
    let first = top + 1 + (interior - count) / 2;

    for (at, group) in groups.into_iter().enumerate() {
        let step = i32::try_from(at).unwrap_or(0).min(count.saturating_sub(1));
        for edge in group.edges {
            if let Some(slot) = rows.get_mut(edge.index()) {
                *slot = Some(first + step);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Adjacency, acyclic::back_edges, layer, order, place, rank};
    use super::*;
    use crate::graph::Node;

    fn build(g: &Graph) -> (Acyclic, Columns, Placed, Ports) {
        let adj = Adjacency::of(g);
        let acyclic = back_edges(g, &adj);
        let ranked = rank::rank(g, &adj, &acyclic);
        let layered = layer::layer(g, &adj, &acyclic, &ranked);
        let hops = order::Hops::of(&layered);
        let columns = layered.all().to_vec();
        let placed = place::place(
            g,
            &columns,
            &hops,
            &interiors(g, &acyclic),
            0,
            place::Merge::Flows,
        );
        let ports = rows(g, &acyclic, &columns, &placed);
        (acyclic, columns, placed, ports)
    }

    /// A long edge and a short one into one box arrive on rows of their own.
    ///
    /// The long one's placeholder carries its id down every column it passes,
    /// so asking where that slot is used to find the first of them — four
    /// columns from the box it arrives at — and the answer was thrown away
    /// along with the edge. Two flows then landed on one row and were drawn as
    /// one line.
    #[test]
    fn a_long_arrival_gets_a_row_of_its_own() {
        let mut g = Graph::new();
        let s = g.add_node(Node::new("s"));
        let chain: Vec<_> = (0..3)
            .map(|i| g.add_node(Node::new(format!("c{i}"))))
            .collect();
        let z = g.add_node(Node::new("z"));

        g.add_tagged_edge(s, chain[0], ["t1"]);
        g.add_tagged_edge(chain[0], chain[1], ["t2"]);
        g.add_tagged_edge(chain[1], chain[2], ["t3"]);
        let near = g.add_tagged_edge(chain[2], z, ["t4"]);
        let far = g.add_tagged_edge(s, z, ["x1"]);

        let (_, _, _, ports) = build(&g);
        assert!(
            ports.entry(far).is_some(),
            "the long arrival got no row at all"
        );
        assert_ne!(
            ports.entry(near),
            ports.entry(far),
            "two flows arriving on one row are drawn as one line"
        );
    }

    #[test]
    fn a_box_with_one_flow_needs_one_interior_row() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        g.add_edge(a, b);
        let adj = Adjacency::of(&g);
        assert_eq!(interiors(&g, &back_edges(&g, &adj)), [1, 1]);
    }

    #[test]
    fn a_box_with_two_tag_sets_out_needs_two() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let x = g.add_node(Node::new("x"));
        let y = g.add_node(Node::new("y"));
        g.add_tagged_edge(a, x, ["even"]);
        g.add_tagged_edge(a, y, ["odd"]);
        let adj = Adjacency::of(&g);
        assert_eq!(interiors(&g, &back_edges(&g, &adj))[0], 2);
    }

    #[test]
    fn edges_sharing_a_tag_set_share_a_row_and_need_no_extra_one() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let x = g.add_node(Node::new("x"));
        let y = g.add_node(Node::new("y"));
        let one = g.add_tagged_edge(a, x, ["even"]);
        let two = g.add_tagged_edge(a, y, ["even"]);
        let adj = Adjacency::of(&g);
        assert_eq!(interiors(&g, &back_edges(&g, &adj))[0], 1);

        let (_, _, _, ports) = build(&g);
        assert_eq!(ports.exit(one), ports.exit(two));
    }

    #[test]
    fn two_tag_sets_never_share_an_attach_row() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let x = g.add_node(Node::new("x"));
        let y = g.add_node(Node::new("y"));
        let even = g.add_tagged_edge(a, x, ["even"]);
        let odd = g.add_tagged_edge(a, y, ["odd"]);

        let (_, _, _, ports) = build(&g);
        assert_ne!(ports.exit(even), ports.exit(odd));
        assert!(ports.exit(even).is_some() && ports.exit(odd).is_some());
    }

    #[test]
    fn a_back_edge_asks_for_no_room() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        g.add_tagged_edge(a, b, ["down"]);
        g.add_tagged_edge(b, a, ["up"]);
        let adj = Adjacency::of(&g);
        assert_eq!(interiors(&g, &back_edges(&g, &adj)), [1, 1]);
    }

    #[test]
    fn an_attach_row_is_always_inside_its_box() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        for name in ["p", "q", "r", "s"] {
            let t = g.add_node(Node::new(name));
            g.add_tagged_edge(a, t, [name]);
        }
        let (_, columns, placed, ports) = build(&g);
        let (column, at) = find(&columns, Slot::Node(a)).expect("a is placed");
        let (top, height) = (placed.top(column, at), placed.height_of(column, at));

        for id in g.edge_ids() {
            let row = ports.exit(id).expect("every edge leaves somewhere");
            assert!(
                row > top && row < top + height - 1,
                "{row} is not inside {top}..{height}"
            );
        }
    }
}
