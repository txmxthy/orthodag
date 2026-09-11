//! Putting a number on a drawing.
//!
//! The scorer exists because "looks better" is not a thing a search can descend
//! and not a thing a test can assert. Every judgement the layout makes — which
//! ordering to keep, where to turn — is eventually a comparison between two
//! drawings, and this is what does the comparing.
//!
//! It works on the drawn cells, not on the graph. A crossing a box hides is not
//! a crossing; two runs that merge into one apparent line are one line. Neither
//! fact is visible in the layered graph, and both are visible here.

use std::fmt;

use crate::graph::{EdgeId, Graph};
use crate::layout::route::{Boxed, Layout, Route};
use crate::paint::grid::{D, L, R, U, walk};

/// What one edge left in one cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Ink {
    pub(crate) edge: EdgeId,
    pub(crate) bits: u8,
}

/// Every cell of a drawing, and which edges put what there.
///
/// Rasterised through the same walk the painter uses, so the numbers describe
/// the picture that actually gets printed rather than an idea of it.
/// Held flat: one run of ink for the whole drawing, and where each cell's share
/// of it starts.
///
/// A vector per cell is the obvious shape and it allocates once per drawn cell —
/// thousands of times for one drawing, and a drawing is made hundreds of times
/// for one graph. Holding a cell's ink inline instead was tried and is worse:
/// the cell grows, and the walk is over every cell of the frame whether or not
/// anything was drawn in it, so the reading costs more than the allocating saved.
///
/// So the ink goes in one run. Counting first says how much room each cell
/// needs, a running sum says where each one starts, and a second walk fills
/// them. Two passes over the routes, two allocations for the drawing, and a cell
/// is four bytes.
#[derive(Clone, Debug, Default)]
pub(crate) struct Raster {
    /// Where each cell's ink starts, with one extra on the end.
    starts: Vec<u32>,
    /// How much of its room each cell has taken.
    filled: Vec<u32>,
    ink: Vec<Ink>,
    width: usize,
    height: usize,
}

impl Raster {
    /// Rasterises every route of a drawing.
    pub(crate) fn of(layout: &Layout) -> Self {
        let width = usize::try_from(layout.width).unwrap_or(0);
        let height = usize::try_from(layout.height).unwrap_or(0);
        let cells = width * height;

        // An edge that touches a cell twice is counted twice here and stored
        // once, so this is room enough rather than room exactly.
        let mut starts = vec![0u32; cells + 1];
        let mut count = |x: i32, y: i32| {
            if let Some(at) = index_of(width, height, x, y)
                && let Some(slot) = starts.get_mut(at + 1)
            {
                *slot += 1;
            }
        };
        for route in &layout.routes {
            walk(&route.points, |x, y, _| count(x, y));
        }
        for at in 1..starts.len() {
            starts[at] += starts[at - 1];
        }

        let total = starts.last().copied().unwrap_or(0) as usize;
        let mut raster = Self {
            starts,
            filled: vec![0; cells],
            // Room, not content: `filled` says how much of each cell's share is
            // real and nothing reads past it, so what this is filled with is
            // never seen. Any edge of the drawing will do, and where there is no
            // edge there is no room either.
            ink: layout.routes.first().map_or_else(Vec::new, |route| {
                vec![
                    Ink {
                        edge: route.edge,
                        bits: 0
                    };
                    total
                ]
            }),
            width,
            height,
        };

        for route in &layout.routes {
            let edge = route.edge;
            let mut gained: Vec<(i32, i32, u8)> = Vec::new();
            walk(&route.points, |x, y, bits| gained.push((x, y, bits)));
            for (x, y, bits) in gained {
                raster.add(x, y, Ink { edge, bits });
            }
        }

        raster
    }

    /// One cell's ink.
    fn cell(&self, at: usize) -> &[Ink] {
        let (Some(from), Some(taken)) = (self.starts.get(at), self.filled.get(at)) else {
            return &[];
        };
        let from = *from as usize;
        self.ink
            .get(from..from + *taken as usize)
            .unwrap_or_default()
    }

    /// What is in one cell: one entry per edge that touched it.
    ///
    /// An edge that passes through a cell twice — which routing should not
    /// produce and the scorer should not hide — appears once, with the union of
    /// what it left.
    #[cfg(test)]
    pub(crate) fn at(&self, x: i32, y: i32) -> &[Ink] {
        self.index(x, y).map_or(&[], |at| self.cell(at))
    }

    /// Every cell that anything was drawn in.
    pub(crate) fn drawn(&self) -> impl Iterator<Item = (i32, i32, &[Ink])> {
        self.filled
            .iter()
            .enumerate()
            .filter(|(_, taken)| **taken > 0)
            .filter_map(move |(at, _)| {
                let width = self.width.max(1);
                let x = i32::try_from(at % width).ok()?;
                let y = i32::try_from(at / width).ok()?;
                Some((x, y, self.cell(at)))
            })
    }

    /// How many cells carry an edge glyph.
    pub(crate) fn ink(&self) -> usize {
        self.filled.iter().filter(|taken| **taken > 0).count()
    }

