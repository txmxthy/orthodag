//! Draw a directed graph as Unicode box-drawing text.
//!
//! Boxes for the vertices, orthogonal lines for the edges, fitted to a target
//! width, with an explicit objective for what a good drawing looks like.
//!
//! See `docs/design.md` for what this is meant to be and `docs/plan.md` for the
//! order it is being built in. The model and the first layout phase exist so
//! far; nothing draws.

#![warn(missing_docs)]

pub mod graph;

// Phases land before there is a `layout()` to call them from; the allow comes
// off with the function that ties them together.
#[allow(dead_code)]
mod layout;

pub use graph::{Edge, EdgeId, Graph, Node, NodeId};
