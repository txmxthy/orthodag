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
//! let mut g = Graph::new();
//! let a = g.add_node(Node::new("a"));
//! let b = g.add_node(Node::new("b"));
//! let edge = g.add_edge(a, b);
//!
//! let mut drawing = Drawing::new(20, 3);
//! drawing.boxed(a, 0, Rect::new(0, 0, 5, 3));
//! drawing.boxed(b, 1, Rect::new(15, 0, 5, 3));
//! drawing.route(edge, [(5, 1), (14, 1)]);
//!
//! // A straight edge between two boxes: nothing to charge for.
//! assert_eq!(orthodag::score_drawing(&g, &drawing).total, 0);
//! ```
//!
//! # What is not checked
//!
//! A `Drawing` is taken at its word. Nothing verifies that a route ends on the
//! box it claims, that the polyline is orthogonal, or that the cells fit the
//! size given — a diagonal segment is skipped by the rasteriser and a cell
//! outside the size is dropped, silently, because a scorer that panics on
//! unexpected input is a scorer nobody runs twice. Points are in the same cell
//! coordinates the drawing is measured in: `x` rightward, `y` downward, origin
//! top left, both inclusive of the cells the line actually occupies.

use crate::graph::{EdgeId, NodeId};
use crate::layout::route::{Boxed, Layout, Route};

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

/// A finished drawing: boxes, orthogonal polylines, and how big the frame is.
///
/// Built up with [`boxed`](Self::boxed) and [`route`](Self::route), then handed
/// to [`score_drawing`](crate::score_drawing).
#[derive(Clone, Debug, Default)]
pub struct Drawing {
    inner: Layout,
}

impl Drawing {
    /// An empty drawing of a given size, in cells.
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            inner: Layout {
                width,
                height,
                ..Layout::default()
            },
        }
    }

    /// Places the box that draws one node.
    ///
    /// `column` is which layer the box is in, counting from zero at the left.
    /// It is what tells the scorer how far an edge into this box has come, and
    /// therefore how many bends that edge is allowed: a renderer that does not
    /// think in layers should pass the rank it would have had.
    pub fn boxed(&mut self, node: NodeId, column: usize, at: Rect) -> &mut Self {
        self.inner.boxes.push(Boxed {
            node,
            column,
            x: at.x,
            y: at.y,
            w: at.w,
            h: at.h,
        });
        self
    }

    /// Lays one edge down as an orthogonal polyline, corner to corner.
    ///
    /// The first point is where the line leaves its source and the last is
    /// where it arrives at its target; every point between is a right-angle
    /// turn. Two points are a straight line, three an **L**, four a **Z**.
    pub fn route(
        &mut self,
        edge: EdgeId,
        points: impl IntoIterator<Item = (i32, i32)>,
    ) -> &mut Self {
        self.inner.routes.push(Route {
            edge,
            points: points.into_iter().collect(),
        });
        self
    }

    /// The frame size this drawing was declared at.
    pub fn size(&self) -> (i32, i32) {
        (self.inner.width, self.inner.height)
    }

    pub(crate) fn layout(&self) -> &Layout {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Graph, Node};
    use crate::options::Options;

    /// Two boxes, one straight line: the drawing every metric is calibrated to
    /// charge nothing for.
    #[test]
    fn a_straight_edge_costs_nothing() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        let edge = g.add_edge(a, b);

        let mut drawing = Drawing::new(20, 3);
        drawing.boxed(a, 0, Rect::new(0, 0, 5, 3));
        drawing.boxed(b, 1, Rect::new(15, 0, 5, 3));
        drawing.route(edge, [(5, 1), (14, 1)]);

        let score = crate::score_drawing(&g, &drawing);
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
        let edge = g.add_edge(a, b);

        let mut drawing = Drawing::new(20, 6);
        drawing.boxed(a, 0, Rect::new(0, 0, 5, 3));
        drawing.boxed(b, 1, Rect::new(15, 3, 5, 3));
        drawing.route(edge, [(5, 1), (10, 1), (10, 4), (14, 4)]);

        let score = crate::score_drawing(&g, &drawing);
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
        let edge = g.add_edge(a, b);

        let boxes = |d: &mut Drawing| {
            d.boxed(a, 0, Rect::new(0, 0, 5, 3));
            d.boxed(b, 1, Rect::new(15, 0, 5, 3));
        };

        let mut straight = Drawing::new(20, 8);
        boxes(&mut straight);
        straight.route(edge, [(5, 1), (14, 1)]);

        let mut wandering = Drawing::new(20, 8);
        boxes(&mut wandering);
        wandering.route(edge, [(5, 1), (8, 1), (8, 6), (12, 6), (12, 1), (14, 1)]);

        assert!(
            crate::score_drawing(&g, &wandering).total > crate::score_drawing(&g, &straight).total
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
        let across = g.add_edge(ids[0], ids[1]);
        let down = g.add_edge(ids[2], ids[3]);

        let mut drawing = Drawing::new(9, 5);
        drawing.boxed(ids[0], 0, Rect::new(0, 1, 3, 3));
        drawing.boxed(ids[1], 1, Rect::new(6, 1, 3, 3));
        drawing.boxed(ids[2], 0, Rect::new(3, 0, 2, 2));
        drawing.boxed(ids[3], 1, Rect::new(3, 4, 2, 2));
        drawing.route(across, [(3, 2), (5, 2)]);
        drawing.route(down, [(4, 0), (4, 4)]);

        let art = crate::draw_drawing(&g, &drawing, Options::default());
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
        let up = g.add_edge(ids[0], ids[1]);
        let down = g.add_edge(ids[0], ids[2]);

        let mut drawing = Drawing::new(9, 5);
        drawing.boxed(ids[0], 0, Rect::new(0, 1, 3, 3));
        drawing.boxed(ids[1], 1, Rect::new(6, 0, 3, 2));
        drawing.boxed(ids[2], 1, Rect::new(6, 3, 3, 2));
        drawing.route(up, [(3, 2), (5, 2)]);
        drawing.route(down, [(4, 1), (4, 4)]);

        let art = crate::draw_drawing(&g, &drawing, Options::default());
        assert!(
            !art.contains('╴') && !art.contains('╶'),
            "a fan was broken apart:\n{art}"
        );
    }

    /// A cell outside the frame is dropped rather than panicking. A caller's
    /// drawing is not trusted input.
    #[test]
    fn a_route_off_the_frame_is_dropped_not_fatal() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        let edge = g.add_edge(a, b);

        let mut drawing = Drawing::new(4, 2);
        drawing.boxed(a, 0, Rect::new(0, 0, 2, 2));
        drawing.boxed(b, 1, Rect::new(2, 0, 2, 2));
        drawing.route(edge, [(-5, 0), (900, 0)]);

        assert_eq!(crate::score_drawing(&g, &drawing).ink, 4);
    }
}