    fn add(&mut self, x: i32, y: i32, ink: Ink) {
        let Some(at) = self.index(x, y) else { return };
        let (Some(from), Some(taken)) =
            (self.starts.get(at).copied(), self.filled.get(at).copied())
        else {
            return;
        };
        let from = from as usize;
        let held = from..from + taken as usize;
        if let Some(found) = self
            .ink
            .get_mut(held)
            .and_then(|held| held.iter_mut().find(|at| at.edge == ink.edge))
        {
            found.bits |= ink.bits;
            return;
        }
        if let Some(slot) = self.ink.get_mut(from + taken as usize) {
            *slot = ink;
            if let Some(taken) = self.filled.get_mut(at) {
                *taken += 1;
            }
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        index_of(self.width, self.height, x, y)
    }
}

fn index_of(width: usize, height: usize, x: i32, y: i32) -> Option<usize> {
    let x = usize::try_from(x).ok().filter(|x| *x < width)?;
    let y = usize::try_from(y).ok().filter(|y| *y < height)?;
    Some(y * width + x)
}

/// What one edge cost, counted on the drawn cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct EdgeScore {
    pub(crate) edge: EdgeId,
    /// How many columns it crosses. One is a neighbour.
    pub(crate) span: usize,
    /// Right angles it turns through.
    pub(crate) bends: usize,
    /// The most it is allowed, given its span.
    pub(crate) allowed: usize,
    /// Distinct runs it shares with edges out of the same box.
    pub(crate) forks: usize,
    /// Distinct runs it shares with edges into the same box.
    pub(crate) joins: usize,
    /// Cells where it passes another edge at a right angle.
    pub(crate) crossings: usize,
    /// Cells it shares with an unrelated edge in any other way.
    pub(crate) overlaps: usize,
    /// Vertical cells travelled beyond the rows it actually had to cover.
    pub(crate) detour: i32,
    /// Bends beyond the fewest it could have had.
    pub(crate) jogs: usize,
}

/// How two edges came to share a cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shared {
    /// One straight through sideways, the other straight through downward.
    Crossing,
    /// They leave the same box.
    Fork,
    /// They arrive at the same box.
    Join,
    /// Anything else, which draws two edges as one line and lies to the reader.
    Overlap,
}

/// Scores every route of a drawing on its own.
pub(crate) fn per_edge(g: &Graph, layout: &Layout, raster: &Raster) -> Vec<EdgeScore> {
    let mut scores: Vec<EdgeScore> = layout
        .routes
        .iter()
        .map(|route| {
            let (span, allowed) = budget(g, layout, route.edge);
            EdgeScore {
                edge: route.edge,
                span,
                bends: route.bends(),
                allowed,
                jogs: route.bends().saturating_sub(fewest(route)),
                detour: detour(route),
                // Filled in below, from the cells the routes actually share.
                forks: 0,
                joins: 0,
                crossings: 0,
                overlaps: 0,
            }
        })
        .collect();

    let mut forks: Vec<Vec<(i32, i32)>> = vec![Vec::new(); scores.len()];
    let mut joins: Vec<Vec<(i32, i32)>> = vec![Vec::new(); scores.len()];

    // Which entry of `scores` an edge is, by its id. The loop below asks once
    // per edge per shared cell, and asking by search made the tally cost an edge
    // scan a question — quadratic in the edges, on exactly the busiest cells.
    let mut held_at = vec![usize::MAX; g.edges().len()];
    for (at, score) in scores.iter().enumerate() {
        if let Some(slot) = held_at.get_mut(score.edge.index()) {
            *slot = at;
        }
    }

    for (x, y, ink) in raster.drawn() {
        for (at, one) in ink.iter().enumerate() {
            for other in &ink[at + 1..] {
                let how = shared(g, *one, *other);
                for held in [one.edge, other.edge] {
                    let Some(index) = held_at
                        .get(held.index())
                        .copied()
                        .filter(|at| *at != usize::MAX)
                    else {
                        continue;
                    };
                    match how {
                        Shared::Crossing => scores[index].crossings += 1,
                        Shared::Overlap => scores[index].overlaps += 1,
                        Shared::Fork => forks[index].push((x, y)),
                        Shared::Join => joins[index].push((x, y)),
                    }
                }
            }
        }
    }

    for (at, score) in scores.iter_mut().enumerate() {
        score.forks = runs(&forks[at]);
        score.joins = runs(&joins[at]);
    }
    scores
}

/// How two edges sharing a cell should be read.
fn shared(g: &Graph, one: Ink, other: Ink) -> Shared {
    if crossing(one.bits, other.bits) {
        return Shared::Crossing;
    }
    let (Some(a), Some(b)) = (g.edge(one.edge), g.edge(other.edge)) else {
        return Shared::Overlap;
    };
    if a.from() == b.from() {
        Shared::Fork
    } else if a.to() == b.to() {
        Shared::Join
    } else {
        Shared::Overlap
    }
}

/// One straight through sideways and one straight through downward.
///
/// Nothing else counts. Two edges meeting at a corner are not passing each
/// other; they are drawn as one line turning, which is the defect, not the
/// exception.
fn crossing(one: u8, other: u8) -> bool {
    let flat = |bits: u8| bits == L | R;
    let upright = |bits: u8| bits == U | D;
    (flat(one) && upright(other)) || (upright(one) && flat(other))
}

