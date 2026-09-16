//! What a caller can ask for.

/// How a cell where two edges genuinely pass each other is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Crossing {
    /// A bridge where the two flows differ, a junction where they do not.
    ///
    /// The default, because a junction glyph says "these are one line" and
    /// where the two runs belong to different flows that is a lie the reader
    /// cannot see through — the cell holds one character and therefore one of
    /// the two colours. Where both runs are the same colour there is nothing to
    /// tell apart, and `┼` is then the honest glyph as well as the tidier one.
    #[default]
    ByColour,
    /// The union of both edges' bits, `───┼───`, everywhere.
    ///
    /// The same glyph a fork leaves where a straight sibling passes through, so
    /// a crossing and a junction look alike. What the box-drawing block is for,
    /// and right when nothing is coloured.
    Cross,
    /// The vertical reads as passing over: `──╴│╶──`, at every crossing.
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
    /// Draw every box this many cells wide, whatever its text.
    ///
    /// For a caller whose boxes are widgets of a fixed size. Text that does
    /// not fit is clipped with an ellipsis, as it is when a drawing is squeezed.
    pub box_width: Option<usize>,
    /// Draw no box shorter than this many rows, borders included.
    ///
    /// A box grows past it when its text or its flows need more rows.
    pub box_height: Option<usize>,
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

    /// Draws every box this many cells wide.
    #[must_use]
    pub fn box_width(mut self, cells: usize) -> Self {
        self.box_width = Some(cells);
        self
    }

    /// Draws no box shorter than this many rows.
    #[must_use]
    pub fn box_height(mut self, rows: usize) -> Self {
        self.box_height = Some(rows);
        self
    }
}
