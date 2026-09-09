//! Deciding the order of each column.
//!
//! Crossings between two neighbouring columns depend only on the order of those
//! two columns, so ordering is a sequence of small independent problems rather
//! than one large one — which is the whole reason long edges were cut into
//! single hops first.
//!
//! The classical move is a barycenter sweep: sort a column by the mean position
//! of each slot's neighbours in the column beside it, and alternate direction
//! until it settles. It is a heuristic and it does settle, usually on something
//! good. Sorting by the median instead is the other classical move, and neither
//! beats the other on every graph, so both sweep and both propose.
//!
//! Either reading is kept as an exact rational rather than a float. Two slots
//! that land on the same spot must compare equal every run, on every machine,
//! or the drawing changes between runs for no reason a reader could see.

use std::cmp::Ordering;

use super::layer::{Layered, Segment, Slot};

/// The order of every column, top to bottom.
pub(crate) type Columns = Vec<Vec<Slot>>;

/// Which reading of "where a slot's neighbours are" a sweep sorts by.
///
/// Barycenter is the mean and median is the middle, and neither dominates: the
/// mean is dragged by one neighbour at the far end of a column, and the median
/// ignores how far away everything is. The literature picks one and argues. Here
/// both sweep, both propose, and the drawing decides — which is what the
/// candidate set is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Read {
    /// The mean position. One distant neighbour moves the slot.
    Mean,
    /// The middle position, or the mean of the middle two. Outliers do not.
    Middle,
}

/// Where a slot's neighbours put it, held exactly.
///
/// A slot with no neighbours in the column being read from takes its own
/// position, so it stays where it is instead of collapsing to the top.
///
/// Both readings are rationals — the median of an even number of neighbours is
/// the mean of the middle two — so one comparison serves both, and it is exact.
/// Two slots that land on the same spot must compare equal every run, on every
/// machine, or the drawing changes for no reason a reader could see.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Bary {
    total: u64,
    count: u64,
}

impl Bary {
    fn of(read: Read, positions: &[usize], fallback: usize) -> Self {
        if positions.is_empty() {
            return Self {
                total: fallback as u64,
                count: 1,
            };
        }
        match read {
            Read::Mean => Self {
                total: positions.iter().map(|p| *p as u64).sum(),
                count: positions.len() as u64,
            },
            Read::Middle => {
                let mut sorted: Vec<u64> = positions.iter().map(|p| *p as u64).collect();
                sorted.sort_unstable();
                let at = sorted.len() / 2;
                if sorted.len() % 2 == 1 {
                    Self {
                        total: sorted[at],
                        count: 1,
                    }
                } else {
                    Self {
                        total: sorted[at - 1] + sorted[at],
                        count: 2,
                    }
                }
            }
        }
    }
}

impl Ord for Bary {
    /// `a/b` against `c/d` by cross-multiplication, in a width that cannot wrap.
    fn cmp(&self, other: &Self) -> Ordering {
        let left = u128::from(self.total) * u128::from(other.count);
        let right = u128::from(other.total) * u128::from(self.count);
        left.cmp(&right)
    }
}

impl PartialOrd for Bary {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The hops of each gap, gathered once so a sweep does not rescan every edge.
pub(crate) struct Hops<'a> {
    per_gap: Vec<Vec<&'a Segment>>,
}

impl<'a> Hops<'a> {
    pub(crate) fn of(l: &'a Layered) -> Self {
        let mut per_gap = vec![Vec::new(); l.columns().saturating_sub(1)];
        for segment in l.segments() {
            if let Some(gap) = per_gap.get_mut(segment.gap) {
                gap.push(segment);
            }
        }
        Self { per_gap }
    }

    pub(crate) fn gap(&self, index: usize) -> &[&'a Segment] {
        self.per_gap.get(index).map_or(&[], Vec::as_slice)
    }
}

/// Where a slot sits in a column.
fn position(columns: &Columns, column: usize, slot: Slot) -> Option<usize> {
    columns.get(column)?.iter().position(|s| *s == slot)
}

