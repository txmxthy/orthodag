//! Where a drawing goes wrong, rather than how much.
//!
//! [`Score`](crate::Score) counts. Counting is what a search descends and what a
//! test asserts, and it is useless the moment the question becomes *why*: a
//! drawing with eleven crossings and a drawing with eleven crossings can need
//! opposite fixes.
//!
//! So the same pass that counts also says where, and between which two edges.
//! That is enough to tell the three cases apart, and they are fixed in three
//! different phases:
//!
//! - two edges **out of one box** crossing each other is a fan crossing itself,
//!   which is the attach rows being handed out in the wrong order;
//! - two edges **into one box** crossing is the same thing at the other end;
//! - two **unrelated** edges crossing is the columns being ordered badly, and is
//!   the only one of the three that is sometimes unavoidable.
//!
//! A caller that draws on a screen can use the same list to point at what it is
//! complaining about.

use crate::graph::{EdgeId, Graph, NodeId};

/// What is wrong in one cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fault {
    /// Two edges pass each other at a right angle.
    ///
    /// Legal, and counted rather than forbidden, but every one of them is a
    /// place a reader has to work out which line is which.
    Crossing,
    /// Two edges share a cell in some way that is not a crossing.
    ///
    /// Categorical: it draws two edges as one line and lies to the reader.
    Overlap,
    /// A fork or join run carrying two flows at once.
    ///
    /// The trunk is real, but it has swallowed a colour: the cell holds one
    /// character, and the flow that lost it is gone for the length of the run.
    Blend,
}

/// One thing wrong, and who is responsible.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Defect {
    /// The cell it happens in.
    pub at: (i32, i32),
    /// What is wrong there.
    pub fault: Fault,
    /// The two edges involved, lowest id first.
    pub edges: (EdgeId, EdgeId),
}

impl Defect {
    /// The box both edges leave, where they leave the same one.
    ///
    /// A crossing between two branches of one fan is the fan crossing itself,
    /// which is a different problem from two strangers passing: nothing about
    /// the graph requires it, only the order the branches were given.
    pub fn same_source(&self, g: &Graph) -> Option<NodeId> {
        let (a, b) = (g.edge(self.edges.0)?, g.edge(self.edges.1)?);
        (a.from() == b.from()).then(|| a.from())
    }

    /// The box both edges arrive at, where they arrive at the same one.
    pub fn same_target(&self, g: &Graph) -> Option<NodeId> {
        let (a, b) = (g.edge(self.edges.0)?, g.edge(self.edges.1)?);
        (a.to() == b.to()).then(|| a.to())
    }
}