/// How many separate runs a set of cells forms.
///
/// Cells that touch side to side or top to bottom are one run. An edge is
/// allowed one fork and one join — the branch off its source's trunk and the
/// merge onto its target's — and a second of either means the line is being
/// read as part of something it is not.
fn runs(cells: &[(i32, i32)]) -> usize {
    let mut held: Vec<(i32, i32)> = cells.to_vec();
    held.sort_unstable();
    held.dedup();

    // Sorted, so a neighbour is a binary search, and visited is a flag rather
    // than a removal — looking one up by scanning and taking it out by shifting
    // the rest made counting a fan's trunk quadratic in its length, and a trunk
    // is exactly the long thing here.
    let mut taken = vec![false; held.len()];
    let mut frontier: Vec<(i32, i32)> = Vec::new();
    let mut found = 0;

    for start in 0..held.len() {
        if taken.get(start) != Some(&false) {
            continue;
        }
        found += 1;
        if let Some(seed) = held.get(start).copied() {
            taken[start] = true;
            frontier.push(seed);
        }
        while let Some((x, y)) = frontier.pop() {
            for next in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                if let Ok(at) = held.binary_search(&next)
                    && taken.get(at) == Some(&false)
                {
                    taken[at] = true;
                    frontier.push(next);
                }
            }
        }
    }
    found
}

/// The columns an edge crosses, and the bends that buys it.
///
/// Two for a neighbour: out, across, in. Four for a skip, however far: a turn
/// at each end and a straight run through the middle.
fn budget(g: &Graph, layout: &Layout, edge: EdgeId) -> (usize, usize) {
    let span = g
        .edge(edge)
        .and_then(|e| Some((layout.boxed(e.from())?, layout.boxed(e.to())?)))
        .map_or(1, |(from, to)| to.column.saturating_sub(from.column));
    (span, if span <= 1 { 2 } else { 4 })
}

/// The fewest bends this route could have had: none if it stays on its row.
fn fewest(route: &Route) -> usize {
    match (route.points.first(), route.points.last()) {
        (Some(first), Some(last)) if first.1 == last.1 => 0,
        _ => 2,
    }
}

/// Vertical cells travelled beyond the rows the edge actually had to cover.
fn detour(route: &Route) -> i32 {
    let travelled: i32 = route
        .points
        .windows(2)
        .map(|p| (p[1].1 - p[0].1).abs())
        .sum();
    let (Some(first), Some(last)) = (route.points.first(), route.points.last()) else {
        return 0;
    };
    travelled - (last.1 - first.1).abs()
}

/// What a whole drawing is worth.
///
/// Three tiers, not one number, because they are not the same kind of thing.
///
/// **Vocabulary** is categorical. A glyph that reads wrong is not worse, it is
/// broken, and no amount of tidiness elsewhere buys it back. A candidate that
/// raises one of these is refused whatever it does to the total.
///
/// **Total** is the scalar a search descends: crossings, asymmetry, detour.
///
/// **Reported** is computed and deliberately left out of the objective, because
/// these are diagnostics rather than goals.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Score {
    /// Neighbouring edges that bend more than twice.
    pub bends_over_fwd: usize,
    /// Skipping edges that bend more than four times.
    pub bends_over_skip: usize,
    /// Edges touching more than one fork run or more than one join run.
    pub junction_over: usize,
    /// Cells shared by unrelated edges in any way but a crossing.
    pub overlaps: usize,
    /// Cells where two edges genuinely pass each other.
    pub cross_cells: usize,
    /// Cells where two flows are drawn as one line.
    ///
    /// A fork run carrying two colours is a trunk that two flows were bundled
    /// onto. The cell holds one character, so one of them is simply gone there
    /// — and unlike a crossing, which has to happen somewhere and reads as a
    /// crossing, this reads as one line, which is a different claim and a false
    /// one. `overlaps` charges exactly this lie for two unrelated edges; the
    /// only thing that let it through here is that these two share an endpoint.
    ///
    /// Counted apart from `mixed` because it is the avoidable half. Where two
    /// flows genuinely cross, a cell has to give up a colour and the bridge
    /// glyph says so.
    pub blends: usize,
    /// Cells where two flows of different colours meet, avoidable or not.
    ///
    /// A cell holds one character and therefore one colour, so where two
    /// colours land the painter has to drop one and the reader loses a flow.
    /// `design.md` §4.7 calls this a defect the layout is responsible for
    /// avoiding — it is reported rather than scored because avoiding it is the
    /// business of ports and tracks, and charging for it here would only tell a
    /// search to make the drawing smaller.
    pub mixed: usize,
    /// How far a fan leans off the row it should be symmetric about.
    pub asymmetry: i64,
    /// Vertical travel beyond what the rows required, over every edge.
    pub detour: i64,
    /// Cells carrying an edge glyph. Falls when two edges become one line.
    pub ink: usize,
    /// Bends beyond the fewest an edge could have had.
    pub jogs: usize,
    /// The size of the drawing.
    pub width: i32,
    /// The size of the drawing.
    pub height: i32,
    /// The weighted sum of everything the objective actually contains.
    pub total: i64,
}

impl Score {
    /// The categorical tier. Must be all zero, and zero is absorbing.
    pub fn vocabulary(&self) -> [usize; 4] {
        [
            self.bends_over_fwd,
            self.bends_over_skip,
            self.junction_over,
            self.overlaps,
        ]
    }

    /// The tier a search pushes down.
    pub fn soft(&self) -> i64 {
        i64::try_from(self.cross_cells).unwrap_or(i64::MAX) + self.asymmetry + self.detour
    }
}

