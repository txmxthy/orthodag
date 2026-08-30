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
    /// The rows it spans, top first.
    pub(crate) lo: i32,
    pub(crate) hi: i32,
    /// What it leaves and what it arrives at, which is what may be shared.
    pub(crate) from: Slot,
    pub(crate) to: Slot,
}

impl Run {
    /// Whether two runs can sit on the same column of cells.
    fn clear_of(self, other: Self) -> bool {
        self.hi < other.lo || other.hi < self.lo
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
/// so the packing is the same every run, and a run only ever joins a track
/// where it does not touch what is already there.
pub(crate) fn pack(runs: &[Run], gaps: usize) -> Tracks {
    let mut order: Vec<usize> = (0..runs.len()).collect();
    order.sort_by_key(|at| runs.get(*at).map(|r| (r.gap, r.lo, r.hi, r.edge)));

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
            .position(|track| track.iter().all(|held| runs[*held].clear_of(*run)));
        let track = free.unwrap_or_else(|| {
            gap.push(Vec::new());
            gap.len() - 1
        });
        if let Some(held) = gap.get_mut(track) {
            held.push(at);
        }
        of[at] = track;
    }

    Tracks {
        of,
        per_gap: taken.iter().map(Vec::len).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::NodeId;

    fn node(id: u32) -> Slot {
        Slot::Node(NodeId::from_index(id as usize))
    }

    fn run(edge: u32, lo: i32, hi: i32, from: Slot, to: Slot) -> Run {
        Run {
            edge: EdgeId::from_index(edge as usize),
            gap: 0,
            lo,
            hi,
            from,
            to,
        }
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
