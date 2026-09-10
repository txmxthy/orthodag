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
/// The ordering phase proposes several candidates; each is laid out in full and
/// scored, and the best drawing wins. See [`build_at`].
pub(crate) fn build(g: &Graph, options: Options) -> route::Layout {
    let Some(target) = options.width.and_then(|w| i32::try_from(w).ok()) else {
        return build_at(g, route::Style::natural(options));
    };

    // Walk the ladder and stop at the first rung that fits. If none do, keep
    // the narrowest: a drawing wider than asked for is still a drawing, and
    // clipping one would be worse than admitting it did not fit.
    let mut narrowest: Option<route::Layout> = None;
    for rung in route::Style::ladder(options) {
        let layout = build_at(g, rung);
        if layout.width <= target {
            return layout;
        }
        if narrowest
            .as_ref()
            .is_none_or(|held| layout.width < held.width)
        {
            narrowest = Some(layout);
        }
    }
    narrowest.unwrap_or_default()
}

/// One drawing at one rung of the ladder, chosen by drawing the candidates.
///
/// A barycenter sweep reads the layered graph, and the layered graph is not
/// what a reader sees: it cannot tell that a box hides a crossing, or that two
/// runs merged into one apparent line. So each ordering it proposed is placed,
/// routed and scored, and the drawing decides.
///
/// Tier before scalar, as everywhere: a candidate with fewer categorical
/// defects wins whatever it costs on the total, because a glyph that reads
/// wrong is not worse, it is broken. Ties keep the earliest candidate, which is
/// the one the sweeps reached first, so the choice is deterministic.
fn build_at(g: &Graph, style: route::Style) -> route::Layout {
    let adj = Adjacency::of(g);
    let acyclic = acyclic::back_edges(g, &adj);
    let ranked = rank::rank(g, &adj, &acyclic);
    let layered = layer::layer(g, &adj, &acyclic, &ranked);
    let hops = order::Hops::of(&layered);
    let interiors = port::interiors(g, &acyclic);

    let draw = |columns: &order::Columns, placed: &place::Placed| {
        let ports = port::rows(g, &acyclic, columns, placed);
        route::route(g, &acyclic, columns, placed, &ports, style)
    };
    // Tier before scalar: a categorical defect is not a large cost, it is a
    // different kind of thing, and no total buys one back.
    let worth = |layout: &route::Layout| {
        let score = crate::score::score(g, layout);
        (score.vocabulary().iter().sum::<usize>(), score.total)
    };

    let mut best: Option<((usize, i64), order::Columns)> = None;
    for columns in order::orderings(&layered).iter().take(drawn(g)) {
        let placed = settle(g, columns, &hops, &interiors, &draw, &worth);
        let key = worth(&draw(columns, &placed));
        if best.as_ref().is_none_or(|(held, _)| key < *held) {
            best = Some((key, columns.clone()));
        }
    }
    let Some((_, columns)) = best else {
        return route::Layout::default();
    };

    let columns = shuffle(g, &columns, &hops, &interiors, &draw, &worth);
    let placed = settle(g, &columns, &hops, &interiors, &draw, &worth);
    draw(&columns, &placed)
}

/// Swaps neighbours in a column while that makes the drawing better.
///
/// The sweeps propose orderings by reading the layered graph, and the layered
/// graph cannot see a crossing: whether two edges cross is a fact about the
/// drawing, and nothing counts one until a candidate has been drawn. So the
/// sweeps get the ordering close and this walks it the rest of the way, one
/// adjacent swap at a time, keeping a swap only when the drawing improves.
///
/// It runs on the winning ordering rather than on all of them. A swap costs a
/// place, a route and a score, and spending that on candidates already known to
/// be worse buys nothing.
///
/// Placement is left to its plain sweeps here rather than its own descent —
/// this is asking which order reads better, and settling every trial first
/// would multiply two searches together for an answer neither of them changes.
fn shuffle(
    g: &Graph,
    from: &order::Columns,
    hops: &order::Hops,
    interiors: &[usize],
    draw: &impl Fn(&order::Columns, &place::Placed) -> route::Layout,
    worth: &impl Fn(&route::Layout) -> (usize, i64),
) -> order::Columns {
    let mut columns = from.clone();
    let Some(passes) = swapped(g) else {
        return columns;
    };
    let placed = place::place(g, &columns, hops, interiors);
    let mut best = worth(&draw(&columns, &placed));

    for _ in 0..passes {
        let mut moved = false;
        for column in 0..columns.len() {
            let slots = columns.get(column).map_or(0, Vec::len);
            for at in 0..slots.saturating_sub(1) {
                let mut trial = columns.clone();
                if let Some(slots) = trial.get_mut(column) {
                    slots.swap(at, at + 1);
                }
                let placed = place::place(g, &trial, hops, interiors);
                let key = worth(&draw(&trial, &placed));
                if key < best {
                    best = key;
                    columns = trial;
                    moved = true;
                }
            }
        }
        if !moved {
            break;
        }
    }
    columns
}