impl Score {
    /// Every number, named, in a fixed order.
    ///
    /// One list feeding both the human line and the stored baseline, so a
    /// baseline can never describe a field the report does not show.
    pub fn fields(&self) -> [(&'static str, i64); 14] {
        let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
        [
            ("bends_fwd", count(self.bends_over_fwd)),
            ("bends_skip", count(self.bends_over_skip)),
            ("junctions", count(self.junction_over)),
            ("overlaps", count(self.overlaps)),
            ("cross", count(self.cross_cells)),
            ("blends", count(self.blends)),
            ("mixed", count(self.mixed)),
            ("asym", self.asymmetry),
            ("detour", self.detour),
            ("jogs", count(self.jogs)),
            ("ink", count(self.ink)),
            ("width", i64::from(self.width)),
            ("height", i64::from(self.height)),
            ("total", self.total),
        ]
    }

    /// The numbers as `key=value` pairs, which is what a baseline holds.
    pub fn record(&self) -> String {
        self.fields()
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl fmt::Display for Score {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (name, value) in self.fields() {
            let separator = if first { "" } else { " " };
            first = false;
            write!(f, "{separator}{name} {value}")?;
        }
        Ok(())
    }
}

/// Every defect in a drawing, with the cell and the pair responsible.
///
/// The same classification the counting uses, kept instead of tallied. One pass
/// over the raster; a cell with three edges in it reports every pair.
pub(crate) fn defects(g: &Graph, layout: &Layout) -> Vec<crate::defect::Defect> {
    use crate::defect::{Defect, Fault};

    let colours = crate::colour::of(g);
    let slot = |edge: EdgeId| colours.get(edge.index()).copied().flatten();
    let mut found = Vec::new();

    for (x, y, ink) in Raster::of(layout).drawn() {
        for (at, one) in ink.iter().enumerate() {
            for other in &ink[at + 1..] {
                let how = shared(g, *one, *other);
                let parted = slot(one.edge) != slot(other.edge);
                let fault = match how {
                    Shared::Crossing => Fault::Crossing,
                    Shared::Overlap => Fault::Overlap,
                    Shared::Fork | Shared::Join if parted => Fault::Blend,
                    // A trunk two branches of one flow share is the shape a fan
                    // is supposed to read as. Nothing is wrong here.
                    Shared::Fork | Shared::Join => continue,
                };
                let (lo, hi) = if one.edge <= other.edge {
                    (one.edge, other.edge)
                } else {
                    (other.edge, one.edge)
                };
                found.push(Defect {
                    at: (x, y),
                    fault,
                    edges: (lo, hi),
                });
            }
        }
    }
    found
}

/// The cells where two unrelated edges genuinely pass each other.
///
/// The painter asks for these so a bridge can be drawn, and it asks *here*
/// rather than working them out for itself: what counts as a crossing is the
/// objective's business, and a painter that disagreed with the scorer about it
/// would draw a bridge over something the numbers called an overlap.
pub(crate) fn crossing_cells(g: &Graph, layout: &Layout) -> Vec<Crossed> {
    cells_where_crossing(g, layout, false)
}

/// A cell two edges pass through, and the colour of the one going down it.
///
/// The painter needs the colour as well as the place. Cutting the horizontal
/// leaves the vertical as the only line drawn there, so the cell stops holding
/// two inks and starts holding one — and without being told which, it would
/// keep the "two colours met here" it was stained with and come out in the
/// caller's default.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Crossed {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) down: Option<crate::colour::Colour>,
}

/// The crossing cells where the two runs are different colours.
///
/// A junction glyph says "these are one line"; where the two runs belong to
/// different flows that is a lie, and the cell can only hold one of the two
/// colours anyway. Where they are the same colour there is nothing to tell
/// apart and the junction is the honest glyph.
pub(crate) fn parted_crossing_cells(g: &Graph, layout: &Layout) -> Vec<Crossed> {
    cells_where_crossing(g, layout, true)
}

fn cells_where_crossing(g: &Graph, layout: &Layout, parted: bool) -> Vec<Crossed> {
    let colours = crate::colour::of(g);
    let slot = |edge: EdgeId| colours.get(edge.index()).copied().flatten();

    Raster::of(layout)
        .drawn()
        .filter_map(|(x, y, ink)| {
            let crossed = ink.iter().enumerate().any(|(at, one)| {
                ink[at + 1..].iter().any(|other| {
                    crossing(one.bits, other.bits)
                        && (!parted || slot(one.edge) != slot(other.edge))
                })
            });
            if !crossed {
                return None;
            }
            // The one that keeps its glyph is the one running down the cell.
            let down = ink
                .iter()
                .find(|held| held.bits & (U | D) != 0)
                .and_then(|held| slot(held.edge));
            Some(Crossed { x, y, down })
        })
        .collect()
}

/// Scores a drawing.
pub(crate) fn score(g: &Graph, layout: &Layout) -> Score {
    let raster = Raster::of(layout);
    let edges = per_edge(g, layout, &raster);

    let over = |pick: fn(&EdgeScore) -> bool| edges.iter().filter(|e| pick(e)).count();
    let bends_over_fwd = over(|e| e.span <= 1 && e.bends > e.allowed);
    let bends_over_skip = over(|e| e.span > 1 && e.bends > e.allowed);
    let junction_over = over(|e| e.forks > 1 || e.joins > 1);
    let overlaps = edges.iter().map(|e| e.overlaps).sum::<usize>();

    let (cross_cells, blends, mixed) = shared_cells(g, &raster);
    let asymmetry = asymmetry(g, layout);
    let detour: i64 = edges.iter().map(|e| i64::from(e.detour)).sum();

    let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    let total = 10 * count(bends_over_fwd + bends_over_skip + junction_over)
        + 5 * count(overlaps)
        + 3 * count(cross_cells)
        + 2 * asymmetry
        + detour;

    Score {
        bends_over_fwd,
        bends_over_skip,
        junction_over,
        overlaps,
        cross_cells,
        blends,
        mixed,
        asymmetry,
        detour,
        ink: raster.ink(),
        jogs: edges.iter().map(|e| e.jogs).sum(),
        width: layout.width,
        height: layout.height,
        total,
    }
}

/// The three things a cell can be guilty of, counted in one walk.
///
/// Each is a question about one cell — does anything cross here, is a trunk
/// carrying two flows, did any two inks meet — and each used to be asked in a
/// pass of its own over every cell of the drawing. The drawing is scored
/// hundreds of times per graph and the walk is the width times the height
/// whether or not anything was drawn, so asking all three at once is three
/// walks saved out of four.
fn shared_cells(g: &Graph, raster: &Raster) -> (usize, usize, usize) {
    let colours = crate::colour::of(g);
    let slot = |edge: EdgeId| colours.get(edge.index()).copied().flatten();
    let (mut crossed, mut blended, mut mixed) = (0, 0, 0);

    for (_, _, ink) in raster.drawn() {
        let (mut is_crossed, mut is_blend, mut is_mixed) = (false, false, false);
        for (at, one) in ink.iter().enumerate() {
            for other in &ink[at + 1..] {
                let parted = slot(one.edge) != slot(other.edge);
                if crossing(one.bits, other.bits) {
                    is_crossed = true;
                } else if parted && matches!(shared(g, *one, *other), Shared::Fork | Shared::Join) {
                    is_blend = true;
                }
                is_mixed |= parted;
            }
        }
        crossed += usize::from(is_crossed);
        blended += usize::from(is_blend);
        mixed += usize::from(is_mixed);
    }
    (crossed, blended, mixed)
}

/// How far each box's fan leans off the row it should be symmetric about.
///
/// A fan out of one box should sit around that box's row: three branches above
/// and one below is a fan that looks pulled. Summing the signed offsets and
/// taking the size of the sum says exactly that, and says nothing at all about
/// a fan that is even.
///
/// Which routes belong to which box comes from the graph, not from where the
/// route happens to start. Every box in a column shares an x, so matching on
/// coordinates gives each of them the whole column's edges.
///
/// An edge through a lane is not part of the fan. It leaves the bottom of its
/// source and comes up into the bottom of its target, which is a different
/// vocabulary from the branches that leave a box sideways, and counting it as a
/// branch makes the fan lean by however deep the lane is. Left in, it buys the
/// symmetry back by bending a forward edge that had no reason to bend.
fn asymmetry(g: &Graph, layout: &Layout) -> i64 {
    let mut total = 0;
    for boxed in &layout.boxes {
        let row = i64::from(boxed.y + (boxed.h - 1) / 2);
        let (mut out, mut fans_out) = (0, 0);
        let (mut into, mut fans_in) = (0, 0);
        for route in &layout.routes {
            let Some(edge) = g.edge(route.edge) else {
                continue;
            };
            if edge.from() == boxed.node
                && !through_lane(route.points.first(), boxed)
                && let Some(y) = leaving(route)
            {
                out += i64::from(y) - row;
                fans_out += 1;
            }
            if edge.to() == boxed.node
                && !through_lane(route.points.last(), boxed)
                && let Some(y) = arriving(route)
            {
                into += i64::from(y) - row;
                fans_in += 1;
            }
        }
        // One edge is not a fan and cannot lean. Charging it would make every
        // ordinary turn look like a defect.
        if fans_out > 1 {
            total += out.abs();
        }
        if fans_in > 1 {
            total += into.abs();
        }
    }
    total
}

/// Whether an edge meets this box below it, which is what a lane looks like.
///
/// A forward edge attaches on one of the box's own rows; an edge through a lane
/// attaches at the bottom border and drops out of it. Read off the drawing
/// rather than asked of the graph, so a drawing this library did not make is
/// judged by the same rule.
fn through_lane(end: Option<&(i32, i32)>, boxed: &Boxed) -> bool {
    end.is_some_and(|(_, y)| *y >= boxed.y + boxed.h - 1)
}

/// The row an edge is on once it has turned away from its source.
fn leaving(route: &Route) -> Option<i32> {
    let start = route.points.first()?.1;
    Some(
        route
            .points
            .iter()
            .find(|p| p.1 != start)
            .map_or(start, |p| p.1),
    )
}

/// The row an edge is on before it turns in towards its target.
fn arriving(route: &Route) -> Option<i32> {
    let end = route.points.last()?.1;
    Some(
        route
            .points
            .iter()
            .rev()
            .find(|p| p.1 != end)
            .map_or(end, |p| p.1),
    )
}

#[cfg(test)]
mod tests {
    /// Two flows meeting in one cell cost a colour; two edges of one flow do not.
    ///
    /// A cell holds one character, so where a red run and a blue run land the
    /// reader loses one of them. Where two runs of the *same* tag set land they
    /// are one line as far as a reader is concerned, which is the whole point of
    /// keying colour on the tag set, and nothing is lost.
    #[test]
    fn mixed_counts_colours_not_edges() {
        use crate::graph::{Graph, Node};

        let mut same = Graph::new();
        let ids: Vec<_> = (0..3)
            .map(|i| same.add_node(Node::new(format!("n{i}"))))
            .collect();
        let mut apart = same.clone();

        same.add_tagged_edge(ids[0], ids[2], ["t"]);
        same.add_tagged_edge(ids[1], ids[2], ["t"]);
        apart.add_tagged_edge(ids[0], ids[2], ["red"]);
        apart.add_tagged_edge(ids[1], ids[2], ["blue"]);

        assert_eq!(
            crate::score(&same).mixed,
            0,
            "one flow into one box cannot lose a colour"
        );
        assert!(
            crate::score(&apart).mixed >= crate::score(&same).mixed,
            "two flows can only cost more"
        );
    }

