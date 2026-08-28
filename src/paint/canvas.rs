//! A rectangle of characters, built from runs of line and text laid over them.
//!
//! Two layers. Underneath, the direction-bit grid, where every route is painted
//! and corners appear on their own. Over it, characters put down verbatim —
//! box borders, labels, arrowheads — which win wherever they are set.
//!
//! The layers exist because a box is not a line. A border that shared the grid
//! would fuse with any edge that touched it and turn into a junction, and the
//! reader would see a line entering a box through its side.

use std::fmt;

use super::glyph::glyph;
use super::grid::Grid;

/// The character an edge ends on.
const HEAD: char = '▶';

/// A drawing, one character per cell.
#[derive(Clone, Debug, Default)]
pub(crate) struct Canvas {
    grid: Grid,
    over: Vec<Option<char>>,
}

impl Canvas {
    /// A blank canvas. Anything drawn outside it is dropped.
    pub(crate) fn new(width: usize, height: usize) -> Self {
        Self {
            grid: Grid::new(width, height),
            over: vec![None; width * height],
        }
    }

    /// Paints an orthogonal polyline as a sequence of runs.
    ///
    /// Order does not matter, here or between calls: runs accumulate bits and
    /// the character is decided at the end, from the bits alone.
    pub(crate) fn path(&mut self, points: &[(i32, i32)]) {
        self.grid.path(points);
    }

    /// Marks where an edge arrives.
    pub(crate) fn head(&mut self, x: i32, y: i32) {
        self.put(x, y, HEAD);
    }

    /// Puts one character over whatever is underneath.
    pub(crate) fn put(&mut self, x: i32, y: i32, ch: char) {
        if let Some(at) = self.at(x, y) {
            self.over[at] = Some(ch);
        }
    }

    /// Puts a run of text, one character per cell, starting at `x`.
    pub(crate) fn write(&mut self, x: i32, y: i32, text: &str) {
        for (step, ch) in text.chars().enumerate() {
            let Ok(step) = i32::try_from(step) else {
                return;
            };
            self.put(x + step, y, ch);
        }
    }

    /// Draws a box border. `w` and `h` count the border cells.
    pub(crate) fn rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        if w < 2 || h < 2 {
            return;
        }
        let (right, bottom) = (x + w - 1, y + h - 1);
        for step in x + 1..right {
            self.put(step, y, '─');
            self.put(step, bottom, '─');
        }
        for step in y + 1..bottom {
            self.put(x, step, '│');
            self.put(right, step, '│');
        }
        self.put(x, y, '┌');
        self.put(right, y, '┐');
        self.put(x, bottom, '└');
        self.put(right, bottom, '┘');
    }

    fn at(&self, x: i32, y: i32) -> Option<usize> {
        let x = usize::try_from(x).ok().filter(|x| *x < self.grid.width())?;
        let y = usize::try_from(y)
            .ok()
            .filter(|y| *y < self.grid.height())?;
        Some(y * self.grid.width() + x)
    }

    /// What one cell reads as: the overlay if there is one, else the bits.
    fn cell(&self, x: i32, y: i32) -> char {
        self.at(x, y)
            .and_then(|at| self.over[at])
            .unwrap_or_else(|| glyph(self.grid.bits(x, y)))
    }
}

impl fmt::Display for Canvas {
    /// One line per row, with trailing blanks cut.
    ///
    /// Trailing blanks are invisible and would otherwise be the difference
    /// between two snapshots that look identical.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for y in 0..self.grid.height() {
            let mut line = String::with_capacity(self.grid.width());
            for x in 0..self.grid.width() {
                let (Ok(x), Ok(y)) = (i32::try_from(x), i32::try_from(y)) else {
                    continue;
                };
                line.push(self.cell(x, y));
            }
            writeln!(f, "{}", line.trim_end())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drawn(canvas: &Canvas) -> Vec<String> {
        canvas.to_string().lines().map(str::to_owned).collect()
    }

    #[test]
    fn a_straight_run_draws_as_line() {
        let mut canvas = Canvas::new(6, 1);
        canvas.path(&[(0, 0), (5, 0)]);
        assert_eq!(drawn(&canvas), ["──────"]);
    }

    #[test]
    fn a_turn_leaves_a_corner_nobody_asked_for() {
        let mut canvas = Canvas::new(4, 3);
        canvas.path(&[(0, 0), (3, 0), (3, 2)]);
        assert_eq!(drawn(&canvas), ["───┐", "   │", "   │"]);
    }

    #[test]
    fn two_paths_crossing_leave_a_cross() {
        let mut canvas = Canvas::new(3, 3);
        canvas.path(&[(0, 1), (2, 1)]);
        canvas.path(&[(1, 0), (1, 2)]);
        assert_eq!(drawn(&canvas), [" │", "─┼─", " │"]);
    }

    #[test]
    fn a_fork_and_a_crossing_draw_the_same_because_they_read_the_same() {
        let (mut fork, mut crossing) = (Canvas::new(3, 3), Canvas::new(3, 3));
        fork.path(&[(0, 1), (1, 1), (1, 0)]);
        fork.path(&[(0, 1), (1, 1), (1, 2)]);
        fork.path(&[(0, 1), (2, 1)]);
        crossing.path(&[(0, 1), (2, 1)]);
        crossing.path(&[(1, 0), (1, 2)]);
        assert_eq!(drawn(&fork), drawn(&crossing));
    }

    #[test]
    fn the_order_paths_are_drawn_in_does_not_matter() {
        let (mut one, mut other) = (Canvas::new(9, 5), Canvas::new(9, 5));
        let (a, b) = ([(0, 0), (4, 0), (4, 4)], [(0, 2), (8, 2)]);
        one.path(&a);
        one.path(&b);
        other.path(&b);
        other.path(&a);
        assert_eq!(drawn(&one), drawn(&other));
    }

    #[test]
    fn a_border_does_not_fuse_with_a_line_that_touches_it() {
        let mut canvas = Canvas::new(8, 3);
        canvas.rect(0, 0, 5, 3);
        canvas.path(&[(5, 1), (7, 1)]);
        assert_eq!(drawn(&canvas), ["┌───┐", "│   │───", "└───┘"]);
    }

    #[test]
    fn text_sits_over_whatever_is_under_it() {
        let mut canvas = Canvas::new(9, 1);
        canvas.path(&[(0, 0), (8, 0)]);
        canvas.write(2, 0, "tag");
        assert_eq!(drawn(&canvas), ["──tag────"]);
    }

    #[test]
    fn an_arrowhead_marks_where_an_edge_arrives() {
        let mut canvas = Canvas::new(5, 1);
        canvas.path(&[(0, 0), (4, 0)]);
        canvas.head(4, 0);
        assert_eq!(drawn(&canvas), ["────▶"]);
    }

    #[test]
    fn trailing_blanks_are_cut() {
        let mut canvas = Canvas::new(10, 2);
        canvas.path(&[(0, 0), (2, 0)]);
        assert_eq!(drawn(&canvas), ["───", ""]);
    }

    #[test]
    fn drawing_outside_the_canvas_is_dropped() {
        let mut canvas = Canvas::new(3, 1);
        canvas.path(&[(-9, 0), (9, 0)]);
        canvas.write(-2, 0, "off");
        canvas.rect(20, 20, 4, 4);
        assert_eq!(drawn(&canvas), ["f──"]);
    }

    #[test]
    fn a_diagonal_is_not_drawn_at_all() {
        let mut canvas = Canvas::new(4, 4);
        canvas.path(&[(0, 0), (3, 3)]);
        assert_eq!(canvas.to_string().trim(), "");
    }
}
