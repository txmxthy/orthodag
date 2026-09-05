//! What a caller can ask for.

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
}