    use super::*;
    use crate::graph::{Graph, Node};
    use crate::layout;

    fn drawing(nodes: &[&str], edges: &[(usize, usize)]) -> (Graph, Layout) {
        let mut g = Graph::new();
        let ids: Vec<_> = nodes.iter().map(|n| g.add_node(Node::new(*n))).collect();
        for &(a, b) in edges {
            g.add_edge(ids[a], ids[b]);
        }
        let layout = layout::build(&g, crate::options::Options::default());
        (g, layout)
    }

    #[test]
    fn a_straight_edge_leaves_one_run_of_ink() {
        let (_, layout) = drawing(&["a", "b"], &[(0, 1)]);
        let raster = Raster::of(&layout);
        let cells: Vec<_> = raster.drawn().collect();

        assert!(!cells.is_empty());
        for (_, _, ink) in cells {
            assert_eq!(ink.len(), 1, "nothing shares a cell in a two-box drawing");
            assert_eq!(
                ink[0].bits & !(L | R),
                0,
                "a straight edge only goes sideways"
            );
        }
    }

    #[test]
    fn a_fan_shares_the_cells_where_its_branches_meet() {
        let (_, layout) = drawing(&["a", "x", "y", "z"], &[(0, 1), (0, 2), (0, 3)]);
        let raster = Raster::of(&layout);
        let shared = raster.drawn().filter(|(_, _, ink)| ink.len() > 1).count();
        assert!(
            shared > 0,
            "three branches out of one box must share their trunk"
        );
    }

