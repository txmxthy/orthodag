//! Packing vertical runs into channels.
//!
//! In the gap between two columns, every edge that changes row needs a vertical
//! run. Two runs on the same column of cells would be drawn as one line, so each
//! needs a channel of its own — a **track**, one cell wide.
//!
//! Runs are packed by first fit over their row intervals, which is interval
//! graph colouring and is optimal for the number of tracks. Which track a run
//! lands on, and in what left-to-right order the tracks sit, is what decides
//! whether a fan reads as a trunk or as a comb.

use super::layer::Slot;
use crate::graph::EdgeId;

/// One edge changing row in one gap.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Run {
    pub(crate) edge: EdgeId,
    pub(crate) gap: usize,
    /// The row it comes in on from the left.
    pub(crate) enter: i32,
    /// The row it leaves on to the right.
    pub(crate) leave: i32,
    /// What it leaves and what it arrives at, which is what may be shared.
    pub(crate) from: Slot,
    pub(crate) to: Slot,
}

impl Run {
    /// The top of the rows it occupies.
    fn lo(self) -> i32 {
        self.enter.min(self.leave)
    }

    /// The bottom of the rows it occupies.
    fn hi(self) -> i32 {
        self.enter.max(self.leave)
    }

    /// Whether their row intervals miss each other entirely.
    fn clear_of(self, other: Self) -> bool {
        self.hi() < other.lo() || other.hi() < self.lo()
    }

    /// Whether they leave the same slot or arrive at the same one.
    ///
    /// Two such runs overlapping on a track is not two lines drawn as one — it
    /// *is* one line, the trunk they share, with a branch off it. Forbidding it
    /// gives a fan a track per branch and draws it as a comb.
    fn meets(self, other: Self) -> bool {
        self.from == other.from || self.to == other.to
    }

    /// Whether two runs can sit on the same column of cells.
    fn may_share(self, other: Self) -> bool {
        self.clear_of(other) || self.meets(other)
    }
}

/// Which track each run sits on, and how many tracks each gap needs.
#[derive(Clone, Debug, Default)]
pub(crate) struct Tracks {
    of: Vec<usize>,
    per_gap: Vec<usize>,
}

impl Tracks {
    /// The track a run sits on.
    pub(crate) fn of(&self, run: usize) -> usize {
        self.of.get(run).copied().unwrap_or(0)
    }

    /// How many tracks a gap needs.
    pub(crate) fn count(&self, gap: usize) -> usize {
        self.per_gap.get(gap).copied().unwrap_or(0)
    }
}

/// Packs every run onto the leftmost track that has room for it.
///
/// Runs are taken in a fixed order — top of the drawing first, then by edge —
/// so the packing is the same every run, and a run only joins a track where it
/// either misses everything already there or meets it at an end.
pub(crate) fn pack(runs: &[Run], gaps: usize) -> Tracks {
    let mut order: Vec<usize> = (0..runs.len()).collect();
    order.sort_by_key(|at| runs.get(*at).map(|r| (r.gap, r.lo(), r.hi(), r.edge)));

    let mut of = vec![0; runs.len()];
    // Per gap, the runs already on each track.
    let mut taken: Vec<Vec<Vec<usize>>> = vec![Vec::new(); gaps];

    for at in order {
        let Some(run) = runs.get(at) else { continue };
        let Some(gap) = taken.get_mut(run.gap) else {
            continue;
        };

        let free = gap
            .iter()
            .position(|track| track.iter().all(|held| runs[*held].may_share(*run)));
        let track = free.unwrap_or_else(|| {
            gap.push(Vec::new());
            gap.len() - 1
        });
        if let Some(held) = gap.get_mut(track) {
            held.push(at);
        }
        of[at] = track;
    }

    for tracks in &taken {
        let places = order_tracks(runs, tracks);
        for (track, held) in tracks.iter().enumerate() {
            let Some(place) = places.get(track) else {
                continue;
            };
            for at in held {
                if let Some(slot) = of.get_mut(*at) {
                    *slot = *place;
                }
            }
        }
    }

    Tracks {
        of,
        per_gap: taken.iter().map(Vec::len).collect(),
    }
}

/// Beyond this many tracks in one gap, trying every order costs more than it is
/// worth, and adjacent swaps get most of the way there.
const EXACT: usize = 6;

