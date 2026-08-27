//! Turning a layout into characters.
//!
//! The layout hands over boxes and orthogonal polylines in cell coordinates and
//! no character at all. This is where they become box-drawing glyphs.

mod canvas;
mod glyph;
mod grid;

pub(crate) use canvas::Canvas;

use crate::graph::Graph;
use crate::layout::route::Layout;

/// Draws a laid-out graph.
///
/// Routes first, boxes over them: a box is opaque, and an edge that ends up
/// running under one should be hidden by it rather than drawn through it, so
/// the defect shows as a line that stops rather than a box full of holes.
pub(crate) fn draw(g: &Graph, layout: &Layout) -> Canvas {
    let width = usize::try_from(layout.width).unwrap_or(0);
    let height = usize::try_from(layout.height).unwrap_or(0);
    let mut canvas = Canvas::new(width, height);

    for route in &layout.routes {
        canvas.path(&route.points);
        if let Some((x, y)) = route.points.last() {
            canvas.head(*x, *y);
        }
    }

    for boxed in &layout.boxes {
        canvas.rect(boxed.x, boxed.y, boxed.w, boxed.h);
        let Some(node) = g.node(boxed.node) else {
            continue;
        };
        canvas.write(boxed.x + 2, boxed.y + 1, node.label());
        for (step, line) in node.lines().iter().enumerate() {
            let Ok(step) = i32::try_from(step) else { break };
            canvas.write(boxed.x + 2, boxed.y + 2 + step, line);
        }
    }

    canvas
}