    #[test]
    fn ink_counts_cells_not_edges() {
        let (_, layout) = drawing(&["a", "x", "y", "z"], &[(0, 1), (0, 2), (0, 3)]);
        let raster = Raster::of(&layout);
        let per_edge: usize = raster.drawn().map(|(_, _, ink)| ink.len()).sum();
        assert!(
            raster.ink() < per_edge,
            "a shared cell is one cell, however many edges are in it"
        );
    }

    #[test]
    fn a_cell_holds_one_entry_per_edge_however_often_it_is_touched() {
        let (_, layout) = drawing(&["a", "b", "c", "d"], &[(0, 1), (1, 2), (2, 3), (0, 3)]);
        let raster = Raster::of(&layout);
        for (x, y, ink) in raster.drawn() {
            let mut seen: Vec<EdgeId> = ink.iter().map(|i| i.edge).collect();
            seen.sort_unstable();
            let count = seen.len();
            seen.dedup();
            assert_eq!(seen.len(), count, "{x},{y} lists an edge twice");
        }
    }

    #[test]
    fn a_turn_leaves_both_directions_in_the_corner_cell() {
        let (_, layout) = drawing(&["a", "x", "y"], &[(0, 1), (0, 2)]);
        let raster = Raster::of(&layout);
        let corners = raster
            .drawn()
            .filter(|(_, _, ink)| {
                ink.iter()
                    .any(|i| i.bits & (U | D) != 0 && i.bits & (L | R) != 0)
            })
            .count();
        assert!(corners > 0, "a fan has to turn somewhere");
    }

    #[test]
    fn nothing_is_rastered_outside_the_drawing() {
        let (_, layout) = drawing(&["a", "b", "c"], &[(0, 1), (1, 2)]);
        let raster = Raster::of(&layout);
        for (x, y, _) in raster.drawn() {
            assert!(x >= 0 && x < layout.width && y >= 0 && y < layout.height);
        }
        assert_eq!(raster.at(-1, -1), []);
        assert_eq!(raster.at(layout.width, 0), []);
    }

    fn edges(nodes: &[&str], links: &[(usize, usize)]) -> Vec<EdgeScore> {
        let (g, layout) = drawing(nodes, links);
        per_edge(&g, &layout, &Raster::of(&layout))
    }

    #[test]
    fn a_chain_costs_nothing() {
        for score in edges(&["a", "b", "c"], &[(0, 1), (1, 2)]) {
            assert_eq!(score.bends, 0);
            assert_eq!(score.jogs, 0);
            assert_eq!(score.detour, 0);
            assert_eq!((score.forks, score.joins), (0, 0));
            assert_eq!((score.crossings, score.overlaps), (0, 0));
            assert_eq!((score.span, score.allowed), (1, 2));
        }
    }

    #[test]
    fn a_branch_of_a_fan_forks_once_and_joins_never() {
        for score in edges(&["a", "x", "y", "z"], &[(0, 1), (0, 2), (0, 3)]) {
            assert!(
                score.forks <= 1,
                "a branch may leave its trunk once, not {}",
                score.forks
            );
            assert_eq!(score.joins, 0);
            assert_eq!(
                score.overlaps, 0,
                "a fan's own branches are never an overlap"
            );
        }
    }

