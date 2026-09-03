//! Turning rows and columns into orthogonal polylines.
//!
//! Placement said which row everything sits on. Routing says where the lines
//! go: out of a box's right edge, across the gap, into the next box's left
//! edge, turning only at right angles.
//!
//! The vocabulary is deliberately small. Between neighbouring columns an edge
//! is straight, an **L** with one bend, or a **Z** with two — nothing else. An
//! edge that skips columns turns at each end and runs straight across the
//! middle, because the placeholder rows it runs on are all the same row. Four
//! bends at most, whatever the skip.
//!
//! Where in the gap an edge turns is not a free choice. Two edges turning on the
//! same column of cells are drawn as one line, so every vertical run takes a
//! track of its own — see [`super::track`] — and the gap is made wide enough to
//! hold them.

use super::acyclic::Acyclic;
use super::layer::Slot;
use super::order::Columns;
use super::place::Placed;
use super::port::Ports;
use super::track::{Run, Tracks, pack};
use crate::graph::{EdgeId, Graph, NodeId};

/// The fewest blank columns between one column of boxes and the next.
const MIN_GAP: i32 = 5;

/// A cell of clearance either side of a gap's tracks: the stub out of the box
/// on one side, and room for the arrowhead on the other.
const CLEARANCE: usize = 2;

/// Padding inside a box: two borders and a space either side of the text.
const PADDING: i32 = 4;

/// The narrowest a box may be drawn.
const MIN_WIDTH: i32 = PADDING + 1;

/// Where one box sits, in cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Boxed {
    pub(crate) node: NodeId,
    /// Which column it is in, which is how far an edge into it has come.
    pub(crate) column: usize,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
}

/// One edge as an orthogonal polyline, corner to corner.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Route {
    pub(crate) edge: EdgeId,
    pub(crate) points: Vec<(i32, i32)>,
}

impl Route {
    /// How many right angles the line turns through.
    pub(crate) fn bends(&self) -> usize {
        self.points.len().saturating_sub(2)
    }
}

