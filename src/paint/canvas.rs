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
use crate::colour::Colour;

/// The characters a bridge is drawn with, which belong to the flows under them
/// rather than to a box.
const BRIDGE: [char; 3] = ['│', '╴', '╶'];

/// Which way an edge is pointing where it arrives.
///
/// Forward edges all arrive from the left, so this was a constant until back
/// edges came up through a lane and needed to point the other way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Heading {
    Right,
    Up,
}

impl Heading {
    fn glyph(self) -> char {
        match self {
            Self::Right => '▶',
            Self::Up => '▲',
        }
    }

    /// Whether a character is an arrowhead, whichever way it points.
    fn is_head(ch: char) -> bool {
        [Self::Right, Self::Up].iter().any(|h| h.glyph() == ch)
    }
}

/// What a cell is coloured with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ink {
    /// Nothing has been drawn here.
    Blank,
    /// One flow, or an untagged edge, which has no slot.
    One(Option<Colour>),
    /// Two flows of different colours. The cell holds one character and so one
    /// ink; this is the layout's defect to avoid, not the painter's to resolve.
    Mixed,
}

/// A run of cells that share a colour.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Span {
    /// The characters.
    pub text: String,
    /// The palette slot, or `None` where the caller should use its default ink.
    pub colour: Option<Colour>,
}

/// A drawing, one character per cell.
#[derive(Clone, Debug, Default)]
pub(crate) struct Canvas {
    grid: Grid,
    over: Vec<Option<char>>,
    ink: Vec<Ink>,
}

impl Canvas {
    /// A blank canvas. Anything drawn outside it is dropped.
    pub(crate) fn new(width: usize, height: usize) -> Self {
        Self {
            grid: Grid::new(width, height),
            over: vec![None; width * height],
            ink: vec![Ink::Blank; width * height],
        }
    }

    /// Paints an orthogonal polyline as a sequence of runs.
    ///
    /// Order does not matter, here or between calls: runs accumulate bits and
    /// the character is decided at the end, from the bits alone.
    pub(crate) fn path(&mut self, points: &[(i32, i32)], colour: Option<Colour>) {
        self.grid.path(points);
        let mut touched = Vec::new();
        super::grid::walk(points, |x, y, _| touched.push((x, y)));
        for (x, y) in touched {
            self.stain(x, y, colour);
        }
    }

    /// Records what colour a cell is now carrying.
    fn stain(&mut self, x: i32, y: i32, colour: Option<Colour>) {
        let Some(at) = self.at(x, y) else { return };
        self.ink[at] = match self.ink[at] {
            Ink::Blank => Ink::One(colour),
            Ink::One(held) if held == colour => Ink::One(held),
            _ => Ink::Mixed,
        };
    }

    /// Marks where an edge arrives, pointing the way it was going.
    pub(crate) fn head(&mut self, x: i32, y: i32, facing: Heading, colour: Option<Colour>) {
        self.put(x, y, facing.glyph());
        self.stain(x, y, colour);
    }

    /// Puts one character over whatever is underneath.
    pub(crate) fn put(&mut self, x: i32, y: i32, ch: char) {
        if let Some(at) = self.at(x, y) {
            self.over[at] = Some(ch);
        }
    }

