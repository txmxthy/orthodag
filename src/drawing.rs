//! A drawing built by the caller, in terms this library can score.
//!
//! [`score`](crate::score) answers "what is the drawing *this* library would
//! make of that graph worth". The objective is also offered on its own, apart
//! from the layout that usually feeds it, so a caller can ask the same question
//! of a drawing it built itself. Anyone with boxes and orthogonal polylines in
//! cell coordinates builds a [`Drawing`] and gets a [`Score`](crate::Score)
//! from the same code that scores this library's own output.
//!
//! # Example
//!
//! ```
//! use orthodag::{Drawing, Graph, Node, Rect};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut g = Graph::new();
//! let a = g.add_node(Node::new("a"));
//! let b = g.add_node(Node::new("b"));
//! let edge = g.add_edge(a, b)?;
//!
//! let mut drawing = Drawing::new(20, 3)?;
//! drawing.boxed(a, 0, Rect::new(0, 0, 5, 3))?;
//! drawing.boxed(b, 1, Rect::new(15, 0, 5, 3))?;
//! drawing.route(edge, [(5, 1), (14, 1)])?;
//!
//! // A straight edge between two boxes: nothing to charge for.
//! assert_eq!(orthodag::score_drawing(&g, &drawing)?.total, 0);
//! # Ok(())
//! # }
//! ```
//!
//! Points are in the same cell coordinates the drawing is measured in: `x`
//! rightward, `y` downward, origin top left, both inclusive of the cells the
//! line actually occupies. Construction rejects geometry that is outside that
//! frame or would make painting and scoring unbounded.

use std::fmt;

use crate::graph::{EdgeId, NodeId};
use crate::layout::route::{Boxed as Placed, Layout, Route};

/// The largest frame a caller-provided drawing may allocate.
pub const MAX_DRAWING_CELLS: u64 = 4_000_000;

/// The most corners accepted in one caller-provided route.
pub const MAX_ROUTE_POINTS: usize = 4_096;

const MAX_DRAWING_ITEMS: usize = 65_536;
const MAX_DRAWING_ROUTE_POINTS: usize = 262_144;
const MAX_DRAWING_ROUTE_CELLS: u64 = 8_000_000;

/// Why caller-provided drawing geometry was rejected.
#[derive(Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum DrawingError {
    /// The frame is empty or has a negative dimension.
    InvalidSize {
        /// Requested width.
        width: i32,
        /// Requested height.
        height: i32,
    },
    /// The frame would allocate more cells than the public drawing budget.
    FrameTooLarge {
        /// Requested cell count.
        cells: u64,
        /// Maximum accepted cell count.
        limit: u64,
    },
    /// A box has an empty or negative size.
    InvalidRect {
        /// Node being placed.
        node: NodeId,
        /// Rejected rectangle.
        rect: Rect,
    },
    /// A box is not wholly inside the frame.
    BoxOutOfBounds {
        /// Node being placed.
        node: NodeId,
        /// Rejected rectangle.
        rect: Rect,
    },
    /// The same node was assigned more than one box.
    DuplicateNode(NodeId),
    /// A box names no node in the graph being drawn.
    UnknownNode(NodeId),
    /// The same edge was assigned more than one route.
    DuplicateEdge(EdgeId),
    /// A route names no edge in the graph being drawn.
    UnknownEdge(EdgeId),
    /// An orthogonal polyline needs at least two points.
    RouteTooShort(EdgeId),
    /// One route contains more corners than the per-route budget.
    RouteTooLong {
        /// Edge being routed.
        edge: EdgeId,
        /// Requested point count.
        points: usize,
        /// Maximum accepted point count.
        limit: usize,
    },
    /// A route point is outside the frame.
    RouteOutOfBounds {
        /// Edge being routed.
        edge: EdgeId,
        /// First rejected point.
        point: (i32, i32),
    },
    /// Consecutive route points do not form a horizontal or vertical segment.
    DiagonalSegment {
        /// Edge being routed.
        edge: EdgeId,
        /// Segment start.
        from: (i32, i32),
        /// Segment end.
        to: (i32, i32),
    },
    /// The drawing contains too many boxes or routes.
    TooManyItems {
        /// Maximum accepted count for boxes or routes.
        limit: usize,
    },
    /// All routes together exceed the point or raster-work budget.
    RouteBudgetExceeded,
}

