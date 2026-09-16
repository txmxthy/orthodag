//! Draw a directed graph as Unicode box-drawing text.
//!
//! Boxes for the vertices, orthogonal lines for the edges, fitted to a target
//! width, with an explicit objective for what a good drawing looks like.
//!
//! See `docs/design.md` for what this is meant to be and `docs/plan.md` for the
//! order it is being built in.

#![warn(missing_docs)]
// The no-unwrap rule is about the library, not its tests: a test that cannot
// reach its own precondition should stop there and say so.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod colour;
pub mod defect;
pub mod drawing;
pub mod graph;
pub mod io;
pub mod options;

// Phases land before there is a `layout()` to call them from; the allow comes
// off with the function that ties them together.
// Five helpers under here are unused and were unused before this allow was
// needed for the phases; they are somebody's to keep or drop, not this change's.
#[allow(dead_code)]
mod layout;
#[allow(dead_code)]
mod paint;
mod score;

pub use defect::{Defect, Fault};
pub use drawing::{Boxed, Drawing, Heading, Rect, Routed};
pub use layout::Phases;
pub use paint::{Part, Span};
pub use score::Score;

pub use colour::{Colour, PALETTE};

pub use graph::{Edge, EdgeId, Graph, Node, NodeId};
pub use options::{Crossing, Options};

/// Draws a graph as box-drawing text.
///
/// # Example
///
/// ```
/// use orthodag::{Graph, Node};
///
/// let mut g = Graph::new();
/// let a = g.add_node(Node::new("a"));
/// let b = g.add_node(Node::new("b"));
/// g.add_edge(a, b);
///
/// print!("{}", orthodag::draw(&g));
/// ```
///
/// This is not the shape the surface ends up in. `docs/design.md` has layout,
/// drawing and scoring as three calls, so a caller can colour one edge or ask
/// what a drawing is worth; none of that exists yet, and one function that
/// returns a string is honest about that.
pub fn draw(graph: &Graph) -> String {
    draw_with(graph, Options::default())
}

/// Draws a graph, with options.
pub fn draw_with(graph: &Graph, options: Options) -> String {
    paint::draw(graph, &layout::build(graph, options), options).to_string()
}

/// Draws a graph as styled runs, one list per row.
///
/// The same drawing [`draw`] returns, cut into runs of one colour so a caller
/// can paint a flow without this library ever knowing what a colour is. A span
/// with no colour is a box, a label, or a cell where two flows met: paint those
/// in whatever the default ink is.
///
/// # Example
///
/// ```
/// use orthodag::{Graph, Node};
///
/// let mut g = Graph::new();
/// let a = g.add_node(Node::new("a"));
/// let b = g.add_node(Node::new("b"));
/// g.add_tagged_edge(a, b, ["even"]);
///
/// for row in orthodag::spans(&g) {
///     for span in row {
///         match span.colour {
///             Some(slot) => print!("\x1b[3{}m{}\x1b[0m", slot.slot() + 1, span.text),
///             None => print!("{}", span.text),
///         }
///     }
///     println!();
/// }
/// ```
pub fn spans(graph: &Graph) -> Vec<Vec<Span>> {
    spans_with(graph, Options::default())
}

/// Draws a graph as styled runs, with options.
pub fn spans_with(graph: &Graph, options: Options) -> Vec<Vec<Span>> {
    paint::draw(graph, &layout::build(graph, options), options).runs()
}

/// What the drawing of a graph is worth.
///
/// See [`Score`] for what the numbers mean. The short version: the vocabulary
/// tier is categorical and must be zero, the total is the scalar to push down,
/// and the rest is reported because it is useful to know, not because it is a
/// goal.
pub fn score(graph: &Graph) -> Score {
    score_with(graph, Options::default())
}

/// What the drawing of a graph is worth, with options.
pub fn score_with(graph: &Graph, options: Options) -> Score {
    score::score(graph, &layout::build(graph, options))
}

/// Lays a graph out and hands back the drawing it would paint.
///
/// For a caller whose boxes are widgets of its own: the boxes and lines this
/// library would draw, in cell coordinates, without a character in them. The
/// same [`Drawing`] goes back into [`draw_drawing`] unchanged.
pub fn layout(graph: &Graph, options: Options) -> Drawing {
    Drawing::from_layout(layout::build(graph, options))
}

/// Paints a drawing built by the caller, with this library's glyphs.
///
/// The companion to [`score_drawing`]: a caller's boxes and polylines drawn by
/// the same painter this library uses, so two drawings of one graph can be put
/// side by side and differ only in their layout.
///
/// The graph supplies what goes inside the boxes; the drawing says where they
/// are.
pub fn draw_drawing(graph: &Graph, drawing: &Drawing, options: Options) -> String {
    paint::draw(graph, drawing.layout(), options).to_string()
}

/// What a drawing built by the caller is worth.
///
/// The same objective [`score`] applies to this library's own output, applied
/// to a [`Drawing`] a caller built instead. One implementation of the metrics
/// serves both.
///
/// The graph is still needed — which edges share a source, how many columns an
/// edge crosses and therefore how many bends it is allowed are facts about the
/// graph, not about the picture.
pub fn score_drawing(graph: &Graph, drawing: &Drawing) -> Score {
    score::score(graph, drawing.layout())
}

/// Everything wrong with the drawing of a graph, and who is responsible.
///
/// [`score`] says how much; this says where and between which two edges, which
/// is the difference between knowing a drawing is bad and knowing what to
/// change. See [`Defect`].
pub fn defects(graph: &Graph) -> Vec<Defect> {
    defects_with(graph, Options::default())
}

/// Everything wrong with the drawing of a graph, with options.
pub fn defects_with(graph: &Graph, options: Options) -> Vec<Defect> {
    score::defects(graph, &layout::build(graph, options))
}

/// Where the time goes laying a graph out.
///
/// The phases are separable and their costs are not remotely equal. Which ones
/// matter stops being a guess once it is measured, and a guess is all a budget
/// is until then — see [`Phases`].
pub fn phases(graph: &Graph) -> Phases {
    layout::phases(graph, Options::default())
}
