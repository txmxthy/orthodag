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

pub mod graph;

// Phases land before there is a `layout()` to call them from; the allow comes
// off with the function that ties them together.
#[allow(dead_code)]
mod layout;
#[allow(dead_code)]
mod paint;

pub use graph::{Edge, EdgeId, Graph, Node, NodeId};

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
    paint::draw(graph, &layout::build(graph)).to_string()
}