/// Puts the tracks of one gap in a left-to-right order.
///
/// A track's position decides which horizontals cross it. Two things can go
/// wrong there, and they are not equally bad:
///
/// - a **landing**: a horizontal passing through the exact row where another
///   run turns, which leaves a junction glyph and reads as one line continuing
///   rather than two lines meeting;
/// - a **crossing**: a horizontal passing through the middle of another run,
///   which reads correctly and merely costs.
///
/// So the search minimises landings first and crossings second.
fn order_tracks(runs: &[Run], tracks: &[Vec<usize>]) -> Vec<usize> {
    let count = tracks.len();
    if count < 2 {
        return (0..count).collect();
    }

    let mut best: Vec<usize> = (0..count).collect();
    let mut best_cost = cost(runs, tracks, &best);

    if count <= EXACT {
        let mut order = best.clone();
        while next_permutation(&mut order) {
            let now = cost(runs, tracks, &order);
            if now < best_cost {
                best_cost = now;
                best.clone_from(&order);
            }
        }
    } else {
        // Adjacent swaps until nothing improves. Deterministic, and standing in
        // for the exact search rather than the other way round.
        let mut moved = true;
        while moved {
            moved = false;
            for at in 0..count - 1 {
                best.swap(at, at + 1);
                let now = cost(runs, tracks, &best);
                if now < best_cost {
                    best_cost = now;
                    moved = true;
                } else {
                    best.swap(at, at + 1);
                }
            }
        }
    }

    // `best` lists tracks in left-to-right order; the caller wants each track's
    // place, which is the inverse.
    let mut places = vec![0; count];
    for (place, track) in best.iter().enumerate() {
        if let Some(slot) = places.get_mut(*track) {
            *slot = place;
        }
    }
    places
}

/// What one left-to-right order costs: landings first, then crossings.
fn cost(runs: &[Run], tracks: &[Vec<usize>], order: &[usize]) -> (usize, usize) {
    let (mut landings, mut crossings) = (0, 0);

    for (place, track) in order.iter().enumerate() {
        let Some(held) = tracks.get(*track) else {
            continue;
        };
        for at in held {
            let Some(run) = runs.get(*at) else { continue };
            // The horizontal in reaches this track from the left; the one out
            // leaves it to the right.
            let reaches = [
                (run.enter, &order[..place]),
                (run.leave, &order[place + 1..]),
            ];
            for (row, passed) in reaches {
                for crossed in passed {
                    let Some(others) = tracks.get(*crossed) else {
                        continue;
                    };
                    for other in others {
                        let Some(other) = runs.get(*other) else {
                            continue;
                        };
                        if other.edge == run.edge {
                            continue;
                        }
                        if row == other.lo() || row == other.hi() {
                            landings += 1;
                        } else if row > other.lo() && row < other.hi() {
                            crossings += 1;
                        }
                    }
                }
            }
        }
    }

    (landings, crossings)
}

