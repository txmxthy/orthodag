//! Giving every slot a row.
//!
//! Ordering said what comes before what inside a column. Placement says where,
//! in cell coordinates, which is what decides whether an edge runs straight or
//! bends twice to reach a box three rows away.
//!
//! Alternating median sweeps: a slot wants the median row of its neighbours in
//! the column beside it, and gets as close to that as its neighbours in its own
//! column allow. Median rather than mean because a fan of five into one box
//! should put the box opposite the middle branch, not opposite the average of
//! branches that may be nowhere near it.

use super::layer::Slot;
use super::order::{Columns, Hops};
use crate::graph::{EdgeId, Graph, NodeId};

/// Blank rows between two stacked slots.
const GAP: i32 = 1;

/// How many times to sweep before taking what there is.
///
/// Median placement converges fast and then oscillates by a row or two; four
/// passes is past the useful part of that. The budget on a stage that scales
/// with the graph.
const SWEEPS: usize = 4;

/// The top row and height of every slot, indexed the way the columns are.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Placed {
    tops: Vec<Vec<i32>>,
    heights: Vec<Vec<i32>>,
}

impl Placed {
    /// The top row of one slot.
    pub(crate) fn top(&self, column: usize, at: usize) -> i32 {
        self.tops
            .get(column)
            .and_then(|c| c.get(at))
            .copied()
            .unwrap_or(0)
    }

    /// How many rows one slot occupies.
    pub(crate) fn height_of(&self, column: usize, at: usize) -> i32 {
        self.heights
            .get(column)
            .and_then(|c| c.get(at))
            .copied()
            .unwrap_or(1)
    }

    /// The row an edge meets this slot on.
    pub(crate) fn middle(&self, column: usize, at: usize) -> i32 {
        self.top(column, at) + (self.height_of(column, at) - 1) / 2
    }

    /// The row past the last one this slot occupies.
    fn bottom(&self, column: usize, at: usize) -> i32 {
        self.top(column, at) + self.height_of(column, at)
    }

    /// How tall the drawing is: one past the lowest row anything occupies.
    pub(crate) fn height(&self) -> i32 {
        let mut height = 0;
        for (column, slots) in self.tops.iter().enumerate() {
            for at in 0..slots.len() {
                height = height.max(self.bottom(column, at));
            }
        }
        height
    }
}

/// How tall a box is: the two borders, and an interior big enough for both the
/// text and the attach rows its flows need — or `floor`, if the caller asked
/// for boxes no shorter than that.
fn node_height(g: &Graph, interiors: &[usize], floor: i32, slot: Slot) -> i32 {
    match slot {
        Slot::Node(id) => {
            let lines = g.node(id).map_or(0, |n| n.lines().len());
            let text = 1 + i32::try_from(lines).unwrap_or(i32::MAX - 3);
            let ports = interiors.get(id.index()).copied().unwrap_or(1);
            (2 + text.max(i32::try_from(ports).unwrap_or(1))).max(floor)
        }
        Slot::Pass(_) => 1,
    }
}

/// Places every slot, sweeping medians until the budget runs out.
/// Whether two long edges of one flow into one box share a row.
///
/// Merging is what `design.md` §3 asks for and it is not always possible: on a
/// dense graph, forcing two chains onto one row can leave a third nowhere legal
/// to go, and an edge drawn on top of an unrelated one is a categorical defect
/// that no tidiness buys back. So it is offered and the drawing decides.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Merge {
    /// One flow into one box, one row.
    Flows,
    /// Every chain its own row, which always has an answer.
    Apart,
}