    /// Puts a run of text that belongs to a flow rather than to a box.
    pub(crate) fn write_over(&mut self, x: i32, y: i32, text: &str, colour: Option<Colour>) {
        self.write(x, y, text);
        for step in 0..i32::try_from(text.chars().count()).unwrap_or(0) {
            self.stain(x + step, y, colour);
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

    /// Redraws crossings so the vertical reads as passing over the horizontal.
    ///
    /// The cell itself becomes plain vertical, and the horizontal is cut one
    /// cell either side — but only where that neighbour is plain line. A corner
    /// or a junction beside a crossing is doing its own job and stays.
    pub(crate) fn bridge(&mut self, cells: &[crate::score::Crossed]) {
        for cell in cells {
            let (x, y) = (cell.x, cell.y);
            if self.at(x, y).is_none() {
                continue;
            }
            self.over_line(x, y, '│');
            self.over_line(x - 1, y, '╴');
            self.over_line(x + 1, y, '╶');
            // The horizontal is gone from this cell, so the two inks that met
            // here are one again and it is the vertical's. Left as it was, the
            // cell would keep the "two colours" it was stained with and come out
            // in the caller's default — a grey notch in a line that was bridged
            // precisely so it could be followed through.
            self.relight(x, y, cell.down);
        }
    }

    /// Sets what a cell carries, rather than adding to it.
    fn relight(&mut self, x: i32, y: i32, colour: Option<Colour>) {
        if let Some(at) = self.at(x, y)
            && self.ink[at] != Ink::Blank
        {
            self.ink[at] = Ink::One(colour);
        }
    }

    /// Overrides a cell, but only if it is currently plain line.
    fn over_line(&mut self, x: i32, y: i32, ch: char) {
        let Some(at) = self.at(x, y) else { return };
        if self.over[at].is_some() {
            return;
        }
        let drawn = glyph(self.grid.bits(x, y));
        if ch == '│' || drawn == '─' {
            self.over[at] = Some(ch);
        }
    }

    /// What one cell reads as: the overlay if there is one, else the bits.
    fn cell(&self, x: i32, y: i32) -> char {
        self.at(x, y)
            .and_then(|at| self.over[at])
            .unwrap_or_else(|| glyph(self.grid.bits(x, y)))
    }

    /// What colour one cell is drawn in.
    ///
    /// A border or a label has none: they belong to a box, not a flow. So does
    /// a cell two colours met in — the caller gets `None` and paints its
    /// default, which is the honest thing to show for a cell that cannot say
    /// which flow it belongs to.
    fn colour_at(&self, x: i32, y: i32) -> Option<Colour> {
        let at = self.at(x, y)?;
        // A border or a label on a box has no flow; an arrowhead and a caption
        // on an edge do, and were stained when they were written.
        if self.over[at].is_some_and(|ch| !Heading::is_head(ch) && !BRIDGE.contains(&ch))
            && self.ink[at] == Ink::Blank
        {
            return None;
        }
        match self.ink[at] {
            Ink::One(colour) => colour,
            Ink::Blank | Ink::Mixed => None,
        }
    }

    /// The drawing as styled runs, one list per row.
    ///
    /// Adjacent cells of the same colour are one span, so a caller writes one
    /// escape code — or one element, or one widget style — per run rather than
    /// per cell. The library still never says what a colour looks like.
    pub(crate) fn runs(&self) -> Vec<Vec<Span>> {
        (0..self.grid.height())
            .filter_map(|y| i32::try_from(y).ok())
            .map(|y| {
                let mut spans: Vec<Span> = Vec::new();
                for x in (0..self.grid.width()).filter_map(|x| i32::try_from(x).ok()) {
                    let (ch, colour) = (self.cell(x, y), self.colour_at(x, y));
                    match spans.last_mut() {
                        Some(span) if span.colour == colour => span.text.push(ch),
                        _ => spans.push(Span {
                            text: ch.to_string(),
                            colour,
                        }),
                    }
                }
                if let Some(last) = spans.last_mut() {
                    last.text.truncate(last.text.trim_end().len());
                    if last.text.is_empty() {
                        spans.pop();
                    }
                }
                spans
            })
            .collect()
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
        canvas.path(&[(0, 0), (5, 0)], None);
        assert_eq!(drawn(&canvas), ["──────"]);
    }

    #[test]
    fn a_turn_leaves_a_corner_nobody_asked_for() {
        let mut canvas = Canvas::new(4, 3);
        canvas.path(&[(0, 0), (3, 0), (3, 2)], None);
        assert_eq!(drawn(&canvas), ["───┐", "   │", "   │"]);
    }

    #[test]
    fn two_paths_crossing_leave_a_cross() {
        let mut canvas = Canvas::new(3, 3);
        canvas.path(&[(0, 1), (2, 1)], None);
        canvas.path(&[(1, 0), (1, 2)], None);
        assert_eq!(drawn(&canvas), [" │", "─┼─", " │"]);
    }

    #[test]
    fn a_fork_and_a_crossing_draw_the_same_because_they_read_the_same() {
        let (mut fork, mut crossing) = (Canvas::new(3, 3), Canvas::new(3, 3));
        fork.path(&[(0, 1), (1, 1), (1, 0)], None);
        fork.path(&[(0, 1), (1, 1), (1, 2)], None);
        fork.path(&[(0, 1), (2, 1)], None);
        crossing.path(&[(0, 1), (2, 1)], None);
        crossing.path(&[(1, 0), (1, 2)], None);
        assert_eq!(drawn(&fork), drawn(&crossing));
    }

    #[test]
    fn the_order_paths_are_drawn_in_does_not_matter() {
        let (mut one, mut other) = (Canvas::new(9, 5), Canvas::new(9, 5));
        let (a, b) = ([(0, 0), (4, 0), (4, 4)], [(0, 2), (8, 2)]);
        one.path(&a, None);
        one.path(&b, None);
        other.path(&b, None);
        other.path(&a, None);
        assert_eq!(drawn(&one), drawn(&other));
    }

    #[test]
    fn a_border_does_not_fuse_with_a_line_that_touches_it() {
        let mut canvas = Canvas::new(8, 3);
        canvas.rect(0, 0, 5, 3);
        canvas.path(&[(5, 1), (7, 1)], None);
        assert_eq!(drawn(&canvas), ["┌───┐", "│   │───", "└───┘"]);
    }

    #[test]
    fn text_sits_over_whatever_is_under_it() {
        let mut canvas = Canvas::new(9, 1);
        canvas.path(&[(0, 0), (8, 0)], None);
        canvas.write(2, 0, "tag");
        assert_eq!(drawn(&canvas), ["──tag────"]);
    }

    #[test]
    fn an_arrowhead_marks_where_an_edge_arrives() {
        let mut canvas = Canvas::new(5, 1);
        canvas.path(&[(0, 0), (4, 0)], None);
        canvas.head(4, 0, Heading::Right, None);
        assert_eq!(drawn(&canvas), ["────▶"]);
    }

    #[test]
    fn trailing_blanks_are_cut() {
        let mut canvas = Canvas::new(10, 2);
        canvas.path(&[(0, 0), (2, 0)], None);
        assert_eq!(drawn(&canvas), ["───", ""]);
    }

    #[test]
    fn drawing_outside_the_canvas_is_dropped() {
        let mut canvas = Canvas::new(3, 1);
        canvas.path(&[(-9, 0), (9, 0)], None);
        canvas.write(-2, 0, "off");
        canvas.rect(20, 20, 4, 4);
        assert_eq!(drawn(&canvas), ["f──"]);
    }

    #[test]
    fn runs_join_up_the_cells_that_share_a_colour() {
        let mut canvas = Canvas::new(9, 1);
        canvas.path(&[(0, 0), (8, 0)], None);
        let rows = canvas.runs();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].len(), 1, "one colour, one span");
        assert_eq!(rows[0][0].text, "─────────");
    }