/// How many passes of adjacent swaps a graph is worth, or `None` for none.
///
/// A pass draws the graph once per adjacent pair in every column, which is
/// roughly once per slot. Cheaper per trial than the placement descent, because
/// placement is not settled for each one, and worth more: crossings are what it
/// moves and crossings are the largest thing left.
fn swapped(g: &Graph) -> Option<usize> {
    match g.edges().len() {
        0..=48 => Some(4),
        49..=160 => Some(2),
        _ => None,
    }
}

/// Places one ordering, then hill-climbs the placement by drawing it.
///
/// Median placement gets the rows roughly right and has no way to correct a
/// lean, because a sweep only pushes down. So each column is offered up and
/// down a row or two, in turn, and a move is kept only when the drawing it
/// makes is better. Coordinate descent, with the objective as the only judge.
///
/// Passes stop early when a whole sweep of the columns changes nothing, which
/// is what a local minimum looks like from here.
fn settle(
    g: &Graph,
    columns: &order::Columns,
    hops: &order::Hops,
    interiors: &[usize],
    draw: &impl Fn(&order::Columns, &place::Placed) -> route::Layout,
    worth: &impl Fn(&route::Layout) -> (usize, i64),
) -> place::Placed {
    let mut placed = place::place(g, columns, hops, interiors);
    let Some(passes) = climbed(g) else {
        return placed;
    };
    let mut best = worth(&draw(columns, &placed));

    for _ in 0..passes {
        let mut moved = false;
        for column in 0..columns.len() {
            for delta in OFFERS {
                let trial = place::nudge(columns, &placed, column, delta);
                let key = worth(&draw(columns, &trial));
                if key < best {
                    best = key;
                    placed = trial;
                    moved = true;
                }
            }
        }
        if !moved {
            break;
        }
    }
    placed
}

/// How far a column is offered up and down, nearest first.
///
/// Two rows either way. One is too little to clear a box and three is far
/// enough that the drawing it makes is a different drawing rather than the same
/// one corrected — and the descent can reach three by taking two twice.
const OFFERS: [i32; 4] = [-1, 1, -2, 2];

/// How many passes of the descent a graph is worth, or `None` for none at all.
///
/// A pass draws the graph `columns × 4` times, on top of the candidate
/// orderings already being drawn, so this is the most expensive budget in the
/// library and it is the first one to run out.
fn climbed(g: &Graph) -> Option<usize> {
    match g.edges().len() {
        0..=48 => Some(3),
        49..=128 => Some(1),
        _ => None,
    }
}

/// How many candidate orderings are drawn before one is chosen.
///
/// Drawing a candidate is a place, a route and a score over every cell, so this
/// is the budget on the most expensive stage in the pipeline and it is
/// expressed in edges. The sweeps propose nine at most; a small graph can
/// afford all of them and a large one cannot afford two.
///
/// The numbers are where the drawing stops paying: past a few dozen edges the
/// orderings differ in places no single drawing is going to fix, and the gain
/// per candidate drawn falls off faster than the cost does.
fn drawn(g: &Graph) -> usize {
    match g.edges().len() {
        0..=48 => usize::MAX,
        49..=128 => 4,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::Adjacency;
    use super::{acyclic::back_edges, build, layer::layer, order, rank::rank};
    use crate::graph::{Graph, Node};
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
        // Seventeen: the initial order, plus at most one per sweep, for each
        // of the two readings.
        for seed in 1..24u64 {
            assert!(
                pipeline(&seeded(seed, 120, 300)).len() <= 17,
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

        // By which edge it is, not by what shape it came out: a forward edge
        // into the same box is free to bend too, and once it did this found it.
        let loop_back = drawing
            .routes
            .iter()
            .find(|r| g.edge(r.edge).map(|e| (e.from(), e.to())) == Some((ids[2], ids[1])))
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