pub(crate) fn place(
    g: &Graph,
    columns: &Columns,
    hops: &Hops,
    interiors: &[usize],
    floor: i32,
    merge: Merge,
) -> Placed {
    let heights: Vec<Vec<i32>> = columns
        .iter()
        .map(|c| {
            c.iter()
                .map(|s| node_height(g, interiors, floor, *s))
                .collect()
        })
        .collect();
    let height = heights.iter().map(|c| natural(c)).max().unwrap_or(0);
    let mut placed = Placed {
        tops: stacked(&heights),
        heights,
    };

    for pass in 0..SWEEPS {
        // Left to right on even passes, right to left on odd ones: a slot's
        // neighbours on the side already placed are the ones worth aiming at.
        let order: Vec<usize> = if pass % 2 == 0 {
            (0..columns.len()).collect()
        } else {
            (0..columns.len()).rev().collect()
        };
        for column in order {
            let wanted = desired(columns, hops, &placed, column, pass % 2 == 0);
            settle(&mut placed, column, &wanted);
        }
        normalise(&mut placed);
        press(&mut placed, height);
    }

    normalise(&mut placed);
    straighten(g, columns, &mut placed, merge);
    normalise(&mut placed);
    placed
}

/// The same placement with one column of boxes moved `delta` rows.
///
/// Sweeping only ever pushes a slot down, and `press` only pulls one up when
/// its column will not otherwise fit, so a drawing has no way back to centre:
/// fans come out leaning the way the sweep happened to leave them. This is the
/// move that gives it one — and it is offered rather than applied, because
/// whether a lean is wrong is a question about the drawing, so the caller
/// answers it by drawing this and scoring it.
///
/// Every box in the column moves together, so they cannot collide with each
/// other; what changes is where they sit relative to the columns either side.
/// The placeholders are laid again afterwards, since the rows they were
/// straightened onto were chosen around the boxes that just moved.
pub(crate) fn nudge(
    g: &Graph,
    columns: &Columns,
    placed: &Placed,
    column: usize,
    delta: i32,
    merge: Merge,
) -> Placed {
    let mut moved = placed.clone();
    if let (Some(slots), Some(tops)) = (columns.get(column), moved.tops.get_mut(column)) {
        for (at, slot) in slots.iter().enumerate() {
            if matches!(slot, Slot::Node(_))
                && let Some(top) = tops.get_mut(at)
            {
                *top += delta;
            }
        }
    }
    normalise(&mut moved);
    straighten(g, columns, &mut moved, merge);
    normalise(&mut moved);
    moved
}

/// Puts every placeholder of one edge on one row, so a long edge cannot bend.
///
/// The literature gets straight chains of placeholders by aligning them
/// afterwards, which is a heuristic and can fail. Here the chain has one row for
/// every column it passes, chosen once, so straightness is not something the
/// layout achieves — it is something it cannot avoid.
///
/// The price is that a placeholder has no row of its own to be moved to, which
/// rules out layering an alignment pass on top later. That is the trade, and it
/// is worth it: a long edge that kinks in the middle of a drawing reads as two
/// edges.
///
/// Rows are handed out longest chain first, because a chain crossing six
/// columns has the least freedom and should not be left with what is left.
fn straighten(g: &Graph, columns: &Columns, placed: &mut Placed, merge: Merge) {
    let mut chains = match merge {
        Merge::Flows => flows(g, columns),
        Merge::Apart => chains(columns),
    };
    chains.sort_by_key(|(edge, cells)| (std::cmp::Reverse(cells.len()), *edge));

    // One row of slack per chain is enough for every chain to find a row of its
    // own even if the boxes leave none.
    let rows = placed.height() + i32::try_from(chains.len()).unwrap_or(0) + 1;
    let mut taken = occupied(columns, placed, rows);

    for (_, cells) in chains {
        let mut wanted: Vec<i32> = cells.iter().map(|(c, at)| placed.middle(*c, *at)).collect();
        let Some(target) = median(&mut wanted) else {
            continue;
        };
        let row = free_row(&taken, &cells, target, rows);
        for (column, at) in cells {
            if let Some(top) = placed.tops.get_mut(column).and_then(|c| c.get_mut(at)) {
                *top = row;
            }
            if let Some(cell) = taken.get_mut(column).and_then(|c| c.get_mut(index(row))) {
                *cell = true;
            }
        }
    }
}

