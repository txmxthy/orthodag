//! Putting a number on a drawing.
//!
//! The scorer exists because "looks better" is not a thing a search can descend
//! and not a thing a test can assert. Every judgement the layout makes — which
//! ordering to keep, where to turn — is eventually a comparison between two
//! drawings, and this is what does the comparing.
//!
//! It works on the drawn cells, not on the graph. A crossing a box hides is not
//! a crossing; two runs that merge into one apparent line are one line. Neither
//! fact is visible in the layered graph, and both are visible here.

use crate::graph::EdgeId;
use crate::layout::route::Layout;
use crate::paint::grid::walk;

/// What one edge left in one cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Ink {
    pub(crate) edge: EdgeId,
    pub(crate) bits: u8,
}

/// Every cell of a drawing, and which edges put what there.
///
/// Rasterised through the same walk the painter uses, so the numbers describe
/// the picture that actually gets printed rather than an idea of it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Raster {
    cells: Vec<Vec<Ink>>,
    width: usize,
    height: usize,
}

impl Raster {
    /// Rasterises every route of a drawing.
    pub(crate) fn of(layout: &Layout) -> Self {
        let width = usize::try_from(layout.width).unwrap_or(0);
        let height = usize::try_from(layout.height).unwrap_or(0);
        let mut raster = Self {
            cells: vec![Vec::new(); width * height],
            width,
            height,
        };

        for route in &layout.routes {
            let edge = route.edge;
            let mut gained: Vec<(i32, i32, u8)> = Vec::new();
            walk(&route.points, |x, y, bits| gained.push((x, y, bits)));
            for (x, y, bits) in gained {
                raster.add(x, y, Ink { edge, bits });
            }
        }

        raster
    }

    /// What is in one cell: one entry per edge that touched it.
    ///
    /// An edge that passes through a cell twice — which routing should not
    /// produce and the scorer should not hide — appears once, with the union of
    /// what it left.
    pub(crate) fn at(&self, x: i32, y: i32) -> &[Ink] {
        self.index(x, y).map_or(&[], |at| self.cells[at].as_slice())
    }

    /// Every cell that anything was drawn in.
    pub(crate) fn drawn(&self) -> impl Iterator<Item = (i32, i32, &[Ink])> {
        self.cells
            .iter()
            .enumerate()
            .filter(|(_, ink)| !ink.is_empty())
            .filter_map(move |(at, ink)| {
                let width = self.width.max(1);
                let x = i32::try_from(at % width).ok()?;
                let y = i32::try_from(at / width).ok()?;
                Some((x, y, ink.as_slice()))
            })
    }

    /// How many cells carry an edge glyph.
    pub(crate) fn ink(&self) -> usize {
        self.cells.iter().filter(|ink| !ink.is_empty()).count()
    }

    fn add(&mut self, x: i32, y: i32, ink: Ink) {
        let Some(at) = self.index(x, y) else { return };
        match self.cells[at].iter_mut().find(|held| held.edge == ink.edge) {
            Some(held) => held.bits |= ink.bits,
            None => self.cells[at].push(ink),
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let x = usize::try_from(x).ok().filter(|x| *x < self.width)?;
        let y = usize::try_from(y).ok().filter(|y| *y < self.height)?;
        Some(y * self.width + x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Graph, Node};
    use crate::layout;
    use crate::paint::grid::{D, L, R, U};

    fn drawing(nodes: &[&str], edges: &[(usize, usize)]) -> (Graph, Layout) {
        let mut g = Graph::new();
        let ids: Vec<_> = nodes.iter().map(|n| g.add_node(Node::new(*n))).collect();
        for &(a, b) in edges {
            g.add_edge(ids[a], ids[b]);
        }
        let layout = layout::build(&g);
        (g, layout)
    }

    #[test]
    fn a_straight_edge_leaves_one_run_of_ink() {
        let (_, layout) = drawing(&["a", "b"], &[(0, 1)]);
        let raster = Raster::of(&layout);
        let cells: Vec<_> = raster.drawn().collect();

        assert!(!cells.is_empty());
        for (_, _, ink) in cells {
            assert_eq!(ink.len(), 1, "nothing shares a cell in a two-box drawing");
            assert_eq!(
                ink[0].bits & !(L | R),
                0,
                "a straight edge only goes sideways"
            );
        }
    }

    #[test]
    fn a_fan_shares_the_cells_where_its_branches_meet() {
        let (_, layout) = drawing(&["a", "x", "y", "z"], &[(0, 1), (0, 2), (0, 3)]);
        let raster = Raster::of(&layout);
        let shared = raster.drawn().filter(|(_, _, ink)| ink.len() > 1).count();
        assert!(
            shared > 0,
            "three branches out of one box must share their trunk"
        );
    }

    #[test]
    fn ink_counts_cells_not_edges() {
        let (_, layout) = drawing(&["a", "x", "y", "z"], &[(0, 1), (0, 2), (0, 3)]);
        let raster = Raster::of(&layout);
        let per_edge: usize = raster.drawn().map(|(_, _, ink)| ink.len()).sum();
        assert!(
            raster.ink() < per_edge,
            "a shared cell is one cell, however many edges are in it"
        );
    }

    #[test]
    fn a_cell_holds_one_entry_per_edge_however_often_it_is_touched() {
        let (_, layout) = drawing(&["a", "b", "c", "d"], &[(0, 1), (1, 2), (2, 3), (0, 3)]);
        let raster = Raster::of(&layout);
        for (x, y, ink) in raster.drawn() {
            let mut seen: Vec<EdgeId> = ink.iter().map(|i| i.edge).collect();
            seen.sort_unstable();
            let count = seen.len();
            seen.dedup();
            assert_eq!(seen.len(), count, "{x},{y} lists an edge twice");
        }
    }

    #[test]
    fn a_turn_leaves_both_directions_in_the_corner_cell() {
        let (_, layout) = drawing(&["a", "x", "y"], &[(0, 1), (0, 2)]);
        let raster = Raster::of(&layout);
        let corners = raster
            .drawn()
            .filter(|(_, _, ink)| {
                ink.iter()
                    .any(|i| i.bits & (U | D) != 0 && i.bits & (L | R) != 0)
            })
            .count();
        assert!(corners > 0, "a fan has to turn somewhere");
    }

    #[test]
    fn nothing_is_rastered_outside_the_drawing() {
        let (_, layout) = drawing(&["a", "b", "c"], &[(0, 1), (1, 2)]);
        let raster = Raster::of(&layout);
        for (x, y, _) in raster.drawn() {
            assert!(x >= 0 && x < layout.width && y >= 0 && y < layout.height);
        }
        assert_eq!(raster.at(-1, -1), []);
        assert_eq!(raster.at(layout.width, 0), []);
    }

    #[test]
    fn an_empty_drawing_rasters_to_nothing() {
        let raster = Raster::of(&Layout::default());
        assert_eq!(raster.ink(), 0);
        assert_eq!(raster.drawn().count(), 0);
    }
}
