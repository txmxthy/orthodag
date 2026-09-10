//! Turning a layout into characters.
//!
//! The layout hands over boxes and orthogonal polylines in cell coordinates and
//! no character at all. This is where they become box-drawing glyphs.

mod canvas;
mod glyph;
pub(crate) mod grid;

pub use canvas::Span;
pub(crate) use canvas::{Canvas, Heading};

use crate::colour;
use crate::graph::Graph;
use crate::layout::route::Layout;
use crate::options::{Crossing, Options};
use crate::score;

/// Draws a laid-out graph.
///
/// Routes first, boxes over them: a box is opaque, and an edge that ends up
/// running under one should be hidden by it rather than drawn through it, so
/// the defect shows as a line that stops rather than a box full of holes.
pub(crate) fn draw(g: &Graph, layout: &Layout, options: Options) -> Canvas {
    let width = usize::try_from(layout.width).unwrap_or(0);
    let height = usize::try_from(layout.height).unwrap_or(0);
    let mut canvas = Canvas::new(width, height);

    let colours = colour::of(g);
    for route in &layout.routes {
        let ink = colours.get(route.edge.index()).copied().flatten();
        canvas.path(&route.points, ink);
        if let Some((x, y)) = route.points.last() {
            // A forward edge arrives from the left; a back edge comes up out of
            // its lane. The last segment says which.
            let facing = match route.points.iter().nth_back(1) {
                Some(before) if before.0 == *x && before.1 > *y => Heading::Up,
                _ => Heading::Right,
            };
            canvas.head(*x, *y, facing, ink);
        }
    }

    match options.crossings {
        Crossing::Bridge => canvas.bridge(&score::crossing_cells(g, layout)),
        Crossing::ByColour => canvas.bridge(&score::parted_crossing_cells(g, layout)),
        Crossing::Cross => {}
    }

    for label in &layout.labels {
        let ink = colours.get(label.edge.index()).copied().flatten();
        canvas.write_over(label.x, label.y, &label.text, ink);
    }

    for boxed in &layout.boxes {
        canvas.rect(boxed.x, boxed.y, boxed.w, boxed.h);
        let Some(node) = g.node(boxed.node) else {
            continue;
        };
        let room = usize::try_from(boxed.w - 4).unwrap_or(0);
        canvas.write(boxed.x + 2, boxed.y + 1, &clip(node.label(), room));
        for (step, line) in node.lines().iter().enumerate() {
            let Ok(step) = i32::try_from(step) else { break };
            canvas.write(boxed.x + 2, boxed.y + 2 + step, &clip(line, room));
        }
    }

    canvas
}

/// Cuts a line of text to fit its box, saying so where it had to.
///
/// The box was sized to hold this already unless the drawing is being squeezed,
/// in which case the reader is better served by an ellipsis than by text
/// spilling through a border.
fn clip(text: &str, room: usize) -> String {
    if text.chars().count() <= room {
        return text.to_owned();
    }
    text.chars()
        .take(room.saturating_sub(1))
        .chain(std::iter::once('…'))
        .collect()
}