/// The next permutation in lexicographic order, or `false` at the last one.
fn next_permutation(order: &mut [usize]) -> bool {
    let Some(pivot) = (0..order.len().saturating_sub(1))
        .rev()
        .find(|at| order[*at] < order[at + 1])
    else {
        return false;
    };
    let Some(swap) = (pivot + 1..order.len())
        .rev()
        .find(|at| order[*at] > order[pivot])
    else {
        return false;
    };
    order.swap(pivot, swap);
    order[pivot + 1..].reverse();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::NodeId;

    fn node(id: u32) -> Slot {
        Slot::Node(NodeId::from_index(id as usize))
    }

    fn run(edge: u32, enter: i32, leave: i32, from: Slot, to: Slot) -> Run {
        Run {
            edge: EdgeId::from_index(edge as usize),
            gap: 0,
            enter,
            leave,
            from,
            to,
        }
    }

    #[test]
    fn every_permutation_is_visited_once() {
        let mut order = vec![0, 1, 2];
        let mut seen = vec![order.clone()];
        while next_permutation(&mut order) {
            seen.push(order.clone());
        }
        assert_eq!(seen.len(), 6);
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 6);
    }

    #[test]
    fn a_horizontal_through_another_runs_corner_is_a_landing() {
        // One run turns at row 4; the other arrives on row 4.
        let a = run(0, 0, 4, node(0), node(1));
        let b = run(1, 4, 9, node(2), node(3));
        let tracks = vec![vec![0], vec![1]];
        // a on the left means b's incoming horizontal at row 4 crosses a's
        // track exactly where a turns.
        assert!(cost(&[a, b], &tracks, &[0, 1]).0 > 0);
        assert_eq!(cost(&[a, b], &tracks, &[1, 0]).0, 0);
    }

    #[test]
    fn the_order_that_avoids_a_landing_is_the_one_chosen() {
        let a = run(0, 0, 4, node(0), node(1));
        let b = run(1, 4, 9, node(2), node(3));
        let places = order_tracks(&[a, b], &[vec![0], vec![1]]);
        assert_eq!(
            places,
            [1, 0],
            "the track that would be landed on goes right"
        );
    }

    #[test]
    fn one_track_needs_no_ordering() {
        assert_eq!(order_tracks(&[], &[vec![]]), [0]);
        assert_eq!(order_tracks(&[], &[]), Vec::<usize>::new());
    }

    #[test]
    fn ordering_is_a_permutation_however_many_tracks_there_are() {
        let runs: Vec<Run> = (0..9u32)
            .map(|i| {
                let row = i32::try_from(i).unwrap_or(0) * 2;
                run(i, row, row + 7, node(i), node(50))
            })
            .collect();
        let tracks: Vec<Vec<usize>> = (0..9).map(|i| vec![i]).collect();
        let mut places = order_tracks(&runs, &tracks);
        places.sort_unstable();
        assert_eq!(places, (0..9).collect::<Vec<_>>());
    }

    #[test]
    fn runs_that_do_not_touch_share_a_track() {
        let runs = [
            run(0, 0, 2, node(0), node(1)),
            run(1, 4, 6, node(2), node(3)),
        ];
        let tracks = pack(&runs, 1);
        assert_eq!(tracks.of(0), tracks.of(1));
        assert_eq!(tracks.count(0), 1);
    }

    #[test]
    fn runs_that_overlap_take_a_track_each() {
        let runs = [
            run(0, 0, 5, node(0), node(1)),
            run(1, 3, 8, node(2), node(3)),
        ];
        let tracks = pack(&runs, 1);
        assert_ne!(tracks.of(0), tracks.of(1));
        assert_eq!(tracks.count(0), 2);
    }

    #[test]
    fn touching_at_one_row_is_still_touching() {
        let runs = [
            run(0, 0, 3, node(0), node(1)),
            run(1, 3, 6, node(2), node(3)),
        ];
        assert_eq!(pack(&runs, 1).count(0), 2);
    }

    #[test]
    fn runs_that_overlap_but_leave_the_same_slot_share_a_track() {
        let source = node(0);
        let runs = [
            run(0, 0, 5, source, node(1)),
            run(1, 3, 8, source, node(2)),
            run(2, 1, 9, source, node(3)),
        ];
        let tracks = pack(&runs, 1);
        assert_eq!(tracks.count(0), 1, "a fan out of one box is one trunk");
        assert_eq!((tracks.of(0), tracks.of(1)), (tracks.of(2), tracks.of(2)));
    }

    #[test]
    fn runs_that_overlap_but_arrive_at_the_same_slot_share_a_track() {
        let sink = node(9);
        let runs = [run(0, 0, 5, node(0), sink), run(1, 3, 8, node(1), sink)];
        assert_eq!(pack(&runs, 1).count(0), 1);
    }

    #[test]
    fn meeting_at_an_end_does_not_excuse_an_unrelated_third_run() {
        let source = node(0);
        let runs = [
            run(0, 0, 5, source, node(1)),
            run(1, 3, 8, source, node(2)),
            run(2, 2, 7, node(5), node(6)),
        ];
        let tracks = pack(&runs, 1);
        assert_eq!(tracks.of(0), tracks.of(1));
        assert_ne!(tracks.of(2), tracks.of(0));
    }

    #[test]
    fn a_gap_with_nothing_in_it_needs_no_tracks() {
        assert_eq!(pack(&[], 3).count(0), 0);
    }

    #[test]
    fn packing_is_the_same_whatever_order_the_runs_arrive_in() {
        let a = run(0, 0, 5, node(0), node(1));
        let b = run(1, 3, 8, node(2), node(3));
        let c = run(2, 9, 11, node(4), node(5));
        let forward = pack(&[a, b, c], 1);
        let backward = pack(&[c, b, a], 1);
        assert_eq!(forward.count(0), backward.count(0));
        assert_eq!(forward.of(0), backward.of(2));
    }

    #[test]
    fn a_run_in_another_gap_is_not_in_the_way() {
        let mut far = run(1, 0, 5, node(2), node(3));
        far.gap = 1;
        let tracks = pack(&[run(0, 0, 5, node(0), node(1)), far], 2);
        assert_eq!((tracks.count(0), tracks.count(1)), (1, 1));
    }
}