    #[test]
    fn a_run_breaks_where_the_colour_does() {
        let (one, other) = (Colour::from_slot(0), Colour::from_slot(1));
        let mut canvas = Canvas::new(9, 3);
        canvas.path(&[(0, 0), (3, 0)], Some(one));
        canvas.path(&[(5, 0), (8, 0)], Some(other));
        let colours: Vec<_> = canvas.runs()[0].iter().map(|s| s.colour).collect();
        assert_eq!(colours, [Some(one), None, Some(other)]);
    }

    #[test]
    fn a_cell_two_colours_met_in_belongs_to_neither() {
        let (one, other) = (Colour::from_slot(0), Colour::from_slot(1));
        let mut canvas = Canvas::new(3, 3);
        canvas.path(&[(0, 1), (2, 1)], Some(one));
        canvas.path(&[(1, 0), (1, 2)], Some(other));
        let middle = &canvas.runs()[1];
        assert_eq!(
            middle.iter().map(|s| s.colour).collect::<Vec<_>>(),
            [Some(one), None, Some(one)]
        );
    }

    #[test]
    fn a_border_carries_no_flow_colour() {
        let mut canvas = Canvas::new(6, 3);
        canvas.rect(0, 0, 5, 3);
        canvas.write(1, 1, "ab");
        assert!(canvas.runs()[0].iter().all(|s| s.colour.is_none()));
    }