impl fmt::Display for DrawingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize { width, height } => {
                write!(f, "drawing size must be positive, got {width}x{height}")
            }
            Self::FrameTooLarge { cells, limit } => {
                write!(f, "drawing has {cells} cells; the limit is {limit}")
            }
            Self::InvalidRect { node, rect } => write!(
                f,
                "box for {node} must have positive dimensions, got {}x{}",
                rect.w, rect.h
            ),
            Self::BoxOutOfBounds { node, .. } => {
                write!(f, "box for {node} is outside the drawing frame")
            }
            Self::DuplicateNode(node) => write!(f, "{node} has more than one box"),
            Self::UnknownNode(node) => write!(f, "{node} is not in the graph"),
            Self::DuplicateEdge(edge) => write!(f, "{edge} has more than one route"),
            Self::UnknownEdge(edge) => write!(f, "{edge} is not in the graph"),
            Self::RouteTooShort(edge) => write!(f, "route for {edge} has fewer than two points"),
            Self::RouteTooLong {
                edge,
                points,
                limit,
            } => write!(
                f,
                "route for {edge} has {points} points; the limit is {limit}"
            ),
            Self::RouteOutOfBounds { edge, point } => {
                write!(f, "route for {edge} leaves the frame at {point:?}")
            }
            Self::DiagonalSegment { edge, from, to } => {
                write!(f, "route for {edge} is diagonal from {from:?} to {to:?}")
            }
            Self::TooManyItems { limit } => {
                write!(f, "drawing has more than {limit} boxes or routes")
            }
            Self::RouteBudgetExceeded => write!(f, "drawing routes exceed the work budget"),
        }
    }
}

impl std::error::Error for DrawingError {}

/// Where a box sits, in cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rect {
    /// Leftmost cell.
    pub x: i32,
    /// Topmost cell.
    pub y: i32,
    /// Width in cells, borders included.
    pub w: i32,
    /// Height in cells, borders included.
    pub h: i32,
}

impl Rect {
    /// A rectangle by its corner and size.
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
}

/// Which way an edge is pointing where it arrives.
///
/// Forward edges all arrive from the left, so this was a constant until back
/// edges came up through a lane and needed to point the other way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Heading {
    /// Arrives from the left: `▶`.
    Right,
    /// Comes up out of a lane below: `▲`.
    Up,
}

/// One box of a drawing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Boxed {
    /// The node it draws.
    pub node: NodeId,
    /// Which column it is in, counting from zero at the left.
    pub column: usize,
    /// Where it sits.
    pub rect: Rect,
}

/// One edge's line, corner to corner, and which way its arrowhead points.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Routed<'a> {
    /// The edge it draws.
    pub edge: EdgeId,
    /// Every corner, source end first; the last point is the arrowhead's cell.
    pub points: &'a [(i32, i32)],
    /// Which way the arrowhead faces.
    pub heading: Heading,
}

/// A finished drawing: boxes, orthogonal polylines, and how big the frame is.
///
/// Built up with [`boxed`](Self::boxed) and [`route`](Self::route), then handed
/// to [`score_drawing`](crate::score_drawing); or handed back by
/// [`layout`](crate::layout) and read through [`boxes`](Self::boxes) and
/// [`routes`](Self::routes) by a caller that paints its own boxes.
#[derive(Clone, Debug)]
pub struct Drawing {
    inner: Layout,
}

impl Drawing {
    /// An empty drawing of a given size, in cells.
    ///
    /// # Errors
    ///
    /// Returns [`DrawingError::InvalidSize`] for an empty frame and
    /// [`DrawingError::FrameTooLarge`] when its allocation would exceed the
    /// public cell budget.
    pub fn new(width: i32, height: i32) -> Result<Self, DrawingError> {
        if width <= 0 || height <= 0 {
            return Err(DrawingError::InvalidSize { width, height });
        }
        let cells = u64::try_from(width)
            .ok()
            .and_then(|width| u64::try_from(height).ok().map(|height| width * height))
            .unwrap_or(u64::MAX);
        if cells > MAX_DRAWING_CELLS {
            return Err(DrawingError::FrameTooLarge {
                cells,
                limit: MAX_DRAWING_CELLS,
            });
        }
        Ok(Self {
            inner: Layout {
                width,
                height,
                ..Layout::default()
            },
        })
    }