/// The placeholders of each long edge, gathered by the flow they belong to.
///
/// Two edges carrying the same tags into the same box are one line as far as a
/// reader is concerned — `design.md` §3 — so their placeholders want one row
/// between them, not a row each. Laid separately they do the opposite: the
/// first takes a row, marks it taken, and the second is pushed off it, so one
/// line is drawn as two that then have to cross to reach the same door.
///
/// Their cells are merged into one chain here, which gives the group a single
/// row wherever any of them passes and makes the overlap literal — the same
/// cells, and so one line.
fn flows(g: &Graph, columns: &Columns) -> Vec<(EdgeId, Vec<(usize, usize)>)> {
    let mut flows: Vec<(EdgeId, Vec<(usize, usize)>)> = Vec::new();
    let mut keys: Vec<(NodeId, &[String])> = Vec::new();

    for (edge, cells) in chains(columns) {
        // Tagged only. Two edges carrying the same tags into one box are one
        // flow and one line; two untagged ones are two lines that happen to
        // share a door, and drawing them as one would say something about them
        // that nothing in the graph supports.
        let Some(held) = g.edge(edge).filter(|e| !e.tags().is_empty()) else {
            flows.push((edge, cells));
            continue;
        };
        let key = (held.to(), held.tags());
        if let Some(at) = keys.iter().position(|k| *k == key)
            && let Some((id, held)) = flows.get_mut(at)
        {
            *id = (*id).min(edge);
            held.extend(cells);
            continue;
        }
        keys.push(key);
        flows.push((edge, cells));
    }
    flows
}

/// The placeholders of each long edge, by column and position.
fn chains(columns: &Columns) -> Vec<(EdgeId, Vec<(usize, usize)>)> {
    let mut chains: Vec<(EdgeId, Vec<(usize, usize)>)> = Vec::new();
    for (column, slots) in columns.iter().enumerate() {
        for (at, slot) in slots.iter().enumerate() {
            let Slot::Pass(edge) = slot else { continue };
            match chains.iter_mut().find(|(id, _)| id == edge) {
                Some((_, cells)) => cells.push((column, at)),
                None => chains.push((*edge, vec![(column, at)])),
            }
        }
    }
    chains
}

/// Which rows of each column a box already sits on.
fn occupied(columns: &Columns, placed: &Placed, rows: i32) -> Vec<Vec<bool>> {
    let mut taken = vec![vec![false; index(rows)]; columns.len()];
    for (column, slots) in columns.iter().enumerate() {
        for (at, slot) in slots.iter().enumerate() {
            if !matches!(slot, Slot::Node(_)) {
                continue;
            }
            for row in placed.top(column, at)..placed.bottom(column, at) {
                if let Some(cell) = taken.get_mut(column).and_then(|c| c.get_mut(index(row))) {
                    *cell = true;
                }
            }
        }
    }
    taken
}

/// The row nearest `target` that is free in every column the chain passes.
fn free_row(taken: &[Vec<bool>], cells: &[(usize, usize)], target: i32, rows: i32) -> i32 {
    let clear = |row: i32| {
        row >= 0
            && row < rows
            && cells.iter().all(|(column, _)| {
                taken.get(*column).and_then(|c| c.get(index(row))) != Some(&true)
            })
    };
    if clear(target) {
        return target;
    }
    for step in 1..=rows {
        for row in [target - step, target + step] {
            if clear(row) {
                return row;
            }
        }
    }
    target
}

/// A row as an index, with anything above the drawing folded onto its top.
fn index(row: i32) -> usize {
    usize::try_from(row.max(0)).unwrap_or(0)
}

/// How tall a column is with its slots packed as tightly as the gap allows.
fn natural(heights: &[i32]) -> i32 {
    let stacked: i32 = heights.iter().sum();
    stacked + GAP * i32::try_from(heights.len().saturating_sub(1)).unwrap_or(0)
}

