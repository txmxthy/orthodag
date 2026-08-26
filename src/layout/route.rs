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
//! Where in the gap an edge turns is decided here by putting it in the middle.
//! That is the placeholder for track packing, which is what actually decides it.

use super::acyclic::Acyclic;
use super::layer::Slot;
use super::order::Columns;
use super::place::Placed;
use crate::graph::{EdgeId, Graph, NodeId};

/// Blank columns between one column of boxes and the next.
const GAP: i32 = 5;

/// Padding inside a box: two borders and a space either side of the text.
const PADDING: i32 = 4;

/// The narrowest a box may be drawn.
const MIN_WIDTH: i32 = PADDING + 1;

/// Where one box sits, in cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Boxed {
    pub(crate) node: NodeId,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
}

impl Boxed {
    /// The cell just past the right border, where an edge leaves.
    fn exit(self) -> i32 {
        self.x + self.w
    }

    /// The cell just before the left border, where an edge arrives.
    fn entry(self) -> i32 {
        self.x - 1
    }
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

/// The finished geometry, before any line is drawn through it.
///
/// Not a context object: nothing here is mutated, and it exists so the two
/// functions that need all five pieces can say so once instead of taking eight
/// arguments each.
struct Frame<'a> {
    columns: &'a Columns,
    placed: &'a Placed,
    boxes: Vec<Boxed>,
    lefts: Vec<i32>,
    widths: Vec<i32>,
}

impl Frame<'_> {
    fn left(&self, column: usize) -> i32 {
        self.lefts.get(column).copied().unwrap_or(0)
    }

    fn width(&self, column: usize) -> i32 {
        self.widths.get(column).copied().unwrap_or(MIN_WIDTH)
    }

    fn boxed(&self, node: NodeId) -> Option<&Boxed> {
        self.boxes.iter().find(|b| b.node == node)
    }
}

/// Lays out the boxes and routes every edge that has a path across the columns.
pub(crate) fn route(g: &Graph, acyclic: &Acyclic, columns: &Columns, placed: &Placed) -> Layout {
    let widths = widths(g, columns);
    let lefts = lefts(&widths);
    let boxes = boxes(columns, placed, &widths, &lefts);
    let frame = Frame {
        columns,
        placed,
        boxes,
        lefts,
        widths,
    };

    let width = frame
        .lefts
        .iter()
        .zip(&frame.widths)
        .map(|(x, w)| x + w)
        .max()
        .unwrap_or(0);
    let routes = routes(g, acyclic, &frame);
    Layout {
        boxes: frame.boxes,
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

/// The left edge of each column.
fn lefts(widths: &[i32]) -> Vec<i32> {
    let mut x = 0;
    widths
        .iter()
        .map(|w| {
            let at = x;
            x += w + GAP;
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
                x: lefts.get(column).copied().unwrap_or(0),
                y: placed.top(column, at),
                w: widths.get(column).copied().unwrap_or(MIN_WIDTH),
                h: placed.height_of(column, at),
            });
        }
    }
    boxes
}

/// Every cell an edge is anchored to, left to right: box, placeholders, box.
///
/// `x` is where the line is at that point and `y` is the row it is on. For a
/// box that is the cell outside its border; for a placeholder it is the middle
/// of the gap the edge is crossing.
fn anchors(frame: &Frame, edge: EdgeId, from: NodeId, to: NodeId) -> Option<Vec<(i32, i32)>> {
    let source = frame.boxed(from)?;
    let target = frame.boxed(to)?;
    let mut anchors = vec![(source.exit(), source.y + (source.h - 1) / 2)];

    for (column, slots) in frame.columns.iter().enumerate() {
        for (at, slot) in slots.iter().enumerate() {
            if *slot != Slot::Pass(edge) {
                continue;
            }
            let row = frame.placed.middle(column, at);
            anchors.push((frame.left(column), row));
            anchors.push((frame.left(column) + frame.width(column) - 1, row));
        }
    }

    anchors.push((target.entry(), target.y + (target.h - 1) / 2));
    Some(anchors)
}

/// Joins consecutive anchors with an L, a Z, or a straight run.
fn polyline(anchors: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let mut points: Vec<(i32, i32)> = Vec::new();
    for pair in anchors.windows(2) {
        let [(x0, y0), (x1, y1)] = *pair else {
            continue;
        };
        if points.is_empty() {
            points.push((x0, y0));
        }
        if y0 != y1 {
            // Turn halfway across, which is what a track will decide properly.
            let turn = x0 + (x1 - x0) / 2;
            points.push((turn, y0));
            points.push((turn, y1));
        }
        points.push((x1, y1));
    }
    collapse(points)
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

/// One route per forward edge whose two boxes are both in the drawing.
///
/// A back edge gets none: it is drawn later through a lane under the boxes, and
/// giving it a left-to-right route here would point it the wrong way.
fn routes(g: &Graph, acyclic: &Acyclic, frame: &Frame) -> Vec<Route> {
    g.edge_ids()
        .filter(|id| !acyclic.is_back(*id))
        .filter_map(|id| {
            let edge = g.edge(id)?;
            let anchors = anchors(frame, id, edge.from(), edge.to())?;
            Some(Route {
                edge: id,
                points: polyline(&anchors),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_that_does_not_turn_keeps_two_points() {
        assert_eq!(polyline(&[(0, 3), (9, 3)]), [(0, 3), (9, 3)]);
    }

    #[test]
    fn a_line_that_changes_row_turns_twice() {
        assert_eq!(
            polyline(&[(0, 1), (8, 5)]),
            [(0, 1), (4, 1), (4, 5), (8, 5)]
        );
    }

    #[test]
    fn collinear_points_are_dropped() {
        assert_eq!(collapse(vec![(0, 0), (3, 0), (7, 0)]), [(0, 0), (7, 0)]);
        assert_eq!(collapse(vec![(0, 0), (0, 4), (0, 9)]), [(0, 0), (0, 9)]);
    }

    #[test]
    fn a_repeated_point_is_dropped() {
        assert_eq!(collapse(vec![(0, 0), (0, 0), (5, 0)]), [(0, 0), (5, 0)]);
    }
}