    /// Places the box that draws one node.
    ///
    /// `column` is which layer the box is in, counting from zero at the left.
    /// It is what tells the scorer how far an edge into this box has come, and
    /// therefore how many bends that edge is allowed: a renderer that does not
    /// think in layers should pass the rank it would have had.
    ///
    /// # Errors
    ///
    /// Returns a [`DrawingError`] when the rectangle is empty, outside the
    /// frame, duplicated, or over the drawing's item budget.
    pub fn boxed(
        &mut self,
        node: NodeId,
        column: usize,
        at: Rect,
    ) -> Result<&mut Self, DrawingError> {
        if self.inner.boxes.len() >= MAX_DRAWING_ITEMS {
            return Err(DrawingError::TooManyItems {
                limit: MAX_DRAWING_ITEMS,
            });
        }
        if at.w <= 0 || at.h <= 0 {
            return Err(DrawingError::InvalidRect { node, rect: at });
        }
        if self.inner.boxes.iter().any(|boxed| boxed.node == node) {
            return Err(DrawingError::DuplicateNode(node));
        }
        let right = i64::from(at.x) + i64::from(at.w);
        let bottom = i64::from(at.y) + i64::from(at.h);
        if at.x < 0
            || at.y < 0
            || right > i64::from(self.inner.width)
            || bottom > i64::from(self.inner.height)
        {
            return Err(DrawingError::BoxOutOfBounds { node, rect: at });
        }
        self.inner.boxes.push(Placed {
            node,
            column,
            x: at.x,
            y: at.y,
            w: at.w,
            h: at.h,
        });
        Ok(self)
    }

    /// Lays one edge down as an orthogonal polyline, corner to corner.
    ///
    /// The first point is where the line leaves its source and the last is
    /// where it arrives at its target; every point between is a right-angle
    /// turn. Two points are a straight line, three an **L**, four a **Z**.
    ///
    /// # Errors
    ///
    /// Returns a [`DrawingError`] when the route is duplicated, non-orthogonal,
    /// outside the frame, or exceeds a point or raster-work budget.
    pub fn route(
        &mut self,
        edge: EdgeId,
        points: impl IntoIterator<Item = (i32, i32)>,
    ) -> Result<&mut Self, DrawingError> {
        if self.inner.routes.len() >= MAX_DRAWING_ITEMS {
            return Err(DrawingError::TooManyItems {
                limit: MAX_DRAWING_ITEMS,
            });
        }
        if self.inner.routes.iter().any(|route| route.edge == edge) {
            return Err(DrawingError::DuplicateEdge(edge));
        }
        let points: Vec<_> = points
            .into_iter()
            .take(MAX_ROUTE_POINTS.saturating_add(1))
            .collect();
        validate_route(edge, &points, self.inner.width, self.inner.height)?;
        let all_points = self
            .inner
            .routes
            .iter()
            .try_fold(points.len(), |total, route| {
                total.checked_add(route.points.len())
            })
            .ok_or(DrawingError::RouteBudgetExceeded)?;
        let all_cells = self
            .inner
            .routes
            .iter()
            .try_fold(route_cells(&points)?, |total, route| {
                total.checked_add(route_cells(&route.points).ok()?)
            })
            .ok_or(DrawingError::RouteBudgetExceeded)?;
        if all_points > MAX_DRAWING_ROUTE_POINTS || all_cells > MAX_DRAWING_ROUTE_CELLS {
            return Err(DrawingError::RouteBudgetExceeded);
        }
        self.inner.routes.push(Route { edge, points });
        Ok(self)
    }

