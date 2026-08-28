//! The cell grid the lines are accumulated on.
//!
//! Every cell holds four bits, one per direction, and a bit means "a line
//! leaves this cell that way". A run of line sets bits along its length; runs
//! are OR-ed into whatever is already there.
//!
//! That is the whole model. Corners and junctions are never drawn on purpose: a
//! corner is what a horizontal run and a vertical run leave behind where they
//! meet. Runs can therefore be painted in any order, and a fork and a crossing
//! take the same code path.

/// A line leaves this cell to the left.
pub(crate) const L: u8 = 1;
/// A line leaves this cell to the right.
pub(crate) const R: u8 = 2;
/// A line leaves this cell upward.
pub(crate) const U: u8 = 4;
/// A line leaves this cell downward.
pub(crate) const D: u8 = 8;

/// Walks an orthogonal polyline, handing each cell the bits it gains.
///
/// The one place that decides what a run puts where. The painter and the scorer
/// both go through it, so a drawing and the numbers about that drawing can never
/// disagree about which cells a line touches — which they would, eventually, if
/// each walked the points itself.
///
/// A segment that is neither horizontal nor vertical is skipped: a diagonal is
/// not in the vocabulary, and neither layer should invent one.
pub(crate) fn walk(points: &[(i32, i32)], mut cell: impl FnMut(i32, i32, u8)) {
    for pair in points.windows(2) {
        let [(x0, y0), (x1, y1)] = *pair else {
            continue;
        };
        if y0 == y1 {
            let (lo, hi) = (x0.min(x1), x0.max(x1));
            for x in lo..=hi {
                cell(
                    x,
                    y0,
                    if x < hi { R } else { 0 } | if x > lo { L } else { 0 },
                );
            }
        } else if x0 == x1 {
            let (lo, hi) = (y0.min(y1), y0.max(y1));
            for y in lo..=hi {
                cell(
                    x0,
                    y,
                    if y < hi { D } else { 0 } | if y > lo { U } else { 0 },
                );
            }
        }
    }
}

/// A rectangle of direction bits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Grid {
    cells: Vec<u8>,
    width: usize,
    height: usize,
}