/// A drawing in cell coordinates, with no character in it yet.
#[derive(Clone, Debug, Default)]
pub(crate) struct Layout {
    pub(crate) boxes: Vec<Boxed>,
    pub(crate) routes: Vec<Route>,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

impl Layout {
    /// One box, by the node it draws.
    pub(crate) fn boxed(&self, node: NodeId) -> Option<&Boxed> {
        self.boxes.iter().find(|b| b.node == node)
    }
}

/// The row an edge is on at each column it touches, source to target.
///
/// Rows are settled before any x is, because they have to be: how wide a gap
/// needs to be depends on how many runs cross it, and which runs cross it
/// depends on which edges change row there.
struct Path {
    edge: EdgeId,
    first: usize,
    rows: Vec<i32>,
    from: NodeId,
    to: NodeId,
}

impl Path {
    /// The slot this edge occupies in one of its columns.
    fn slot(&self, column: usize) -> Slot {
        if column == self.first {
            Slot::Node(self.from)
        } else if column + 1 == self.first + self.rows.len() {
            Slot::Node(self.to)
        } else {
            Slot::Pass(self.edge)
        }
    }
}

/// Lays out the boxes and routes every edge that has a path across the columns.
pub(crate) fn route(
    g: &Graph,
    acyclic: &Acyclic,
    columns: &Columns,
    placed: &Placed,
    ports: &Ports,
) -> Layout {
    let widths = widths(g, columns);
    let paths = paths(g, acyclic, columns, placed, ports);
    let runs = runs(&paths);
    let tracks = pack(&runs, columns.len().saturating_sub(1));

    let gaps = gaps(&tracks, columns.len());
    let lefts = lefts(&widths, &gaps);
    let boxes = boxes(columns, placed, &widths, &lefts);

    let routes = paths
        .iter()
        .filter_map(|path| {
            let points = polyline(path, &runs, &tracks, &lefts, &widths)?;
            Some(Route {
                edge: path.edge,
                points,
            })
        })
        .collect();

    let width = lefts
        .iter()
        .zip(&widths)
        .map(|(x, w)| x + w)
        .max()
        .unwrap_or(0);
    Layout {
        boxes,
        routes,
        width,
        height: placed.height(),
    }
}

/// How wide each column's boxes are: the widest text in the column, padded.
///
/// One width per column rather than per box, because boxes whose left edges do
/// not line up read as a ragged margin rather than as a column.
fn widths(g: &Graph, columns: &Columns) -> Vec<i32> {
    columns
        .iter()
        .map(|slots| {
            slots
                .iter()
                .filter_map(|slot| match slot {
                    Slot::Node(id) => g.node(*id),
                    Slot::Pass(_) => None,
                })
                .map(|node| {
                    let longest = std::iter::once(node.label())
                        .chain(node.lines().iter().map(String::as_str))
                        .map(|line| i32::try_from(line.chars().count()).unwrap_or(i32::MAX))
                        .max()
                        .unwrap_or(0);
                    longest.saturating_add(PADDING)
                })
                .max()
                .unwrap_or(MIN_WIDTH)
                .max(MIN_WIDTH)
        })
        .collect()
}

/// The row every forward edge is on at each of its columns.
fn paths(
    g: &Graph,
    acyclic: &Acyclic,
    columns: &Columns,
    placed: &Placed,
    ports: &Ports,
) -> Vec<Path> {
    g.edge_ids()
        .filter(|id| !acyclic.is_back(*id))
        .filter_map(|id| {
            let edge = g.edge(id)?;
            let (from, to) = (edge.from(), edge.to());
            let first = column_of(columns, Slot::Node(from))?;
            let last = column_of(columns, Slot::Node(to))?;
            if last <= first {
                return None;
            }
            let rows = (first..=last)
                .map(|column| {
                    let slot = if column == first {
                        Slot::Node(from)
                    } else if column == last {
                        Slot::Node(to)
                    } else {
                        Slot::Pass(id)
                    };
                    let at = columns.get(column)?.iter().position(|s| *s == slot)?;
                    // A box's ends are its attach rows, one per tag set; a
                    // placeholder has only the row it was given.
                    Some(match (column == first, column == last) {
                        (true, _) => ports.exit(id).unwrap_or(placed.middle(column, at)),
                        (_, true) => ports.entry(id).unwrap_or(placed.middle(column, at)),
                        _ => placed.middle(column, at),
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Path {
                edge: id,
                first,
                rows,
                from,
                to,
            })
        })
        .collect()
}

/// Which column a slot is in.
fn column_of(columns: &Columns, slot: Slot) -> Option<usize> {
    columns.iter().position(|slots| slots.contains(&slot))
}

/// Every vertical run every path needs, in path order then gap order.
///
/// A hop that stays on its row needs none: it is drawn as one straight line and
/// nothing has to make room for it.
fn runs(paths: &[Path]) -> Vec<Run> {
    let mut runs = Vec::new();
    for path in paths {
        for (step, pair) in path.rows.windows(2).enumerate() {
            let [from_row, to_row] = *pair else { continue };
            if from_row == to_row {
                continue;
            }
            let gap = path.first + step;
            runs.push(Run {
                edge: path.edge,
                gap,
                enter: from_row,
                leave: to_row,
                from: path.slot(gap),
                to: path.slot(gap + 1),
            });
        }
    }
    runs
}

/// How wide each gap has to be to hold its tracks.
fn gaps(tracks: &Tracks, columns: usize) -> Vec<i32> {
    (0..columns.saturating_sub(1))
        .map(|gap| {
            let needed = i32::try_from(tracks.count(gap) + CLEARANCE).unwrap_or(MIN_GAP);
            needed.max(MIN_GAP)
        })
        .collect()
}

/// The left edge of each column.
fn lefts(widths: &[i32], gaps: &[i32]) -> Vec<i32> {
    let mut x = 0;
    widths
        .iter()
        .enumerate()
        .map(|(column, w)| {
            let at = x;
            x += w + gaps.get(column).copied().unwrap_or(MIN_GAP);
            at
        })
        .collect()
}

fn boxes(columns: &Columns, placed: &Placed, widths: &[i32], lefts: &[i32]) -> Vec<Boxed> {
    let mut boxes = Vec::new();
    for (column, slots) in columns.iter().enumerate() {
        for (at, slot) in slots.iter().enumerate() {
            let Slot::Node(node) = slot else { continue };
            boxes.push(Boxed {
                node: *node,
                column,
                x: lefts.get(column).copied().unwrap_or(0),
                y: placed.top(column, at),
                w: widths.get(column).copied().unwrap_or(MIN_WIDTH),
                h: placed.height_of(column, at),
            });
        }
    }
    boxes
}

/// Joins the rows of a path, turning on the track each of its runs was given.
fn polyline(
    path: &Path,
    runs: &[Run],
    tracks: &Tracks,
    lefts: &[i32],
    widths: &[i32],
) -> Option<Vec<(i32, i32)>> {
    let last = path.first + path.rows.len() - 1;
    let exit = lefts
        .get(path.first)?
        .checked_add(*widths.get(path.first)?)?;
    let entry = lefts.get(last)?.checked_sub(1)?;

    let mut points = vec![(exit, *path.rows.first()?)];
    for (step, pair) in path.rows.windows(2).enumerate() {
        let [from_row, to_row] = *pair else { continue };
        if from_row == to_row {
            continue;
        }
        let gap = path.first + step;
        let track = runs
            .iter()
            .position(|run| run.edge == path.edge && run.gap == gap)
            .map_or(0, |run| tracks.of(run));
        let x = track_x(lefts, widths, gap, track);
        points.push((x, from_row));
        points.push((x, to_row));
    }
    points.push((entry, *path.rows.last()?));
    Some(collapse(points))
}

/// Where one track of one gap sits: a cell clear of the box, then one per track.
fn track_x(lefts: &[i32], widths: &[i32], gap: usize, track: usize) -> i32 {
    let start =
        lefts.get(gap).copied().unwrap_or(0) + widths.get(gap).copied().unwrap_or(MIN_WIDTH);
    start + 1 + i32::try_from(track).unwrap_or(0)
}

/// Drops the points that do not turn.
fn collapse(points: Vec<(i32, i32)>) -> Vec<(i32, i32)> {
    let mut kept: Vec<(i32, i32)> = Vec::with_capacity(points.len());
    for point in points {
        if kept.last() == Some(&point) {
            continue;
        }
        if let [.., before, middle] = kept.as_slice() {
            let straight = (before.0 == middle.0 && middle.0 == point.0)
                || (before.1 == middle.1 && middle.1 == point.1);
            if straight {
                kept.pop();
            }
        }
        kept.push(point);
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collinear_points_are_dropped() {
        assert_eq!(collapse(vec![(0, 0), (3, 0), (7, 0)]), [(0, 0), (7, 0)]);
        assert_eq!(collapse(vec![(0, 0), (0, 4), (0, 9)]), [(0, 0), (0, 9)]);
    }

    #[test]
    fn a_repeated_point_is_dropped() {
        assert_eq!(collapse(vec![(0, 0), (0, 0), (5, 0)]), [(0, 0), (5, 0)]);
    }

    #[test]
    fn a_gap_is_never_narrower_than_the_minimum() {
        assert_eq!(gaps(&Tracks::default(), 3), [MIN_GAP, MIN_GAP]);
    }

    #[test]
    fn a_track_sits_clear_of_the_box_it_leaves() {
        let (widths, gaps) = (vec![6, 6], vec![5]);
        let lefts = lefts(&widths, &gaps);
        assert_eq!(lefts, [0, 11]);
        // The box occupies 0..5, so 6 is the first free cell and the first
        // track is one clear of that.
        assert_eq!(track_x(&lefts, &widths, 0, 0), 7);
        assert_eq!(track_x(&lefts, &widths, 0, 1), 8);
    }
}
