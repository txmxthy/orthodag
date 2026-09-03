//! Which palette slot an edge gets.
//!
//! The library does not know what a colour is. It hands back a slot number and
//! the caller decides what that looks like — an escape code, a CSS class, a
//! widget style. That is the whole reason the painter has no dependency on a
//! terminal library.
//!
//! Slots are keyed on the **tag set**, not on the edge. Two edges carrying the
//! same tags are one logical flow as far as a reader is concerned, and a flow
//! that changes colour halfway across a drawing is worse than no colour at all.

use crate::graph::Graph;

/// How many slots there are before they start repeating.
///
/// Six is what a terminal can be relied on for without asking, and a drawing
/// that needs more than six distinguishable flows has a bigger problem than the
/// palette.
pub const PALETTE: u8 = 6;

/// A slot in the caller's palette.
///
/// Always less than [`PALETTE`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Colour(u8);

impl Colour {
    /// The slot number, for indexing whatever the caller calls a palette.
    pub fn slot(self) -> u8 {
        self.0
    }

    /// A slot by number, wrapped into the palette. For tests.
    #[cfg(test)]
    pub(crate) fn from_slot(slot: u8) -> Self {
        Self(slot % PALETTE)
    }
}

/// The palette slot of every edge, or `None` where it carries no tags.
///
/// Slots go out in the order the tag sets are first seen, so a drawing's colours
/// follow the order the graph was built in rather than anything the layout did.
/// Two edges with the same tags always get the same slot; two edges with
/// different tags may share one once the palette wraps, which is the price of
/// six.
pub fn of(g: &Graph) -> Vec<Option<Colour>> {
    let mut seen: Vec<&[String]> = Vec::new();
    g.edge_ids()
        .map(|id| {
            let tags = g.edge(id)?.tags();
            if tags.is_empty() {
                return None;
            }
            let at = seen
                .iter()
                .position(|held| *held == tags)
                .unwrap_or_else(|| {
                    seen.push(tags);
                    seen.len() - 1
                });
            Some(Colour(u8::try_from(at % usize::from(PALETTE)).unwrap_or(0)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Node;

    fn graph(tags: &[&[&str]]) -> Graph {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        for set in tags {
            g.add_tagged_edge(a, b, set.iter().copied());
        }
        g
    }

    #[test]
    fn an_untagged_edge_has_no_colour() {
        let mut g = Graph::new();
        let a = g.add_node(Node::new("a"));
        let b = g.add_node(Node::new("b"));
        g.add_edge(a, b);
        assert_eq!(of(&g), [None]);
    }

    #[test]
    fn the_same_tags_always_get_the_same_slot() {
        let g = graph(&[&["even"], &["odd"], &["even"]]);
        let slots = of(&g);
        assert_eq!(slots[0], slots[2]);
        assert_ne!(slots[0], slots[1]);
    }

    #[test]
    fn the_order_tags_were_written_in_does_not_matter() {
        let g = graph(&[&["late", "odd"], &["odd", "late"]]);
        let slots = of(&g);
        assert_eq!(
            slots[0], slots[1],
            "the tag set is canonical, so these are one flow"
        );
    }

    #[test]
    fn slots_go_out_in_the_order_the_sets_are_first_seen() {
        let g = graph(&[&["a"], &["b"], &["c"]]);
        let slots: Vec<_> = of(&g).into_iter().flatten().map(Colour::slot).collect();
        assert_eq!(slots, [0, 1, 2]);
    }

    #[test]
    fn the_palette_wraps_rather_than_running_out() {
        let sets: Vec<Vec<&str>> = (0..8)
            .map(|i| vec![["a", "b", "c", "d", "e", "f", "g", "h"][i]])
            .collect();
        let borrowed: Vec<&[&str]> = sets.iter().map(Vec::as_slice).collect();
        let g = graph(&borrowed);
        let slots: Vec<_> = of(&g).into_iter().flatten().map(Colour::slot).collect();
        assert_eq!(slots, [0, 1, 2, 3, 4, 5, 0, 1]);
        assert!(slots.iter().all(|s| *s < PALETTE));
    }
}