    #[test]
    fn a_branch_of_a_fan_in_joins_once_and_forks_never() {
        for score in edges(&["x", "y", "z", "a"], &[(0, 3), (1, 3), (2, 3)]) {
            assert!(score.joins <= 1);
            assert_eq!(score.forks, 0);
            assert_eq!(score.overlaps, 0);
        }
    }

    #[test]
    fn a_skip_is_allowed_twice_the_bends_of_a_neighbour() {
        let scored = edges(&["a", "b", "c", "d"], &[(0, 1), (1, 2), (2, 3), (0, 3)]);
        let long = scored.last().expect("the skip is the last edge");
        assert_eq!((long.span, long.allowed), (3, 4));
        assert!(scored[..3].iter().all(|s| s.allowed == 2));
    }

    #[test]
    fn two_edges_at_a_right_angle_are_a_crossing_and_not_an_overlap() {
        assert!(crossing(L | R, U | D));
        assert!(crossing(U | D, L | R));
        assert!(!crossing(L | R, L | R));
        assert!(!crossing(L | D, U | D), "a corner is not passing through");
        assert!(
            !crossing(L | R | U, U | D),
            "a tee is a junction, not a crossing"
        );
    }

    #[test]
    fn touching_cells_are_one_run_and_separated_cells_are_two() {
        assert_eq!(runs(&[]), 0);
        assert_eq!(runs(&[(0, 0), (1, 0), (1, 1)]), 1);
        assert_eq!(runs(&[(0, 0), (0, 0)]), 1);
        assert_eq!(runs(&[(0, 0), (5, 5)]), 2);
        assert_eq!(runs(&[(0, 0), (1, 0), (4, 0), (5, 0)]), 2);
    }

    #[test]
    fn two_unrelated_edges_in_one_cell_any_other_way_are_an_overlap() {
        let mut g = Graph::new();
        let ids: Vec<_> = (0..4)
            .map(|i| g.add_node(Node::new(format!("n{i}"))))
            .collect();
        let one = g.add_edge(ids[0], ids[1]);
        let other = g.add_edge(ids[2], ids[3]);
        let flat = |edge| Ink { edge, bits: L | R };

        assert_eq!(shared(&g, flat(one), flat(other)), Shared::Overlap);
        assert_eq!(
            shared(
                &g,
                flat(one),
                Ink {
                    edge: other,
                    bits: U | D
                }
            ),
            Shared::Crossing,
            "at a right angle they pass; anywhere else they are one line"
        );
    }

    #[test]
    fn the_diamond_stays_inside_the_vocabulary_and_pays_for_it() {
        // The frame looks like the skip edge lands on another line. It does not:
        // everything converging there arrives at the same box, so those cells
        // are joins. What the skip does cost is the long way round.
        let scored = edges(
            &["in", "split", "even", "odd", "sink"],
            &[(0, 1), (1, 2), (1, 3), (2, 4), (3, 4), (0, 4)],
        );
        assert!(
            scored.iter().all(|s| s.overlaps == 0),
            "no overlaps here: {scored:?}"
        );
        assert!(scored.iter().all(|s| s.bends <= s.allowed));
        assert!(scored.iter().all(|s| s.forks <= 1 && s.joins <= 1));

        let skip = scored.last().expect("the skip is the last edge");
        assert!(
            skip.detour > 0 && skip.jogs > 0,
            "the skip should be paying something"
        );
    }

    fn scored(nodes: &[&str], links: &[(usize, usize)]) -> Score {
        let (g, layout) = drawing(nodes, links);
        score(&g, &layout)
    }

    #[test]
    fn a_chain_is_worth_nothing_at_all() {
        let s = scored(&["a", "b", "c"], &[(0, 1), (1, 2)]);
        assert_eq!(s.vocabulary(), [0, 0, 0, 0]);
        assert_eq!(s.soft(), 0);
        assert_eq!(s.total, 0);
        assert!(s.ink > 0, "there is a line there");
    }

    #[test]
    fn an_even_fan_is_not_charged_for_being_a_fan() {
        let s = scored(&["a", "x", "y", "z"], &[(0, 1), (0, 2), (0, 3)]);
        assert_eq!(s.vocabulary(), [0, 0, 0, 0]);
        assert_eq!(
            s.asymmetry, 0,
            "three branches centred on the source lean nowhere"
        );
    }

    #[test]
    fn a_fan_pulled_to_one_side_is_charged_for_it() {
        // Two branches, and a third box in the second column that nothing feeds,
        // so the fan cannot sit centred on its source.
        let mut g = Graph::new();
        let source = g.add_node(Node::new("a"));
        let ids: Vec<_> = ["x", "y", "z"]
            .iter()
            .map(|n| g.add_node(Node::new(*n)))
            .collect();
        let spare = g.add_node(Node::new("spare"));
        g.add_edge(spare, ids[0]);
        g.add_edge(source, ids[1]);
        g.add_edge(source, ids[2]);
        let s = score(
            &g,
            &crate::layout::build(&g, crate::options::Options::default()),
        );
        assert!(s.asymmetry >= 0);
    }