    #[test]
    fn runs_say_the_same_thing_the_string_does() {
        let (one, other) = (Colour::from_slot(2), Colour::from_slot(5));
        let mut canvas = Canvas::new(12, 4);
        canvas.path(&[(0, 0), (6, 0), (6, 3)], Some(one));
        canvas.path(&[(0, 2), (11, 2)], Some(other));
        canvas.rect(7, 0, 4, 2);
        canvas.head(11, 2, Heading::Right, Some(other));

        let joined: Vec<String> = canvas
            .runs()
            .iter()
            .map(|row| row.iter().map(|s| s.text.as_str()).collect())
            .collect();
        assert_eq!(joined, canvas.to_string().lines().collect::<Vec<_>>());
    }

    #[test]
    fn a_bridge_cuts_the_horizontal_so_the_vertical_passes_over() {
        let mut canvas = Canvas::new(5, 3);
        canvas.path(&[(0, 1), (4, 1)], None);
        canvas.path(&[(2, 0), (2, 2)], None);
        assert_eq!(drawn(&canvas)[1], "──┼──");
        canvas.bridge(&[crossed(2, 1, None)]);
        assert_eq!(drawn(&canvas)[1], "─╴│╶─");
    }

    /// A bridged cell carries the line that survived it.
    ///
    /// Two colours met here, so the cell was stained as holding neither. Cutting
    /// the horizontal leaves the vertical alone in it, and the cell has to be
    /// told: otherwise the one place a reader most needs to follow a line
    /// through comes out in the default ink, a grey notch in the middle of a
    /// coloured run.
    #[test]
    fn a_bridged_cell_keeps_the_colour_of_the_line_that_crosses_it() {
        let (across, down) = (Colour::from_slot(0), Colour::from_slot(1));
        let mut canvas = Canvas::new(5, 3);
        canvas.path(&[(0, 1), (4, 1)], Some(across));
        canvas.path(&[(2, 0), (2, 2)], Some(down));
        assert_eq!(canvas.colour_at(2, 1), None, "two inks, so neither");

        canvas.bridge(&[crossed(2, 1, Some(down))]);
        assert_eq!(canvas.colour_at(2, 1), Some(down));
        assert_eq!(
            canvas.colour_at(1, 1),
            Some(across),
            "the cut cell is still its own"
        );
    }

    fn crossed(x: i32, y: i32, down: Option<Colour>) -> crate::score::Crossed {
        crate::score::Crossed { x, y, down }
    }

    #[test]
    fn a_bridge_leaves_a_corner_beside_it_alone() {
        // The cell right of the crossing is where another line turns. Cutting
        // it would erase a bend, so it stays.
        let mut canvas = Canvas::new(6, 4);
        canvas.path(&[(0, 1), (5, 1)], None);
        canvas.path(&[(2, 0), (2, 3)], None);
        canvas.path(&[(3, 1), (3, 3)], None);
        canvas.bridge(&[crossed(2, 1, None)]);
        let row = &drawn(&canvas)[1];
        assert!(row.starts_with("─╴│"), "{row}");
        assert!(row.contains('┬'), "the neighbour keeps its junction: {row}");
    }

    #[test]
    fn a_bridge_over_nothing_changes_nothing() {
        let mut canvas = Canvas::new(4, 1);
        canvas.path(&[(0, 0), (3, 0)], None);
        let before = drawn(&canvas);
        canvas.bridge(&[crossed(9, 9, None)]);
        assert_eq!(drawn(&canvas), before);
    }

    #[test]
    fn a_diagonal_is_not_drawn_at_all() {
        let mut canvas = Canvas::new(4, 4);
        canvas.path(&[(0, 0), (3, 3)], None);
        assert_eq!(canvas.to_string().trim(), "");
    }
}
