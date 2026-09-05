//! Turning a graph into a drawing, one phase at a time.
//!
//! The phases are the classical layered ones — make it acyclic, assign layers,
//! order them, place them, route between them — plus the ones the classical
//! presentation leaves to the renderer. Each is a plain function over the phase
//! before it; nothing is shared through a context object.

mod acyclic;
mod layer;
mod order;
mod place;
mod port;
mod rank;
pub(crate) mod route;
mod track;

use crate::graph::{EdgeId, Graph, NodeId};
use crate::options::Options;

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

/// Everything, in order: acyclic, ranked, layered, ordered, placed, routed.
///
/// The ordering phase proposes several candidates and this takes the first.
/// Choosing between them means drawing each one and scoring the drawing, and
/// there is nothing to draw with yet.
pub(crate) fn build(g: &Graph, options: Options) -> route::Layout {
    let adj = Adjacency::of(g);
    let acyclic = acyclic::back_edges(g, &adj);
    let ranked = rank::rank(g, &adj, &acyclic);
    let layered = layer::layer(g, &adj, &acyclic, &ranked);
    let hops = order::Hops::of(&layered);
    let columns = order::orderings(&layered)
        .into_iter()
        .next()
        .unwrap_or_default();
    let interiors = port::interiors(g, &acyclic);
    let placed = place::place(g, &columns, &hops, &interiors);
    let ports = port::rows(g, &acyclic, &columns, &placed);
    route::route(g, &acyclic, &columns, &placed, &ports, options)
}

#[cfg(test)]
mod tests {
    use super::Adjacency;
    use super::{acyclic::back_edges, build, layer::layer, order, rank::rank};
    use crate::graph::{Edge, Graph, Node};
    use crate::options::Options;

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
    fn every_edge_reaches_the_page() {
        // The scorer scores what is drawn, so an edge that never got drawn
        // costs nothing and no metric can see it. This is the only thing that
        // can: one route per edge, forward or back, or the drawing is lying
        // about the graph.
        for seed in 1..12u64 {
            let g = seeded(seed, 24, 50);
            let drawing = build(&g, Options::default());
            let mut routed: Vec<_> = drawing.routes.iter().map(|r| r.edge).collect();
            routed.sort_unstable();
            assert_eq!(
                routed,
                g.edge_ids().collect::<Vec<_>>(),
                "seed {seed}: some edge is missing from the drawing"
            );
        }
    }

    #[test]
    fn a_forward_route_starts_and_ends_on_its_boxes() {
        for seed in 1..12u64 {
            let g = seeded(seed, 24, 50);
            let adj = Adjacency::of(&g);
            let acyclic = back_edges(&g, &adj);
            let drawing = build(&g, Options::default());

            for route in drawing.routes.iter().filter(|r| !acyclic.is_back(r.edge)) {
                let edge = g.edge(route.edge).expect("a route names a real edge");
                let source = drawing.boxed(edge.from()).expect("the source is drawn");
                let target = drawing.boxed(edge.to()).expect("the target is drawn");
                let (first, last) = (
                    *route.points.first().expect("a route has points"),
                    *route.points.last().expect("a route has points"),
                );

                // A box has an attach row per tag set now, so the row is not
                // fixed; what is fixed is that it is inside the box, and that
                // the line starts and stops one cell clear of the border.
                assert_eq!(first.0, source.x + source.w);
                assert_eq!(last.0, target.x - 1);
                assert!(first.1 > source.y && first.1 < source.y + source.h - 1);
                assert!(last.1 > target.y && last.1 < target.y + target.h - 1);
            }
        }
    }

    #[test]
    fn a_back_edge_leaves_downward_and_arrives_from_below() {
        let mut g = Graph::new();
        let ids: Vec<_> = (0..3)
            .map(|i| g.add_node(Node::new(format!("n{i}"))))
            .collect();
        for &(a, b) in &[(0, 1), (1, 2), (2, 1)] {
            g.add_edge(ids[a], ids[b]);
        }
        let drawing = build(&g, Options::default());
        assert_eq!(drawing.routes.len(), 3, "the loop is drawn too");

        let loop_back = drawing
            .routes
            .iter()
            .find(|r| g.edge(r.edge).map(Edge::to) == Some(ids[1]) && r.points.len() > 2)
            .expect("the back edge is routed");
        let source = drawing.boxed(ids[2]).expect("its source is drawn");
        let target = drawing.boxed(ids[1]).expect("its target is drawn");
        let (first, last) = (
            loop_back.points[0],
            loop_back.points[loop_back.points.len() - 1],
        );

        assert_eq!(
            first,
            (source.x + source.w / 2, source.y + source.h),
            "leaves downward"
        );
        assert_eq!(
            last,
            (target.x + target.w / 2, target.y + target.h),
            "arrives from below"
        );
        assert!(
            loop_back.points.iter().any(|p| p.1 >= drawing.height - 2),
            "runs in a lane"
        );
        assert!(loop_back.bends() <= 4);
    }

    #[test]
    fn a_route_turns_only_at_right_angles() {
        for seed in 1..12u64 {
            for route in build(&seeded(seed, 24, 50), Options::default()).routes {
                for pair in route.points.windows(2) {
                    let [a, b] = pair else { continue };
                    assert!(
                        a.0 == b.0 || a.1 == b.1,
                        "seed {seed}: {a:?}..{b:?} is diagonal"
                    );
                }
            }
        }
    }

    #[test]
    fn an_edge_between_neighbours_bends_twice_at_most() {
        for seed in 1..12u64 {
            let g = seeded(seed, 24, 50);
            let adj = Adjacency::of(&g);
            let acyclic = back_edges(&g, &adj);
            let ranked = rank(&g, &adj, &acyclic);

            for route in build(&g, Options::default()).routes {
                let Some(edge) = g.edge(route.edge) else {
                    continue;
                };
                let span = ranked
                    .rank(edge.to())
                    .saturating_sub(ranked.rank(edge.from()));
                let allowed = if span <= 1 { 2 } else { 4 };
                assert!(
                    route.bends() <= allowed,
                    "seed {seed}: a span of {span} bent {} times",
                    route.bends()
                );
            }
        }
    }

    #[test]
    fn the_drawing_is_the_same_every_run() {
        let g = seeded(0xD12E, 40, 90);
        let once = build(&g, Options::default());
        for _ in 0..8 {
            let again = build(&g, Options::default());
            assert_eq!(again.boxes, once.boxes);
            assert_eq!(again.routes, once.routes);
            assert_eq!((again.width, again.height), (once.width, once.height));
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