    #[test]
    fn the_total_is_the_weights_it_says_it_is() {
        let s = scored(
            &["in", "split", "even", "odd", "sink"],
            &[(0, 1), (1, 2), (1, 3), (2, 4), (3, 4), (0, 4)],
        );
        let count = |n: usize| i64::try_from(n).unwrap();
        let vocabulary: i64 = s.vocabulary()[..3].iter().copied().map(count).sum();
        let want = 10 * vocabulary
            + 5 * count(s.overlaps)
            + 3 * count(s.cross_cells)
            + 2 * s.asymmetry
            + s.detour;
        assert_eq!(s.total, want);
    }

    #[test]
    fn the_vocabulary_tier_is_separate_from_the_scalar() {
        let s = Score {
            overlaps: 1,
            ..Score::default()
        };
        assert_eq!(s.vocabulary(), [0, 0, 0, 1]);
        assert_eq!(s.soft(), 0, "an overlap is not a soft cost, it is a defect");
    }

    #[test]
    fn a_score_reads_as_one_line() {
        let line = scored(&["a", "b"], &[(0, 1)]).to_string();
        assert!(!line.contains('\n'));
        assert!(line.contains("total 0"), "{line}");
    }

    #[test]
    fn a_record_says_the_same_things_the_line_does() {
        let s = scored(&["a", "x", "y"], &[(0, 1), (0, 2)]);
        let record = s.record();
        for (name, value) in s.fields() {
            assert!(
                record.contains(&format!("{name}={value}")),
                "{name} missing from {record}"
            );
        }
        assert_eq!(record.split(' ').count(), s.fields().len());
    }

    #[test]
    fn every_stored_shape_is_inside_the_vocabulary() {
        for (name, nodes, links) in [
            ("chain", &["a", "b", "c"][..], &[(0, 1), (1, 2)][..]),
            ("fan out", &["a", "x", "y", "z"], &[(0, 1), (0, 2), (0, 3)]),
            ("fan in", &["x", "y", "z", "a"], &[(0, 3), (1, 3), (2, 3)]),
            (
                "diamond",
                &["a", "b", "c", "d"],
                &[(0, 1), (0, 2), (1, 3), (2, 3)],
            ),
            (
                "skip",
                &["a", "b", "c", "d"],
                &[(0, 1), (1, 2), (2, 3), (0, 3)],
            ),
        ] {
            let s = scored(nodes, links);
            assert_eq!(s.vocabulary(), [0, 0, 0, 0], "{name}: {s}");
        }
    }

    fn fan_in(tags: Option<&str>) -> Score {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        let sink = g.add_node(Node::new("sink"));
        for source in [a, b] {
            match tags {
                Some(tag) => g.add_tagged_edge(source, sink, [tag]),
                None => g.add_edge(source, sink),
            };
        }
        score(
            &g,
            &crate::layout::build(&g, crate::options::Options::default()),
        )
    }

    /// Two edges into one box, carrying the tag sets given.
    fn into_one(one: &str, other: &str) -> Score {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        let sink = g.add_node(Node::new("sink"));
        g.add_tagged_edge(a, sink, [one]);
        g.add_tagged_edge(b, sink, [other]);
        score(
            &g,
            &crate::layout::build(&g, crate::options::Options::default()),
        )
    }

    #[test]
    fn edges_sharing_a_tag_set_and_a_target_are_drawn_as_one_line() {
        // One attach row means one track means one trunk with a branch off it,
        // rather than two lines side by side. It falls out of the track rule and
        // the port rule together; this is here so it stays a rule rather than an
        // accident. Two different tag sets get rows of their own and cannot
        // merge, which is what makes the comparison mean anything.
        let merged = into_one("x", "x");
        let apart = into_one("x", "y");
        assert!(
            merged.ink < apart.ink,
            "one flow should cost less ink than two: {} against {}",
            merged.ink,
            apart.ink
        );
    }

    #[test]
    fn a_merge_is_free_whether_or_not_the_edges_are_tagged() {
        // Two edges into one box merge because they arrive on one row, and an
        // untagged pair shares the empty set's row just as a tagged pair shares
        // its own. Neither is charged for the other's line.
        assert_eq!(fan_in(None).ink, fan_in(Some("x")).ink);
        assert_eq!(fan_in(None).vocabulary(), [0, 0, 0, 0]);
        assert_eq!(fan_in(Some("x")).vocabulary(), [0, 0, 0, 0]);
    }

    #[test]
    fn ink_is_the_only_metric_that_sees_a_merge_as_a_win() {
        // Every other number is per edge, so two edges drawn as one line each
        // pay for the whole path and a merge reads as a regression. Ink counts
        // drawn cells. This is why it is reported and not in the objective, and
        // why the runbook says to judge a merge by it.
        let merged = fan_in(Some("x"));
        let raster = {
            let mut g = Graph::new();
            let a = g.add_node(Node::new("a"));
            let b = g.add_node(Node::new("b"));
            let sink = g.add_node(Node::new("sink"));
            g.add_tagged_edge(a, sink, ["x"]);
            g.add_tagged_edge(b, sink, ["x"]);
            Raster::of(&crate::layout::build(
                &g,
                crate::options::Options::default(),
            ))
        };
        let charged: usize = raster.drawn().map(|(_, _, ink)| ink.len()).sum();
        assert!(
            charged > merged.ink,
            "the per-edge view charges twice for a shared cell"
        );
    }

    #[test]
    fn an_empty_drawing_rasters_to_nothing() {
        let raster = Raster::of(&Layout::default());
        assert_eq!(raster.ink(), 0);
        assert_eq!(raster.drawn().count(), 0);
    }
}
