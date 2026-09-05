//! What a caller can ask for.

/// How a cell where two edges genuinely pass each other is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Crossing {
    /// The union of both edges' bits, `───┼───`.
    ///
    /// The same glyph a fork leaves where a straight sibling passes through, so
    /// a crossing and a junction look alike. That is the cost, and it is the
    /// default because it is what the box-drawing block is for.
    #[default]
    Cross,
    /// The vertical reads as passing over: `──╴│╶──`.
    ///
    /// The horizontal is cut one cell either side, and only where the neighbour
    /// is plain line — a corner or a junction beside a crossing stays put.
    Bridge,
}

/// How a drawing should be made.
///
/// Everything here has a default that is the plainest reading of the graph, so
/// `Options::default()` draws what most callers want and each field is
/// something asked for deliberately.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Options {
    /// Draw each edge's tags on it.
    ///
    /// Off by default: tags are what colour is for, and printing them as well
    /// costs a gap wide enough to hold the longest of them. Worth it when the
    /// output is going somewhere colour cannot follow.
    pub labels: bool,
    /// How a cell where two edges pass each other is drawn.
    pub crossings: Crossing,
    /// Columns to fit the drawing into, if the caller has a limit.
    ///
    /// A drawing has a natural width. Asking for less makes it shrink through a
    /// fixed ladder of steps down to a legibility floor; if even the floor is
    /// too wide, the narrowest attempt is what comes back. Nothing is ever
    /// clipped, because half a box is worse than a wide one.
    pub width: Option<usize>,
}

impl Options {
    /// The defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Draws each edge's tags on it.
    #[must_use]
    pub fn labels(mut self, on: bool) -> Self {
        self.labels = on;
        self
    }

    /// Chooses how a crossing is drawn.
    #[must_use]
    pub fn crossings(mut self, style: Crossing) -> Self {
        self.crossings = style;
        self
    }

    /// Asks for a drawing no wider than this, if one can be had.
    #[must_use]
    pub fn width(mut self, columns: usize) -> Self {
        self.width = Some(columns);
        self
    }
}