    /// The frame size this drawing was declared at.
    pub fn size(&self) -> (i32, i32) {
        (self.inner.width, self.inner.height)
    }

    /// Every box, in the order the nodes were added.
    pub fn boxes(&self) -> impl Iterator<Item = Boxed> + '_ {
        self.inner.boxes.iter().map(|b| Boxed {
            node: b.node,
            column: b.column,
            rect: Rect::new(b.x, b.y, b.w, b.h),
        })
    }

    /// The box that draws one node.
    pub fn boxed_at(&self, node: NodeId) -> Option<Boxed> {
        self.boxes().find(|b| b.node == node)
    }

    /// Every edge's line, in the order the edges were added.
    pub fn routes(&self) -> impl Iterator<Item = Routed<'_>> {
        self.inner.routes.iter().map(|r| Routed {
            edge: r.edge,
            points: &r.points,
            heading: r.heading(),
        })
    }

    /// Checks that every box and route belongs to the graph being drawn.
    ///
    /// # Errors
    ///
    /// Returns [`DrawingError::UnknownNode`] or [`DrawingError::UnknownEdge`]
    /// for an identifier the graph cannot resolve.
    pub fn validate(&self, graph: &crate::Graph) -> Result<(), DrawingError> {
        for boxed in &self.inner.boxes {
            if graph.node(boxed.node).is_none() {
                return Err(DrawingError::UnknownNode(boxed.node));
            }
        }
        for route in &self.inner.routes {
            if graph.edge(route.edge).is_none() {
                return Err(DrawingError::UnknownEdge(route.edge));
            }
        }
        Ok(())
    }

    pub(crate) fn from_layout(inner: Layout) -> Self {
        Self { inner }
    }

    pub(crate) fn layout(&self) -> &Layout {
        &self.inner
    }
}

fn validate_route(
    edge: EdgeId,
    points: &[(i32, i32)],
    width: i32,
    height: i32,
) -> Result<(), DrawingError> {
    if points.len() < 2 {
        return Err(DrawingError::RouteTooShort(edge));
    }
    if points.len() > MAX_ROUTE_POINTS {
        return Err(DrawingError::RouteTooLong {
            edge,
            points: points.len(),
            limit: MAX_ROUTE_POINTS,
        });
    }
    for &point in points {
        if point.0 < 0 || point.1 < 0 || point.0 >= width || point.1 >= height {
            return Err(DrawingError::RouteOutOfBounds { edge, point });
        }
    }
    for segment in points.windows(2) {
        let [from, to] = segment else { continue };
        if from.0 != to.0 && from.1 != to.1 {
            return Err(DrawingError::DiagonalSegment {
                edge,
                from: *from,
                to: *to,
            });
        }
    }
    route_cells(points)?;
    Ok(())
}