/// Sorts one column by the mean position of its neighbours in the one before it.
///
/// Ties keep the order they had: the sort is stable, so a sweep never reshuffles
/// slots it has no opinion about.
fn sweep_down(columns: &mut Columns, hops: &Hops, read: Read, column: usize) {
    let Some(previous) = column.checked_sub(1) else {
        return;
    };
    let keys: Vec<_> = columns
        .get(column)
        .map(|slots| {
            slots
                .iter()
                .enumerate()
                .map(|(at, slot)| {
                    let neighbours: Vec<_> = hops
                        .gap(previous)
                        .iter()
                        .filter(|s| s.to == *slot)
                        .filter_map(|s| position(columns, previous, s.from))
                        .collect();
                    (*slot, Bary::of(read, &neighbours, at))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    apply(columns, column, keys);
}

/// Sorts one column by the mean position of its neighbours in the one after it.
fn sweep_up(columns: &mut Columns, hops: &Hops, read: Read, column: usize) {
    let keys: Vec<_> = columns
        .get(column)
        .map(|slots| {
            slots
                .iter()
                .enumerate()
                .map(|(at, slot)| {
                    let neighbours: Vec<_> = hops
                        .gap(column)
                        .iter()
                        .filter(|s| s.from == *slot)
                        .filter_map(|s| position(columns, column + 1, s.to))
                        .collect();
                    (*slot, Bary::of(read, &neighbours, at))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    apply(columns, column, keys);
}

fn apply(columns: &mut Columns, column: usize, mut keys: Vec<(Slot, Bary)>) {
    keys.sort_by_key(|(_, bary)| *bary);
    if let Some(slots) = columns.get_mut(column) {
        *slots = keys.into_iter().map(|(slot, _)| slot).collect();
    }
}

/// How many sweeps to run before giving up on finding anything new.
///
/// The sweeps either settle or cycle, and both happen quickly. This is the
/// budget on a stage that scales with the graph, and it is small because the
/// expensive half of ordering is not here — it is drawing each candidate.
const MAX_SWEEPS: usize = 8;

/// Every distinct column order the sweeps propose, the initial one first.
///
/// A sweep is a **proposal, not a decision**. Both readings are heuristics over
/// the layered graph, and the layered graph is not what a reader sees: it cannot
/// tell that a box hides a crossing, or that two runs merge into one apparent
/// line. So every distinct order either of them passes through is kept, and the
/// choice between them is made later, by drawing them and scoring the drawings.
///
/// Each reading starts again from the initial order rather than from where the
/// other one stopped, so neither inherits the other's local minimum. Sweeping
/// stops as soon as it repeats itself, which is what settling looks like from
/// here, and an order one reading already proposed is not proposed twice.
pub(crate) fn orderings(l: &Layered) -> Vec<Columns> {
    let hops = Hops::of(l);
    let initial = l.all().to_vec();
    let mut candidates = vec![initial.clone()];

    for read in [Read::Mean, Read::Middle] {
        let mut current = initial.clone();
        for _ in 0..MAX_SWEEPS {
            sweep(&mut current, &hops, read);
            if candidates.contains(&current) {
                break;
            }
            candidates.push(current.clone());
        }
    }

    candidates
}

/// One pass over every column, left to right then right to left.
pub(crate) fn sweep(columns: &mut Columns, hops: &Hops, read: Read) {
    for column in 1..columns.len() {
        sweep_down(columns, hops, read, column);
    }
    for column in (0..columns.len().saturating_sub(1)).rev() {
        sweep_up(columns, hops, read, column);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Adjacency, acyclic::back_edges, layer::layer, rank::rank};
    use super::*;
    use crate::graph::{Graph, Node, NodeId};

    fn build(nodes: usize, edges: &[(usize, usize)]) -> (Vec<NodeId>, Layered) {
        let mut g = Graph::new();
        let ids: Vec<_> = (0..nodes)
            .map(|i| g.add_node(Node::new(format!("n{i}"))))
            .collect();
        for &(a, b) in edges {
            g.add_edge(ids[a], ids[b]);
        }
        let adj = Adjacency::of(&g);
        let acyclic = back_edges(&g, &adj);
        let ranked = rank(&g, &adj, &acyclic);
        (ids, layer(&g, &adj, &acyclic, &ranked))
    }

    fn swept(l: &Layered) -> Columns {
        let hops = Hops::of(l);
        let mut columns = l.all().to_vec();
        sweep(&mut columns, &hops, Read::Mean);
        columns
    }

    #[test]
    fn a_rational_barycenter_orders_without_rounding() {
        // 1/3 < 2/5 < 1/2, which is the kind of gap a float would keep too, and
        // 2/6 == 1/3, which is the kind it would not.
        let third = Bary { total: 1, count: 3 };
        let two_fifths = Bary { total: 2, count: 5 };
        let half = Bary { total: 1, count: 2 };
        assert!(third < two_fifths && two_fifths < half);
        assert_eq!(third.cmp(&Bary { total: 2, count: 6 }), Ordering::Equal);
    }

    /// The case the two readings disagree on, which is the reason to keep both.
    ///
    /// Four neighbours at 0, 1, 2 and 9: the mean is 3 and the middle is 1½, so
    /// one slot follows the outlier and the other does not.
    #[test]
    fn the_middle_ignores_an_outlier_the_mean_follows() {
        let at = [0, 1, 2, 9];
        let mean = Bary::of(Read::Mean, &at, 0);
        let middle = Bary::of(Read::Middle, &at, 0);

        assert_eq!(mean.cmp(&Bary { total: 3, count: 1 }), Ordering::Equal);
        assert_eq!(middle.cmp(&Bary { total: 3, count: 2 }), Ordering::Equal);
        assert!(middle < mean);
    }

    /// An odd count has a middle of its own, and it is one of the positions.
    #[test]
    fn the_middle_of_an_odd_run_is_a_position() {
        assert_eq!(
            Bary::of(Read::Middle, &[0, 4, 100], 0).cmp(&Bary { total: 4, count: 1 }),
            Ordering::Equal
        );
    }

    /// Neither reading has an opinion about a slot with no neighbours.
    #[test]
    fn neither_reading_moves_a_slot_with_nothing_to_read() {
        for read in [Read::Mean, Read::Middle] {
            assert_eq!(
                Bary::of(read, &[], 7).cmp(&Bary { total: 7, count: 1 }),
                Ordering::Equal
            );
        }
    }

    #[test]
    fn a_barycenter_that_would_overflow_a_narrower_width_still_compares() {
        let big = Bary {
            total: u64::MAX,
            count: u64::MAX,
        };
        let same = Bary { total: 1, count: 1 };
        assert_eq!(big.cmp(&same), Ordering::Equal);
    }

    #[test]
    fn a_slot_with_no_neighbours_stays_where_it_is() {
        // 0 -> 2 only; node 1 is loose and must not float to the top.
        let (ids, l) = build(3, &[(0, 1)]);
        let mut columns = vec![
            vec![Slot::Node(ids[2]), Slot::Node(ids[0])],
            vec![Slot::Node(ids[1])],
        ];
        sweep(&mut columns, &Hops::of(&l), Read::Mean);
        assert_eq!(columns[0], [Slot::Node(ids[2]), Slot::Node(ids[0])]);
    }

    #[test]
    fn a_sweep_pulls_a_crossing_apart() {
        // 0 -> 3 and 1 -> 2, with the second column entered in the wrong order.
        let (ids, l) = build(4, &[(0, 3), (1, 2)]);
        let mut columns = vec![
            vec![Slot::Node(ids[0]), Slot::Node(ids[1])],
            vec![Slot::Node(ids[2]), Slot::Node(ids[3])],
        ];
        sweep(&mut columns, &Hops::of(&l), Read::Mean);
        assert_eq!(columns[1], [Slot::Node(ids[3]), Slot::Node(ids[2])]);
    }

    #[test]
    fn a_fan_out_keeps_the_order_it_was_given() {
        let (ids, l) = build(4, &[(0, 1), (0, 2), (0, 3)]);
        assert_eq!(
            swept(&l)[1],
            [Slot::Node(ids[1]), Slot::Node(ids[2]), Slot::Node(ids[3])]
        );
    }

    #[test]
    fn the_initial_order_is_always_one_of_the_candidates() {
        let (_, l) = build(4, &[(0, 3), (1, 2)]);
        assert_eq!(orderings(&l).first(), Some(&l.all().to_vec()));
    }

    #[test]
    fn a_settled_graph_proposes_nothing_further() {
        // A chain is already in the only order it has.
        let (_, l) = build(3, &[(0, 1), (1, 2)]);
        assert_eq!(orderings(&l).len(), 1);
    }

    #[test]
    fn a_crossing_produces_a_second_candidate() {
        let (ids, l) = build(4, &[(0, 3), (1, 2)]);
        let candidates = orderings(&l);
        assert!(candidates.len() > 1, "the sweep found nothing to propose");
        let last = candidates.last().expect("candidates are never empty");
        assert_eq!(last[1], [Slot::Node(ids[3]), Slot::Node(ids[2])]);
    }

    #[test]
    fn no_candidate_is_offered_twice() {
        let (_, l) = build(7, &[(0, 3), (1, 4), (2, 3), (0, 5), (3, 6), (4, 6), (5, 6)]);
        let candidates = orderings(&l);
        for (at, candidate) in candidates.iter().enumerate() {
            assert!(
                !candidates[..at].contains(candidate),
                "candidate {at} repeats an earlier one"
            );
        }
    }

    #[test]
    fn a_candidate_never_gains_or_loses_a_slot() {
        let (_, l) = build(6, &[(0, 1), (0, 5), (1, 2), (2, 3), (3, 5), (0, 4), (4, 5)]);
        let mut want: Vec<_> = l.all().iter().flatten().copied().collect();
        want.sort_unstable();
        for candidate in orderings(&l) {
            let mut got: Vec<_> = candidate.iter().flatten().copied().collect();
            got.sort_unstable();
            assert_eq!(got, want);
        }
    }

    #[test]
    fn a_sweep_is_the_same_every_run() {
        let (_, l) = build(7, &[(0, 3), (1, 4), (2, 3), (0, 5), (3, 6), (4, 6), (5, 6)]);
        let once = swept(&l);
        for _ in 0..8 {
            assert_eq!(swept(&l), once);
        }
    }
}
