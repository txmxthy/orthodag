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
use crate::colour::Colour;
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
    /// Which flow it belongs to, or `None` where it carries no tags.
    pub(crate) ink: Option<Colour>,
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

    /// Whether they are one line: the same flow, meeting at an end.
    ///
    /// Two such runs overlapping on a track is not two lines drawn as one — it
    /// *is* one line, the trunk they share, with a branch off it. Forbidding it
    /// gives a fan a track per branch and draws it as a comb.
    ///
    /// The flow has to match. Sharing an end makes two runs *eligible* to be one
    /// line; carrying the same colour is what makes them one. Two flows out of
    /// one box put on one track are a trunk the reader cannot take apart —
    /// `score::blends` counts exactly this — and the comb is the better of the
    /// two pictures, because a comb can at least be followed.
    ///
    /// Untagged runs share a flow with each other and with nothing else: they
    /// are all drawn in the caller's default ink, so there is no colour for a
    /// shared trunk to lose.
    fn meets(self, other: Self) -> bool {
        self.ink == other.ink && (self.from == other.from || self.to == other.to)
    }

    /// Whether two runs can sit on the same column of cells.
    fn may_share(self, other: Self) -> bool {
        self.clear_of(other) || self.meets(other)
    }

    /// Whether this run has to sit left of `other`.
    ///
    /// A run's entry stub runs along its `enter` row from the gap's left edge
    /// to its track, and its exit stub along its `leave` row from its track to
    /// the right edge. Two runs of different flows can share a row in only one
    /// way — one leaves on the row the other enters on — and then the two stubs
    /// overlap unless the one entering turns first. The overlap is two flows in
    /// one cell, which is exactly what `score::blends` counts. Runs that are one
    /// line anyway are free to share.
    fn precedes(self, other: Self) -> bool {
        other.leave == self.enter && self.edge != other.edge && !self.meets(other)
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

/// Orders every run of a gap left to right, then folds neighbours onto one
/// track where they can share it.
///
/// The order comes first because it is what decides what crosses what: a
/// run's stubs cross every track between its own and the gap's edges. Packing
/// first and ordering the packed tracks afterwards could pin two runs to one
/// track that the order then had to keep on the wrong side of a third. So each
/// run is ordered on its own, and only then are neighbours in that order put on
/// one column of cells when they miss each other or meet at an end — which
/// keeps every run on the same side of every other, so nothing the order paid
/// for is lost, and the gap is as narrow as that order allows.
pub(crate) fn pack(runs: &[Run], gaps: usize) -> Tracks {
    let mut per_gap: Vec<Vec<usize>> = vec![Vec::new(); gaps];
    for (at, run) in runs.iter().enumerate() {
        if let Some(held) = per_gap.get_mut(run.gap) {
            held.push(at);
        }
    }

    let mut of = vec![0; runs.len()];
    let mut per_gap_count = Vec::with_capacity(gaps);
    for held in &per_gap {
        let mut held = held.clone();
        held.sort_by_key(|at| runs.get(*at).map(|r| (r.lo(), r.hi(), r.edge)));

        // A trunk is one line and is ordered as one: runs of one flow that
        // meet at an end go on one track before anything is ordered.
        let mut trunks: Vec<Vec<usize>> = Vec::new();
        for at in held {
            let Some(run) = runs.get(at) else { continue };
            let joined = trunks.iter_mut().find(|track| {
                track.iter().any(|h| runs[*h].meets(*run))
                    && track.iter().all(|h| runs[*h].may_share(*run))
            });
            match joined {
                Some(track) => track.push(at),
                None => trunks.push(vec![at]),
            }
        }
        let places = order_tracks(runs, &trunks);
        let mut ordered: Vec<usize> = (0..trunks.len()).collect();
        ordered.sort_by_key(|slot| places.get(*slot).copied().unwrap_or(0));

        let mut tracks: Vec<Vec<usize>> = Vec::new();
        for slot in ordered {
            let trunk = &trunks[slot];
            let fits = tracks.last().is_some_and(|track| {
                trunk
                    .iter()
                    .all(|at| track.iter().all(|h| runs[*h].may_share(runs[*at])))
            });
            match tracks.last_mut() {
                Some(track) if fits => track.extend(trunk.iter().copied()),
                _ => tracks.push(trunk.clone()),
            }
        }
        for (track, held) in tracks.iter().enumerate() {
            for at in held {
                of[*at] = track;
            }
        }
        per_gap_count.push(tracks.len());
    }

    Tracks {
        of,
        per_gap: per_gap_count,
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

    let between = Between::of(runs, tracks);
    let mut best: Vec<usize> = nesting(runs, tracks);
    let mut best_cost = between.cost(&best);

    if count <= EXACT {
        let mut order: Vec<usize> = (0..count).collect();
        loop {
            let now = between.cost(&order);
            if now < best_cost {
                best_cost = now;
                best.clone_from(&order);
            }
            if !next_permutation(&mut order) {
                break;
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
                let now = between.cost(&best);
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

/// The order a fan nests in: the branch that travels furthest turns first.
///
/// `design.md` §4.5 says the outermost branch of a fan turns first, and this is
/// that sentence as an ordering. Two runs leaving one box at neighbouring rows
/// and climbing to different heights do not cross when the taller one takes the
/// nearer track — its horizontal leaves above everything the shorter one
/// occupies — and cross once when they are the other way round.
///
/// It matters because it is where the search starts. Past [`EXACT`] tracks the
/// order is hill-climbed by adjacent swaps, and a hill climb is worth about as
/// much as the place it begins: first-fit packing order, which is what it used
/// to begin from, says nothing about left-to-right at all.
///
/// A track that has to sit left of another ([`Run::precedes`]) is placed
/// first whatever its reach, so the seed already avoids every landing that can
/// be avoided; where those constraints form a cycle the track with the fewest
/// still waiting goes next, and the search inherits the one landing no order
/// could remove. Ties go to the lower track index, so the seed is the same
/// every run.
fn nesting(runs: &[Run], tracks: &[Vec<usize>]) -> Vec<usize> {
    let reach = |track: usize| {
        tracks
            .get(track)
            .into_iter()
            .flatten()
            .filter_map(|at| runs.get(*at))
            .map(|run| run.hi() - run.lo())
            .max()
            .unwrap_or(0)
    };
    let before = |left: usize, right: usize| {
        left != right
            && tracks[left]
                .iter()
                .any(|l| tracks[right].iter().any(|r| runs[*l].precedes(runs[*r])))
    };

    let count = tracks.len();
    let mut order: Vec<usize> = Vec::with_capacity(count);
    let mut placed = vec![false; count];
    while order.len() < count {
        let waiting = |t: usize| (0..count).filter(|l| !placed[*l] && before(*l, t)).count();
        let Some(next) = (0..count)
            .filter(|t| !placed[*t])
            .min_by_key(|t| (waiting(*t), std::cmp::Reverse(reach(*t)), *t))
        else {
            break;
        };
        placed[next] = true;
        order.push(next);
    }
    order
}

/// What one left-to-right order costs: landings first, then crossings.
/// What one track costs another, for each way round the two can sit.
///
/// The sum in [`cost`] never looks at the whole order. A run on track `T` charges
/// its `enter` against the runs on every track left of `T` and its `leave`
/// against those right of it, and which tracks those are is the only thing the
/// order decides. So every pair of tracks contributes the same two numbers
/// wherever they sit, and an order is the sum of the pairs it puts in each
/// arrangement.
///
/// Worked out once per gap in a pass over the runs, an order then costs one
/// addition per pair instead of a walk over every run it passes. The arithmetic
/// is the same arithmetic — the same integers added in a different grouping —
/// so the order that wins is the order that won before.
struct Between {
    tracks: usize,
    /// `[left][right]`: what the runs of `right` owe for the tracks of `left`
    /// sitting to their left, and what the runs of `left` owe for `right`
    /// sitting to their right.
    pairs: Vec<(usize, usize)>,
}

impl Between {
    fn of(runs: &[Run], tracks: &[Vec<usize>]) -> Self {
        let count = tracks.len();
        let mut pairs = vec![(0, 0); count * count];

        for (left, held) in tracks.iter().enumerate() {
            for (right, others) in tracks.iter().enumerate() {
                if left == right {
                    continue;
                }
                let mut owed = (0, 0);
                // A run on `right`, reached from the left by way of `left`.
                for at in others {
                    let Some(run) = runs.get(*at) else { continue };
                    tally(&mut owed, runs, held, run.enter, *run);
                }
                // A run on `left`, leaving to the right across `right`.
                for at in held {
                    let Some(run) = runs.get(*at) else { continue };
                    tally(&mut owed, runs, others, run.leave, *run);
                }
                if let Some(slot) = pairs.get_mut(left * count + right) {
                    *slot = owed;
                }
            }
        }
        Self {
            tracks: count,
            pairs,
        }
    }

    /// What one left-to-right order costs: landings first, then crossings.
    fn cost(&self, order: &[usize]) -> (usize, usize) {
        let (mut landings, mut crossings) = (0, 0);
        for (place, left) in order.iter().enumerate() {
            for right in &order[place + 1..] {
                let Some((owed_landings, owed_crossings)) =
                    self.pairs.get(left * self.tracks + right)
                else {
                    continue;
                };
                landings += owed_landings;
                crossings += owed_crossings;
            }
        }
        (landings, crossings)
    }
}

/// What one row costs against the runs held on one track.
///
/// Two runs that are one line — the same flow, meeting at an end — do not
/// cross each other whatever the order puts between them, so they cost
/// nothing here; the packing folds them onto one track when they are neighbours.
fn tally(owed: &mut (usize, usize), runs: &[Run], held: &[usize], row: i32, run: Run) {
    for at in held {
        let Some(other) = runs.get(*at) else { continue };
        if other.edge == run.edge || other.meets(run) {
            continue;
        }
        if row == other.lo() || row == other.hi() {
            owed.0 += 1;
        } else if row > other.lo() && row < other.hi() {
            owed.1 += 1;
        }
    }
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

    /// An untagged run. Untagged runs all share the one flow, so these keep
    /// testing what they always tested: whether meeting at an end is enough.
    fn run(edge: u32, enter: i32, leave: i32, from: Slot, to: Slot) -> Run {
        inked(edge, enter, leave, from, to, None)
    }

    fn inked(edge: u32, enter: i32, leave: i32, from: Slot, to: Slot, ink: Option<Colour>) -> Run {
        Run {
            edge: EdgeId::from_index(edge as usize),
            gap: 0,
            enter,
            leave,
            from,
            to,
            ink,
        }
    }

    /// Two flows out of one box do not share a trunk, however much they overlap.
    ///
    /// Meeting at an end makes two runs eligible to be one line; carrying the
    /// same colour is what makes them one. Put two flows on a single track and
    /// the reader cannot take the trunk apart again — a comb is worse to look at
    /// and better to follow.
    #[test]
    fn two_flows_leaving_one_box_take_a_track_each() {
        let (a, b) = (node(0), node(1));
        let red = Some(Colour::from_slot(0));
        let blue = Some(Colour::from_slot(1));

        let same = pack(
            &[inked(0, 0, 4, a, b, red), inked(1, 0, 6, a, node(2), red)],
            1,
        );
        assert_eq!(same.of(0), same.of(1), "one flow, one trunk");

        let apart = pack(
            &[inked(0, 0, 4, a, b, red), inked(1, 0, 6, a, node(2), blue)],
            1,
        );
        assert_ne!(apart.of(0), apart.of(1), "two flows, two tracks");
    }

    /// The pair table says what walking the whole order used to say.
    ///
    /// The grouping changed and the arithmetic did not, so this checks the two
    /// against each other on a gap busy enough to have opinions: four runs on
    /// three tracks, every order, both numbers.
    #[test]
    fn the_pair_table_agrees_with_walking_the_order() {
        let runs = [
            run(0, 0, 6, node(0), node(1)),
            run(1, 6, 2, node(2), node(3)),
            run(2, 3, 9, node(4), node(5)),
            run(3, 9, 0, node(6), node(7)),
        ];
        let tracks = vec![vec![0, 1], vec![2], vec![3]];
        let between = Between::of(&runs, &tracks);

        let mut order = vec![0, 1, 2];
        loop {
            assert_eq!(
                between.cost(&order),
                walked(&runs, &tracks, &order),
                "order {order:?}"
            );
            if !next_permutation(&mut order) {
                break;
            }
        }
    }

    /// The sum written the long way, kept only to check the short way.
    fn walked(runs: &[Run], tracks: &[Vec<usize>], order: &[usize]) -> (usize, usize) {
        let (mut landings, mut crossings) = (0, 0);
        for (place, track) in order.iter().enumerate() {
            let Some(held) = tracks.get(*track) else {
                continue;
            };
            for at in held {
                let Some(run) = runs.get(*at) else { continue };
                for (row, passed) in [
                    (run.enter, &order[..place]),
                    (run.leave, &order[place + 1..]),
                ] {
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
        let between = Between::of(&[a, b], &tracks);
        assert!(between.cost(&[0, 1]).0 > 0);
        assert_eq!(between.cost(&[1, 0]).0, 0);
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

    /// v20->v21 leaves on the row v20->v13 enters on, so v20->v13 has to turn
    /// first; a second such pair the other way round used to make a
    /// landing-free order impossible, because first fit had already put the
    /// two constraints on tracks that wanted opposite orders.
    #[test]
    fn a_run_leaving_on_anothers_entry_row_takes_a_track_to_its_right() {
        let (red, blue) = (Some(Colour::from_slot(0)), Some(Colour::from_slot(1)));
        let a = inked(0, 9, 8, node(20), node(21), red);
        let b = inked(1, 8, 5, node(20), node(13), blue);
        let later = run(2, 14, 12, node(3), node(4));
        let earlier = run(3, 12, 16, node(5), node(6));

        let t = pack(&[a, b, later, earlier], 1);
        assert!(t.of(1) < t.of(0), "the run entering on row 8 sits left");
        assert!(t.of(3) < t.of(2), "the run entering on row 12 sits left");
    }

    /// v24->v20 leaves on row 11, which v24->v21 enters on.
    #[test]
    fn two_flows_out_of_one_box_on_neighbouring_rows_do_not_land() {
        let (red, blue) = (Some(Colour::from_slot(0)), Some(Colour::from_slot(1)));
        let a = inked(0, 10, 11, node(24), node(20), red);
        let b = inked(1, 11, 12, node(24), node(21), blue);
        let earlier = run(3, 13, 14, node(5), node(6));
        let later = run(2, 16, 13, node(3), node(4));

        let t = pack(&[a, b, later, earlier], 1);
        assert!(t.of(1) < t.of(0));
        assert!(t.of(3) < t.of(2));
    }

    /// Two boxes on rows a and b feeding two boxes on rows b and a: each run
    /// wants to sit left of the other, which no order can give. The packing
    /// still ends, on two tracks, with the one landing placement will have to
    /// move.
    #[test]
    fn a_cycle_of_precedences_still_packs() {
        let (red, blue) = (Some(Colour::from_slot(0)), Some(Colour::from_slot(1)));
        let a = inked(0, 3, 7, node(0), node(1), red);
        let b = inked(1, 7, 3, node(2), node(3), blue);
        let t = pack(&[a, b], 1);
        assert_eq!(t.count(0), 2);
        assert_ne!(t.of(0), t.of(1));
    }

    /// A run that has to sit right of another still shares a track with a
    /// third it misses, so the constraint costs no width it need not.
    #[test]
    fn a_constrained_run_still_shares_a_track_it_can() {
        let (red, blue) = (Some(Colour::from_slot(0)), Some(Colour::from_slot(1)));
        let a = inked(0, 9, 8, node(20), node(21), red);
        let b = inked(1, 8, 5, node(20), node(13), blue);
        let far = run(2, 20, 24, node(3), node(4));
        let t = pack(&[a, b, far], 1);
        assert!(t.of(1) < t.of(0));
        assert_eq!(t.count(0), 2);
    }

    #[test]
    fn a_run_in_another_gap_is_not_in_the_way() {
        let mut far = run(1, 0, 5, node(2), node(3));
        far.gap = 1;
        let tracks = pack(&[run(0, 0, 5, node(0), node(1)), far], 2);
        assert_eq!((tracks.count(0), tracks.count(1)), (1, 1));
    }
}