impl Grid {
    /// An empty grid. Anything painted outside it is dropped.
    pub(crate) fn new(width: usize, height: usize) -> Self {
        Self {
            cells: vec![0; width * height],
            width,
            height,
        }
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn height(&self) -> usize {
        self.height
    }

    /// What is at one cell, or nothing if it is off the grid.
    pub(crate) fn bits(&self, x: i32, y: i32) -> u8 {
        self.at(x, y).map_or(0, |at| self.cells[at])
    }

    /// Adds bits to one cell, leaving whatever was there.
    pub(crate) fn add(&mut self, x: i32, y: i32, bits: u8) {
        if let Some(at) = self.at(x, y) {
            self.cells[at] |= bits;
        }
    }

    fn at(&self, x: i32, y: i32) -> Option<usize> {
        let x = usize::try_from(x).ok().filter(|x| *x < self.width)?;
        let y = usize::try_from(y).ok().filter(|y| *y < self.height)?;
        Some(y * self.width + x)
    }

    /// A run of line along one row, endpoints included.
    ///
    /// Every cell but the last leaves to the right and every cell but the first
    /// leaves to the left, so the ends carry only the bit that points inward and
    /// have room for whatever meets them there. A run of one cell sets nothing,
    /// which is right: it goes nowhere.
    pub(crate) fn hline(&mut self, y: i32, from: i32, to: i32) {
        let (lo, hi) = (from.min(to), from.max(to));
        for x in lo..=hi {
            if x < hi {
                self.add(x, y, R);
            }
            if x > lo {
                self.add(x, y, L);
            }
        }
    }

    /// A run of line down one column, endpoints included.
    pub(crate) fn vline(&mut self, x: i32, from: i32, to: i32) {
        let (lo, hi) = (from.min(to), from.max(to));
        for y in lo..=hi {
            if y < hi {
                self.add(x, y, D);
            }
            if y > lo {
                self.add(x, y, U);
            }
        }
    }

    /// Paints a whole polyline.
    pub(crate) fn path(&mut self, points: &[(i32, i32)]) {
        let mut gained = Vec::new();
        walk(points, |x, y, bits| gained.push((x, y, bits)));
        for (x, y, bits) in gained {
            self.add(x, y, bits);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_horizontal_run_points_inward_at_its_ends() {
        let mut grid = Grid::new(5, 1);
        grid.hline(0, 1, 3);
        assert_eq!(grid.bits(0, 0), 0);
        assert_eq!(grid.bits(1, 0), R);
        assert_eq!(grid.bits(2, 0), L | R);
        assert_eq!(grid.bits(3, 0), L);
        assert_eq!(grid.bits(4, 0), 0);
    }

    #[test]
    fn a_vertical_run_points_inward_at_its_ends() {
        let mut grid = Grid::new(1, 4);
        grid.vline(0, 0, 2);
        assert_eq!(grid.bits(0, 0), D);
        assert_eq!(grid.bits(0, 1), U | D);
        assert_eq!(grid.bits(0, 2), U);
        assert_eq!(grid.bits(0, 3), 0);
    }

    #[test]
    fn a_run_of_one_cell_goes_nowhere() {
        let mut grid = Grid::new(3, 3);
        grid.hline(1, 1, 1);
        grid.vline(1, 1, 1);
        assert_eq!(grid.bits(1, 1), 0);
    }

    #[test]
    fn a_run_drawn_backwards_is_the_same_run() {
        let (mut forward, mut backward) = (Grid::new(6, 3), Grid::new(6, 3));
        forward.hline(1, 1, 4);
        backward.hline(1, 4, 1);
        assert_eq!(forward, backward);
    }

    #[test]
    fn runs_that_meet_leave_a_corner_behind() {
        let mut grid = Grid::new(6, 6);
        grid.hline(1, 1, 4);
        grid.vline(4, 1, 5);
        assert_eq!(
            grid.bits(4, 1),
            L | D,
            "the corner is the union, not a decision"
        );
    }

    #[test]
    fn the_order_runs_are_painted_in_does_not_matter() {
        let (mut one, mut other) = (Grid::new(8, 8), Grid::new(8, 8));
        one.hline(3, 0, 7);
        one.vline(4, 0, 7);
        other.vline(4, 0, 7);
        other.hline(3, 0, 7);
        assert_eq!(one, other);
    }

    #[test]
    fn painting_off_the_grid_is_dropped_not_wrapped() {
        let mut grid = Grid::new(3, 3);
        grid.hline(1, -20, 20);
        grid.vline(9, 0, 2);
        grid.add(-1, -1, L | R | U | D);
        assert_eq!(grid.bits(0, 1), L | R);
        assert_eq!(grid.bits(2, 1), L | R);
        assert_eq!(grid.bits(0, 0), 0);
    }

    #[test]
    fn a_walk_and_a_run_agree_on_every_cell() {
        let points = [(1, 1), (6, 1), (6, 5), (2, 5)];
        let (mut walked, mut drawn) = (Grid::new(9, 9), Grid::new(9, 9));
        walk(&points, |x, y, bits| walked.add(x, y, bits));
        drawn.hline(1, 1, 6);
        drawn.vline(6, 1, 5);
        drawn.hline(5, 6, 2);
        assert_eq!(walked, drawn);
    }

    #[test]
    fn a_walk_skips_a_diagonal() {
        let mut touched = 0;
        walk(&[(0, 0), (4, 4)], |_, _, _| touched += 1);
        assert_eq!(touched, 0);
    }

    #[test]
    fn an_empty_grid_holds_nothing() {
        let grid = Grid::new(0, 0);
        assert_eq!(grid.bits(0, 0), 0);
        assert_eq!((grid.width(), grid.height()), (0, 0));
    }
}