fn route_cells(points: &[(i32, i32)]) -> Result<u64, DrawingError> {
    points
        .windows(2)
        .try_fold(0u64, |total, segment| {
            let [from, to] = segment else {
                return Some(total);
            };
            let cells = i64::from(from.0).abs_diff(i64::from(to.0))
                + i64::from(from.1).abs_diff(i64::from(to.1))
                + 1;
            total.checked_add(cells)
        })
        .filter(|cells| *cells <= MAX_DRAWING_ROUTE_CELLS)
        .ok_or(DrawingError::RouteBudgetExceeded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Graph, Node};
    use crate::options::Options;

    fn drawing(width: i32, height: i32) -> Drawing {
        Drawing::new(width, height).expect("the test frame is valid")
    }

    /// Two boxes, one straight line: the drawing every metric is calibrated to
    /// charge nothing for.
    #[test]
    fn a_straight_edge_costs_nothing() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        let edge = g.add_edge(a, b).unwrap();

        let mut drawing = drawing(20, 3);
        drawing.boxed(a, 0, Rect::new(0, 0, 5, 3)).unwrap();
        drawing.boxed(b, 1, Rect::new(15, 0, 5, 3)).unwrap();
        drawing.route(edge, [(5, 1), (14, 1)]).unwrap();

        let score = crate::score_drawing(&g, &drawing).unwrap();
        assert_eq!(score.vocabulary(), [0; 4]);
        assert_eq!(score.total, 0);
        assert_eq!(score.ink, 10);
    }

    /// A drawing the library would never emit still gets counted: two bends is
    /// a Z, which the vocabulary allows between neighbouring columns, and a Z
    /// that only goes as far down as its target costs no detour.
    #[test]
    fn a_z_between_neighbours_is_charged_its_detour_and_no_more() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        let edge = g.add_edge(a, b).unwrap();

        let mut drawing = drawing(20, 6);
        drawing.boxed(a, 0, Rect::new(0, 0, 5, 3)).unwrap();
        drawing.boxed(b, 1, Rect::new(15, 3, 5, 3)).unwrap();
        drawing
            .route(edge, [(5, 1), (10, 1), (10, 4), (14, 4)])
            .unwrap();

        let score = crate::score_drawing(&g, &drawing).unwrap();
        assert_eq!(score.vocabulary(), [0; 4]);
        assert_eq!(
            score.detour, 0,
            "three rows down is three rows it had to go"
        );
    }

    /// The same graph, drawn twice, scored once each: the point of the port.
    #[test]
    fn a_worse_drawing_of_the_same_graph_scores_worse() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        let edge = g.add_edge(a, b).unwrap();

        let boxes = |d: &mut Drawing| {
            d.boxed(a, 0, Rect::new(0, 0, 5, 3)).unwrap();
            d.boxed(b, 1, Rect::new(15, 0, 5, 3)).unwrap();
        };

        let mut straight = drawing(20, 8);
        boxes(&mut straight);
        straight.route(edge, [(5, 1), (14, 1)]).unwrap();

        let mut wandering = drawing(20, 8);
        boxes(&mut wandering);
        wandering
            .route(edge, [(5, 1), (8, 1), (8, 6), (12, 6), (12, 1), (14, 1)])
            .unwrap();

        assert!(
            crate::score_drawing(&g, &wandering).unwrap().total
                > crate::score_drawing(&g, &straight).unwrap().total
        );
    }

    /// Two untagged edges crossing are still two lines.
    ///
    /// They share an ink because neither has one, which is not the same as
    /// belonging to the same flow: nothing about them says they are one line,
    /// and a reader following either through a junction cannot tell which way it
    /// went. The break is what tells them apart, and it is the only thing that
    /// can, since there is no colour to do it.
    #[test]
    fn two_untagged_edges_crossing_are_told_apart() {
        let mut g = Graph::new();
        let ids: Vec<_> = ["a", "b", "c", "d"]
            .iter()
            .map(|n| g.add_node(Node::new(*n)))
            .collect();
        let across = g.add_edge(ids[0], ids[1]).unwrap();
        let down = g.add_edge(ids[2], ids[3]).unwrap();

        let mut drawing = drawing(9, 6);
        drawing.boxed(ids[0], 0, Rect::new(0, 1, 3, 3)).unwrap();
        drawing.boxed(ids[1], 1, Rect::new(6, 1, 3, 3)).unwrap();
        drawing.boxed(ids[2], 0, Rect::new(3, 0, 2, 2)).unwrap();
        drawing.boxed(ids[3], 1, Rect::new(3, 4, 2, 2)).unwrap();
        drawing.route(across, [(3, 2), (5, 2)]).unwrap();
        drawing.route(down, [(4, 0), (4, 4)]).unwrap();

        let art = crate::draw_drawing(&g, &drawing, Options::default()).unwrap();
        assert!(
            art.contains('╴') || art.contains('╶'),
            "the crossing reads as one line:\n{art}"
        );
    }

    /// Two branches of one fan keep their junction.
    ///
    /// These really are one line — one ink, and a box they both leave — so the
    /// glyph that says so is the honest one, and the tidier one.
    #[test]
    fn two_branches_of_one_fan_keep_their_junction() {
        let mut g = Graph::new();
        let ids: Vec<_> = ["a", "b", "c"]
            .iter()
            .map(|n| g.add_node(Node::new(*n)))
            .collect();
        let up = g.add_edge(ids[0], ids[1]).unwrap();
        let down = g.add_edge(ids[0], ids[2]).unwrap();

        let mut drawing = drawing(9, 5);
        drawing.boxed(ids[0], 0, Rect::new(0, 1, 3, 3)).unwrap();
        drawing.boxed(ids[1], 1, Rect::new(6, 0, 3, 2)).unwrap();
        drawing.boxed(ids[2], 1, Rect::new(6, 3, 3, 2)).unwrap();
        drawing.route(up, [(3, 2), (5, 2)]).unwrap();
        drawing.route(down, [(4, 1), (4, 4)]).unwrap();

        let art = crate::draw_drawing(&g, &drawing, Options::default()).unwrap();
        assert!(
            !art.contains('╴') && !art.contains('╶'),
            "a fan was broken apart:\n{art}"
        );
    }

    /// A box hides what runs under it.
    ///
    /// A route beneath a box is a defect, and the drawing says so by stopping
    /// the line at the box rather than running it through — otherwise a reader
    /// follows a line somewhere it does not go. `design.md` §4.7.
    #[test]
    fn a_box_hides_what_runs_under_it() {
        let mut g = Graph::new();
        let ids: Vec<_> = ["a", "b", "o"]
            .iter()
            .map(|n| g.add_node(Node::new(*n)))
            .collect();
        let beneath = g.add_tagged_edge(ids[0], ids[1], ["t"]).unwrap();

        let mut drawing = drawing(9, 7);
        drawing.boxed(ids[2], 0, Rect::new(2, 1, 5, 5)).unwrap();
        // Straight down the middle: in at the top of the box, out at the bottom.
        drawing.route(beneath, [(4, 0), (4, 6)]).unwrap();

        let art = crate::draw_drawing(&g, &drawing, Options::default()).unwrap();
        let rows: Vec<Vec<char>> = art.lines().map(|row| row.chars().collect()).collect();
        let across = |row: usize| -> String {
            rows.get(row)
                .map(|row| row.iter().skip(2).take(5).collect())
                .unwrap_or_default()
        };

        assert_eq!(across(1), "┌───┐", "the top border:\n{art}");
        assert_eq!(across(2), "│ o │", "the line shows through:\n{art}");
        assert_eq!(across(3), "│   │", "the line shows through:\n{art}");
        assert_eq!(across(5), "└───┘", "the bottom border:\n{art}");
        assert!(art.starts_with("    │"), "it should still arrive:\n{art}");
    }

    /// What the library lays out reads back as what it paints: a box per node,
    /// a route per edge, and the same picture either way round.
    #[test]
    fn a_layout_reads_back_what_paint_draws() {
        let mut g = Graph::new();
        let ids: Vec<_> = ["in", "work", "retry"]
            .iter()
            .map(|n| g.add_node(Node::new(*n)))
            .collect();
        g.add_edge(ids[0], ids[1]).unwrap();
        g.add_tagged_edge(ids[1], ids[2], ["t"]).unwrap();
        g.add_edge(ids[2], ids[1]).unwrap();
        let options = Options::default();

        let drawing = crate::layout(&g, options);
        assert_eq!(drawing.boxes().count(), 3);
        assert_eq!(drawing.routes().count(), 3);
        assert_eq!(
            drawing.boxes().map(|b| b.node).collect::<Vec<_>>(),
            ids,
            "boxes come back in node order"
        );
        let first = drawing.boxed_at(ids[0]).map(|b| b.column);
        assert_eq!(first, Some(0));
        assert_eq!(
            crate::draw_drawing(&g, &drawing, options).unwrap(),
            crate::draw_with(&g, options)
        );
    }

    /// A back edge comes up out of its lane, and says so.
    #[test]
    fn a_back_edge_reports_heading_up() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        let forward = g.add_edge(a, b).unwrap();
        let back = g.add_edge(b, a).unwrap();

        let drawing = crate::layout(&g, Options::default());
        let heading = |edge| drawing.routes().find(|r| r.edge == edge).map(|r| r.heading);
        assert_eq!(heading(forward), Some(Heading::Right));
        assert_eq!(heading(back), Some(Heading::Up));
    }

    /// A fixed width applies to every box, whatever its text.
    #[test]
    fn a_box_width_fixes_every_column() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("a much longer label than the other"));
        g.add_edge(a, b).unwrap();

        let drawing = crate::layout(&g, Options::new().box_width(18));
        assert!(drawing.boxes().all(|b| b.rect.w == 18), "{drawing:?}");
        let art = crate::draw_with(&g, Options::new().box_width(18));
        assert!(art.contains('…'), "the long label is clipped:\n{art}");
    }

    /// A box height is a floor: a box grows past it when it needs the rows.
    #[test]
    fn a_box_height_is_a_floor() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let tall = g.add_node(Node::new("b").line("1").line("2").line("3").line("4"));
        g.add_edge(a, tall).unwrap();

        let drawing = crate::layout(&g, Options::new().box_height(5));
        let height = |node| drawing.boxed_at(node).map(|b| b.rect.h);
        assert_eq!(height(a), Some(5));
        assert_eq!(height(tall), Some(7), "four lines need seven rows");
    }

    #[test]
    fn an_invalid_frame_is_rejected_before_it_allocates() {
        assert!(matches!(
            Drawing::new(0, 2),
            Err(DrawingError::InvalidSize { .. })
        ));
        assert!(matches!(
            Drawing::new(2_001, 2_000),
            Err(DrawingError::FrameTooLarge { .. })
        ));
    }

    #[test]
    fn boxes_must_be_positive_unique_and_inside_the_frame() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let mut drawing = drawing(4, 2);

        assert!(matches!(
            drawing.boxed(a, 0, Rect::new(0, 0, 0, 2)),
            Err(DrawingError::InvalidRect { .. })
        ));
        assert!(matches!(
            drawing.boxed(a, 0, Rect::new(i32::MAX, 0, 2, 2)),
            Err(DrawingError::BoxOutOfBounds { .. })
        ));
        drawing.boxed(a, 0, Rect::new(0, 0, 2, 2)).unwrap();
        assert!(matches!(
            drawing.boxed(a, 0, Rect::new(2, 0, 2, 2)),
            Err(DrawingError::DuplicateNode(node)) if node == a
        ));
    }

    #[test]
    fn routes_must_be_bounded_orthogonal_and_unique() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        let edge = g.add_edge(a, b).unwrap();
        let mut drawing = drawing(4, 2);

        assert!(matches!(
            drawing.route(edge, [(0, 0)]),
            Err(DrawingError::RouteTooShort(_))
        ));
        assert!(matches!(
            drawing.route(edge, [(0, 0), (2, 1)]),
            Err(DrawingError::DiagonalSegment { .. })
        ));
        assert!(matches!(
            drawing.route(edge, [(-1, 0), (2, 0)]),
            Err(DrawingError::RouteOutOfBounds { .. })
        ));
        drawing.route(edge, [(0, 0), (3, 0)]).unwrap();
        assert!(matches!(
            drawing.route(edge, [(0, 1), (3, 1)]),
            Err(DrawingError::DuplicateEdge(found)) if found == edge
        ));
    }

    #[test]
    fn aggregate_route_work_is_bounded() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        let edge = g.add_edge(a, b).unwrap();
        let mut drawing = drawing(2_000, 1);
        let points = (0..MAX_ROUTE_POINTS).map(|at| if at % 2 == 0 { (0, 0) } else { (1_999, 0) });

        assert!(matches!(
            drawing.route(edge, points),
            Err(DrawingError::RouteBudgetExceeded)
        ));
    }

    #[test]
    fn drawing_consumers_reject_ids_outside_the_graph() {
        let g = Graph::new();
        let mut drawing = drawing(2, 2);
        let missing = NodeId::from_index(7);
        drawing.boxed(missing, 0, Rect::new(0, 0, 2, 2)).unwrap();

        assert_eq!(
            crate::score_drawing(&g, &drawing),
            Err(DrawingError::UnknownNode(missing))
        );
        assert_eq!(
            crate::draw_drawing(&g, &drawing, Options::default()),
            Err(DrawingError::UnknownNode(missing))
        );
    }
}