/// Pulls a column back inside `height`, moving each slot as little as it can.
///
/// A sweep only ever pushes slots down — a slot that cannot have the row it
/// wants takes the next one free below — so a column drifts taller than it needs
/// to be. Pressing walks it from the bottom up and moves a slot only when it
/// does not fit, which is the smallest correction that makes the column fit.
fn press(placed: &mut Placed, height: i32) {
    for column in 0..placed.tops.len() {
        let Some(heights) = placed.heights.get(column) else {
            continue;
        };
        let mut ceiling = height;
        let mut tops = placed.tops.get(column).cloned().unwrap_or_default();
        for at in (0..tops.len()).rev() {
            let (Some(top), Some(h)) = (tops.get_mut(at), heights.get(at)) else {
                continue;
            };
            *top = (*top).min(ceiling - h);
            ceiling = *top - GAP;
        }
        if let Some(slot) = placed.tops.get_mut(column) {
            *slot = tops;
        }
    }
}

/// Every column stacked from the top, which is where a sweep starts from.
fn stacked(heights: &[Vec<i32>]) -> Vec<Vec<i32>> {
    heights
        .iter()
        .map(|column| {
            let mut top = 0;
            column
                .iter()
                .map(|h| {
                    let at = top;
                    top += h + GAP;
                    at
                })
                .collect()
        })
        .collect()
}

/// The row each slot of a column would like to sit on.
///
/// `None` for a slot with no neighbours on the side being read from: it has no
/// opinion, and forcing one on it would drag it away from where it already is.
fn desired(
    columns: &Columns,
    hops: &Hops,
    placed: &Placed,
    column: usize,
    forward: bool,
) -> Vec<Option<i32>> {
    let Some(slots) = columns.get(column) else {
        return Vec::new();
    };
    let neighbour = if forward {
        column.checked_sub(1)
    } else {
        column.checked_add(1)
    };
    let Some(neighbour) = neighbour.filter(|c| *c < columns.len()) else {
        return vec![None; slots.len()];
    };
    let gap = if forward { neighbour } else { column };

    slots
        .iter()
        .map(|slot| {
            let mut rows: Vec<i32> = hops
                .gap(gap)
                .iter()
                .filter_map(|hop| {
                    let other = if forward {
                        (hop.to == *slot).then_some(hop.from)
                    } else {
                        (hop.from == *slot).then_some(hop.to)
                    }?;
                    let at = columns.get(neighbour)?.iter().position(|s| *s == other)?;
                    Some(placed.middle(neighbour, at))
                })
                .collect();
            median(&mut rows)
        })
        .collect()
}

/// The middle of a sorted run, or the mean of the middle two.
///
/// Averaging the two middles keeps a fan symmetric: four branches put their box
/// between the second and third, where taking either one alone would shift the
/// whole fan a row off centre.
fn median(rows: &mut [i32]) -> Option<i32> {
    if rows.is_empty() {
        return None;
    }
    rows.sort_unstable();
    let middle = rows.len() / 2;
    if rows.len() % 2 == 1 {
        rows.get(middle).copied()
    } else {
        let (low, high) = (rows.get(middle - 1)?, rows.get(middle)?);
        Some(low + (high - low) / 2)
    }
}

/// Moves each slot of a column as close to where it wants to be as its
/// neighbours above it allow, then centres the column on what it asked for.
///
/// The second half is not decoration. Stacking downward from each slot's wish
/// puts the first slot where it wanted to be and everything after it below,
/// so a fan-in whose three sources all want the same row comes out with the
/// group a row-and-a-half low. Shifting the whole column by the median
/// disagreement fixes that, and a uniform shift cannot introduce an overlap.
fn settle(placed: &mut Placed, column: usize, wanted: &[Option<i32>]) {
    let Some(heights) = placed.heights.get(column) else {
        return;
    };
    let mut tops = Vec::with_capacity(heights.len());
    let mut floor = i32::MIN;

    for (at, height) in heights.iter().enumerate() {
        let aimed = wanted
            .get(at)
            .copied()
            .flatten()
            .map_or_else(|| placed.top(column, at), |row| row - (height - 1) / 2);
        let top = aimed.max(floor);
        tops.push(top);
        floor = top + height + GAP;
    }

    let mut missed: Vec<i32> = wanted
        .iter()
        .enumerate()
        .filter_map(|(at, row)| {
            let (top, height) = (*tops.get(at)?, *heights.get(at)?);
            Some((*row)? - (top + (height - 1) / 2))
        })
        .collect();
    if let Some(shift) = median(&mut missed) {
        for top in &mut tops {
            *top += shift;
        }
    }

    if let Some(slot) = placed.tops.get_mut(column) {
        *slot = tops;
    }
}

/// Slides everything down so the highest row is zero.
///
/// Sweeps work in relative rows and a column is free to drift above the top
/// while they run; clamping each column as it moves would fight the centring.
fn normalise(placed: &mut Placed) {
    let Some(highest) = placed.tops.iter().flatten().copied().min() else {
        return;
    };
    for column in &mut placed.tops {
        for top in column {
            *top -= highest;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Adjacency, acyclic::back_edges, layer::layer, order, rank::rank};
    use super::*;
    use crate::graph::{Node, NodeId};

    struct Case {
        g: Graph,
        ids: Vec<NodeId>,
        columns: Columns,
        placed: Placed,
    }

    impl Case {
        fn row_of(&self, node: usize) -> i32 {
            let want = Slot::Node(self.ids[node]);
            for (column, slots) in self.columns.iter().enumerate() {
                if let Some(at) = slots.iter().position(|s| *s == want) {
                    return self.placed.middle(column, at);
                }
            }
            panic!("{node} is not in any column")
        }
    }

    fn case(nodes: &[&str], edges: &[(usize, usize)]) -> Case {
        let mut g = Graph::new();
        let ids: Vec<_> = nodes.iter().map(|n| g.add_node(Node::new(*n))).collect();
        for &(a, b) in edges {
            g.add_edge(ids[a], ids[b]);
        }
        let adj = Adjacency::of(&g);
        let acyclic = back_edges(&g, &adj);
        let ranked = rank(&g, &adj, &acyclic);
        let layered = layer(&g, &adj, &acyclic, &ranked);
        let hops = Hops::of(&layered);
        let columns = layered.all().to_vec();
        let placed = place(&g, &columns, &hops, &[], 0, Merge::Flows);
        Case {
            g,
            ids,
            columns,
            placed,
        }
    }

    #[test]
    fn a_chain_sits_on_one_row() {
        let c = case(&["a", "b", "c", "d"], &[(0, 1), (1, 2), (2, 3)]);
        assert_eq!(c.row_of(0), c.row_of(1));
        assert_eq!(c.row_of(1), c.row_of(2));
        assert_eq!(c.row_of(2), c.row_of(3));
    }

    #[test]
    fn a_fan_out_centres_its_source() {
        let c = case(&["a", "x", "y", "z"], &[(0, 1), (0, 2), (0, 3)]);
        assert_eq!(
            c.row_of(0),
            c.row_of(2),
            "the source sits opposite the middle branch"
        );
    }

    #[test]
    fn a_fan_out_of_four_centres_between_the_middle_two() {
        let c = case(
            &["a", "w", "x", "y", "z"],
            &[(0, 1), (0, 2), (0, 3), (0, 4)],
        );
        let middle = c.row_of(2) + (c.row_of(3) - c.row_of(2)) / 2;
        assert_eq!(c.row_of(0), middle);
    }

    #[test]
    fn a_fan_in_centres_its_target() {
        let c = case(&["x", "y", "z", "a"], &[(0, 3), (1, 3), (2, 3)]);
        assert_eq!(c.row_of(3), c.row_of(1));
    }

    #[test]
    fn stacked_slots_never_overlap() {
        let c = case(
            &["a", "w", "x", "y", "z"],
            &[(0, 1), (0, 2), (0, 3), (0, 4)],
        );
        for (column, slots) in c.columns.iter().enumerate() {
            for at in 1..slots.len() {
                assert!(
                    c.placed.top(column, at) >= c.placed.bottom(column, at - 1),
                    "column {column} slot {at} overlaps the one above"
                );
            }
        }
    }

    #[test]
    fn nothing_is_placed_above_the_top_of_the_drawing() {
        let c = case(
            &["a", "b", "x", "y", "z"],
            &[(0, 2), (1, 2), (1, 3), (1, 4)],
        );
        for (column, slots) in c.columns.iter().enumerate() {
            for at in 0..slots.len() {
                assert!(
                    c.placed.top(column, at) >= 0,
                    "column {column} slot {at} is off the top"
                );
            }
        }
        assert!(c.placed.height() > 0);
    }

    #[test]
    fn no_column_is_taller_than_the_tallest_needs_to_be() {
        // Five in one column against one in the next: the tall column sets the
        // height and nothing else may exceed it.
        let c = case(
            &["a", "v", "w", "x", "y", "z", "end"],
            &[(0, 1), (0, 2), (0, 3), (0, 4), (0, 5), (1, 6), (5, 6)],
        );
        let height = c.placed.height();
        for (column, slots) in c.columns.iter().enumerate() {
            for at in 0..slots.len() {
                assert!(
                    c.placed.top(column, at) >= 0 && c.placed.bottom(column, at) <= height,
                    "column {column} slot {at} falls outside 0..{height}"
                );
            }
        }
    }

    #[test]
    fn the_drawing_is_no_taller_than_its_fullest_column() {
        let c = case(
            &["a", "w", "x", "y", "z"],
            &[(0, 1), (0, 2), (0, 3), (0, 4)],
        );
        // Four boxes of three rows with a blank row between them.
        assert_eq!(c.placed.height(), 4 * 3 + 3);
    }

    #[test]
    fn a_taller_box_takes_more_rows() {
        let mut g = Graph::new();
        let short = g.add_node(Node::new("short"));
        let tall = g.add_node(Node::new("tall").line("one").line("two"));
        g.add_edge(short, tall);
        let adj = Adjacency::of(&g);
        let acyclic = back_edges(&g, &adj);
        let ranked = rank(&g, &adj, &acyclic);
        let layered = layer(&g, &adj, &acyclic, &ranked);
        let columns = layered.all().to_vec();
        let placed = place(&g, &columns, &Hops::of(&layered), &[], 0, Merge::Flows);

        assert_eq!(placed.height_of(0, 0), 3);
        assert_eq!(placed.height_of(1, 0), 5);
    }

    #[test]
    fn a_long_edge_runs_on_one_row_the_whole_way() {
        // 0 -> 1 -> 2 -> 3 with 0 -> 3 passing two columns.
        let c = case(&["a", "b", "c", "d"], &[(0, 1), (1, 2), (2, 3), (0, 3)]);
        let long = c.g.edge_ids().last().unwrap();

        let rows: Vec<i32> = c
            .columns
            .iter()
            .enumerate()
            .flat_map(|(column, slots)| {
                slots.iter().enumerate().filter_map(move |(at, slot)| {
                    (*slot == Slot::Pass(long)).then_some((column, at))
                })
            })
            .map(|(column, at)| c.placed.middle(column, at))
            .collect();

        assert_eq!(rows.len(), 2, "the long edge should pass two columns");
        assert!(
            rows.windows(2).all(|w| w[0] == w[1]),
            "the chain bends: {rows:?}"
        );
    }

    #[test]
    fn a_long_edge_never_runs_through_a_box() {
        let c = case(
            &["a", "b", "c", "d", "e"],
            &[(0, 1), (1, 2), (2, 3), (3, 4), (0, 4), (0, 3)],
        );
        for (column, slots) in c.columns.iter().enumerate() {
            let boxes: Vec<(i32, i32)> = slots
                .iter()
                .enumerate()
                .filter(|(_, s)| matches!(s, Slot::Node(_)))
                .map(|(at, _)| (c.placed.top(column, at), c.placed.bottom(column, at)))
                .collect();
            for (at, slot) in slots.iter().enumerate() {
                if !matches!(slot, Slot::Pass(_)) {
                    continue;
                }
                let row = c.placed.middle(column, at);
                assert!(
                    boxes
                        .iter()
                        .all(|(top, bottom)| row < *top || row >= *bottom),
                    "column {column}: a pass row {row} lands inside a box"
                );
            }
        }
    }

    /// Two long edges of one flow into one box run on one row.
    ///
    /// Same tags, same target: one line, by `design.md` §3. Laid separately
    /// each takes a row and marks it taken, so the second is pushed off the
    /// first and the two have to cross to reach the same door.
    #[test]
    fn one_flow_into_one_box_takes_one_row() {
        let mut g = Graph::new();
        let ids: Vec<_> = ["a", "b", "c", "d", "z"]
            .iter()
            .map(|n| g.add_node(Node::new(*n)))
            .collect();
        g.add_edge(ids[0], ids[1]);
        g.add_edge(ids[1], ids[2]);
        g.add_edge(ids[2], ids[3]);
        g.add_edge(ids[3], ids[4]);
        // Both skip, both carry `t`, both end at z.
        g.add_tagged_edge(ids[1], ids[4], ["t"]);
        g.add_tagged_edge(ids[2], ids[4], ["t"]);

        let adj = Adjacency::of(&g);
        let acyclic = back_edges(&g, &adj);
        let ranked = rank(&g, &adj, &acyclic);
        let layered = layer(&g, &adj, &acyclic, &ranked);
        let columns = layered.all().to_vec();
        let placed = place(
            &g,
            &columns,
            &order::Hops::of(&layered),
            &super::super::port::interiors(&g, &acyclic),
            0,
            Merge::Flows,
        );

        let mut rows: Vec<(usize, i32)> = Vec::new();
        for (column, slots) in columns.iter().enumerate() {
            for (at, slot) in slots.iter().enumerate() {
                if matches!(slot, Slot::Pass(_)) {
                    rows.push((column, placed.middle(column, at)));
                }
            }
        }
        let shared = rows.len() - {
            let mut distinct = rows.clone();
            distinct.sort_unstable();
            distinct.dedup();
            distinct.len()
        };
        assert!(shared > 0, "one flow was drawn as two lines");
    }

    #[test]
    fn two_long_edges_across_the_same_columns_take_different_rows() {
        let c = case(
            &["a", "b", "c", "d", "e"],
            &[(0, 1), (1, 2), (2, 3), (0, 3), (0, 4), (4, 3)],
        );
        let mut seen: Vec<(usize, i32)> = Vec::new();
        for (column, slots) in c.columns.iter().enumerate() {
            for (at, slot) in slots.iter().enumerate() {
                if matches!(slot, Slot::Pass(_)) {
                    let row = c.placed.middle(column, at);
                    assert!(
                        !seen.contains(&(column, row)),
                        "two passes share {column}:{row}"
                    );
                    seen.push((column, row));
                }
            }
        }
    }

    #[test]
    fn placement_is_the_same_every_run() {
        let edges = [(0, 1), (0, 2), (1, 3), (2, 3), (0, 4), (4, 3)];
        let once = case(&["a", "b", "c", "d", "e"], &edges).placed;
        for _ in 0..8 {
            assert_eq!(case(&["a", "b", "c", "d", "e"], &edges).placed, once);
        }
    }

    #[test]
    fn every_candidate_ordering_places_without_complaint() {
        let mut g = Graph::new();
        let ids: Vec<_> = (0..7)
            .map(|i| g.add_node(Node::new(format!("n{i}"))))
            .collect();
        for &(a, b) in &[(0, 3), (1, 4), (2, 3), (0, 5), (3, 6), (4, 6), (5, 6)] {
            g.add_edge(ids[a], ids[b]);
        }
        let adj = Adjacency::of(&g);
        let acyclic = back_edges(&g, &adj);
        let ranked = rank(&g, &adj, &acyclic);
        let layered = layer(&g, &adj, &acyclic, &ranked);
        let hops = Hops::of(&layered);

        for columns in order::orderings(&layered) {
            assert!(place(&g, &columns, &hops, &[], 0, Merge::Flows).height() > 0);
        }
    }
}
