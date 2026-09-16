//! Turning rows and columns into orthogonal polylines.
//!
//! Placement said which row everything sits on. Routing says where the lines
//! go: out of a box's right edge, across the gap, into the next box's left
//! edge, turning only at right angles.
//!
//! The vocabulary is deliberately small. Between neighbouring columns an edge
//! is straight, an **L** with one bend, or a **Z** with two — nothing else. An
//! edge that skips columns turns at each end and runs straight across the
//! middle, because the placeholder rows it runs on are all the same row. Four
//! bends at most, whatever the skip.
//!
//! Where in the gap an edge turns is not a free choice. Two edges turning on the
//! same column of cells are drawn as one line, so every vertical run takes a
//! track of its own — see [`super::track`] — and the gap is made wide enough to
//! hold them.

use super::acyclic::Acyclic;
use super::layer::Slot;
use super::order::Columns;
use super::place::Placed;
use super::port::Ports;
use super::track::{Run, Tracks, pack};
use crate::drawing::Heading;
use crate::graph::{EdgeId, Graph, NodeId};
use crate::options::Options;

/// How the drawing is sized and captioned.
///
/// A drawing has a natural size, and a caller asking for less gets a walk down
/// a fixed ladder rather than a search: predictable, and every rung is a
/// drawing somebody could have asked for on its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Style {
    /// The fewest blank columns between one column of boxes and the next.
    pub(crate) gap: i32,
    /// The most characters of a box's text to keep.
    pub(crate) cap: usize,
    /// Whether each edge carries its tags.
    pub(crate) labels: bool,
    /// Every box this wide, when the caller fixed it.
    pub(crate) box_width: Option<i32>,
    /// No box shorter than this, when the caller asked.
    pub(crate) box_height: Option<i32>,
}

impl Style {
    /// The natural drawing: roomy gaps, nothing trimmed.
    pub(crate) fn natural(options: Options) -> Self {
        Self {
            gap: MIN_GAP,
            cap: usize::MAX,
            labels: options.labels,
            box_width: fixed_width(options),
            box_height: options.box_height.and_then(|h| i32::try_from(h).ok()),
        }
    }

    /// The rungs, widest first. The last is the legibility floor: below six
    /// characters a box says nothing worth reading, so nothing goes below it.
    pub(crate) fn ladder(options: Options) -> [Self; 5] {
        let rung = |gap, cap| Self {
            gap,
            cap,
            labels: options.labels,
            box_width: fixed_width(options),
            box_height: options.box_height.and_then(|h| i32::try_from(h).ok()),
        };
        [
            Self::natural(options),
            rung(4, 24),
            rung(3, 16),
            rung(3, 10),
            rung(3, 6),
        ]
    }
}

/// A fixed box width the caller asked for, floored at the narrowest box.
fn fixed_width(options: Options) -> Option<i32> {
    options
        .box_width
        .map(|w| i32::try_from(w).unwrap_or(i32::MAX).max(MIN_WIDTH))
}

/// The fewest blank columns between one column of boxes and the next, unsqueezed.
const MIN_GAP: i32 = 5;

/// Clearance either side of a gap's tracks: one cell of stub out of the box,
/// and two on the other side so an arrowhead has a line to sit on the end of
/// rather than appearing straight after a corner.
const CLEARANCE: usize = 3;

/// Padding inside a box: two borders and a space either side of the text.
const PADDING: i32 = 4;

/// The narrowest a box may be drawn.
const MIN_WIDTH: i32 = PADDING + 1;

/// A run of text drawn on an edge, in cells.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Label {
    pub(crate) edge: EdgeId,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) text: String,
}

/// Where one box sits, in cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Boxed {
    pub(crate) node: NodeId,
    /// Which column it is in, which is how far an edge into it has come.
    pub(crate) column: usize,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
}

/// One edge as an orthogonal polyline, corner to corner.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Route {
    pub(crate) edge: EdgeId,
    pub(crate) points: Vec<(i32, i32)>,
}

impl Route {
    /// How many right angles the line turns through.
    pub(crate) fn bends(&self) -> usize {
        self.points.len().saturating_sub(2)
    }

    /// Which way the arrowhead points: a forward edge arrives from the left, a
    /// back edge comes up out of its lane. The last segment says which.
    pub(crate) fn heading(&self) -> Heading {
        match (self.points.iter().nth_back(1), self.points.last()) {
            (Some(before), Some(at)) if before.0 == at.0 && before.1 > at.1 => Heading::Up,
            _ => Heading::Right,
        }
    }
}

/// A drawing in cell coordinates, with no character in it yet.
#[derive(Clone, Debug, Default)]
pub(crate) struct Layout {
    pub(crate) boxes: Vec<Boxed>,
    pub(crate) routes: Vec<Route>,
    pub(crate) labels: Vec<Label>,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

impl Layout {
    /// One box, by the node it draws.
    pub(crate) fn boxed(&self, node: NodeId) -> Option<&Boxed> {
        self.boxes.iter().find(|b| b.node == node)
    }
}

/// The row an edge is on at each column it touches, source to target.
///
/// Rows are settled before any x is, because they have to be: how wide a gap
/// needs to be depends on how many runs cross it, and which runs cross it
/// depends on which edges change row there.
struct Path {
    edge: EdgeId,
    first: usize,
    rows: Vec<i32>,
    from: NodeId,
    to: NodeId,
}

impl Path {
    /// The slot this edge occupies in one of its columns.
    fn slot(&self, column: usize) -> Slot {
        if column == self.first {
            Slot::Node(self.from)
        } else if column + 1 == self.first + self.rows.len() {
            Slot::Node(self.to)
        } else {
            Slot::Pass(self.edge)
        }
    }
}

/// Lays out the boxes and routes every edge that has a path across the columns.
pub(crate) fn route(
    g: &Graph,
    acyclic: &Acyclic,
    columns: &Columns,
    placed: &Placed,
    ports: &Ports,
    style: Style,
) -> Layout {
    let widths = widths(g, columns, style);
    let paths = paths(g, acyclic, columns, placed, ports);
    let runs = runs(g, &paths, &crate::colour::of(g));
    let tracks = pack(&runs, columns.len().saturating_sub(1));

    let captions = captions(g, &paths, style);
    let gaps = gaps(&tracks, &captions, columns.len(), style);
    let lefts = lefts(&widths, &gaps, style);
    let boxes = boxes(columns, placed, &widths, &lefts);

    let mut routes: Vec<Route> = paths
        .iter()
        .filter_map(|path| {
            let points = polyline(path, &runs, &tracks, &captions, &lefts, &widths)?;
            Some(Route {
                edge: path.edge,
                points,
            })
        })
        .collect();

    let (back, lanes) = back_routes(g, acyclic, &boxes, placed.height());
    routes.extend(back);
    let labels = labels(&paths, &captions, &lefts, &widths);

    let width = lefts
        .iter()
        .zip(&widths)
        .map(|(x, w)| x + w)
        .max()
        .unwrap_or(0);
    Layout {
        boxes,
        routes,
        labels,
        width,
        height: placed.height() + lanes,
    }
}

/// What each edge's caption says, and how much room each gap needs for one.
///
/// A caption belongs to the gap the edge leaves its source in, on the row it
/// leaves on. Two edges on one row carry one tag set between them, so two
/// captions can never collide.
struct Captions {
    text: Vec<Option<String>>,
    per_gap: Vec<i32>,
}

impl Captions {
    fn text(&self, at: usize) -> Option<&str> {
        self.text.get(at)?.as_deref()
    }

    fn room(&self, gap: usize) -> i32 {
        self.per_gap.get(gap).copied().unwrap_or(0)
    }
}

fn captions(g: &Graph, paths: &[Path], style: Style) -> Captions {
    if !style.labels {
        return Captions {
            text: vec![None; paths.len()],
            per_gap: Vec::new(),
        };
    }

    let text: Vec<Option<String>> = paths
        .iter()
        .map(|path| {
            let tags = g.edge(path.edge)?.tags();
            (!tags.is_empty()).then(|| tags.join(", "))
        })
        .collect();

    let mut per_gap: Vec<i32> = Vec::new();
    for (at, path) in paths.iter().enumerate() {
        let Some(caption) = text.get(at).and_then(Option::as_deref) else {
            continue;
        };
        let room = i32::try_from(caption.chars().count()).unwrap_or(0) + 1;
        if per_gap.len() <= path.first {
            per_gap.resize(path.first + 1, 0);
        }
        if let Some(slot) = per_gap.get_mut(path.first) {
            *slot = (*slot).max(room);
        }
    }
    Captions { text, per_gap }
}

/// Where each caption goes: the first cell of its gap's caption area, on the
/// row its edge leaves the source on.
fn labels(paths: &[Path], captions: &Captions, lefts: &[i32], widths: &[i32]) -> Vec<Label> {
    paths
        .iter()
        .enumerate()
        .filter_map(|(at, path)| {
            let text = captions.text(at)?;
            let start = lefts.get(path.first)? + widths.get(path.first)?;
            Some(Label {
                edge: path.edge,
                x: start + 1,
                y: *path.rows.first()?,
                text: text.to_owned(),
            })
        })
        .collect()
}

/// A blank row between the boxes and the first lane.
const LANE_MARGIN: i32 = 1;

/// Routes every back edge through a lane under the boxes, and says how many
/// rows that added.
///
/// A back edge was taken out of the layering rather than reversed, because a
/// reversed arrow reads as pointing the wrong way. That leaves it needing a
/// vocabulary of its own, and this is it: down out of the source, left along a
/// lane below everything, and up into the target. Two bends, well inside the
/// four a back edge is allowed.
///
/// One lane per source, so a box with two loops back sends them down the same
/// line and forks; shortest hops take the lanes nearest the boxes, so a short
/// loop does not have to travel under a long one.
fn back_routes(g: &Graph, acyclic: &Acyclic, boxes: &[Boxed], height: i32) -> (Vec<Route>, i32) {
    let mut sources: Vec<(NodeId, i32, Vec<EdgeId>)> = Vec::new();
    for id in g.edge_ids().filter(|id| acyclic.is_back(*id)) {
        let Some(edge) = g.edge(id) else { continue };
        let Some(from) = boxes.iter().find(|b| b.node == edge.from()) else {
            continue;
        };
        let Some(to) = boxes.iter().find(|b| b.node == edge.to()) else {
            continue;
        };
        let hop = i32::try_from(from.column.abs_diff(to.column)).unwrap_or(0);

        match sources.iter_mut().find(|(node, _, _)| *node == edge.from()) {
            Some((_, shortest, edges)) => {
                *shortest = (*shortest).min(hop);
                edges.push(id);
            }
            None => sources.push((edge.from(), hop, vec![id])),
        }
    }
    if sources.is_empty() {
        return (Vec::new(), 0);
    }
    sources.sort_by_key(|(node, shortest, _)| (*shortest, *node));

    let mut routes = Vec::new();
    for (at, (_, _, edges)) in sources.iter().enumerate() {
        let lane = height + LANE_MARGIN + i32::try_from(at).unwrap_or(0);
        for id in edges {
            let Some(edge) = g.edge(*id) else { continue };
            let (Some(from), Some(to)) = (
                boxes.iter().find(|b| b.node == edge.from()),
                boxes.iter().find(|b| b.node == edge.to()),
            ) else {
                continue;
            };
            // A self-loop leaves and returns under the same box; give the two
            // verticals a column each or the four points fold into one cell.
            let (out, back) = if from.node == to.node {
                (from.x + from.w / 2 - 1, from.x + from.w / 2 + 1)
            } else {
                (from.x + from.w / 2, to.x + to.w / 2)
            };
            routes.push(Route {
                edge: *id,
                points: collapse(vec![
                    (out, from.y + from.h),
                    (out, lane),
                    (back, lane),
                    (back, to.y + to.h),
                ]),
            });
        }
    }

    let lanes = i32::try_from(sources.len()).unwrap_or(0) + LANE_MARGIN;
    (routes, lanes)
}

/// How wide each column's boxes are: the widest text in the column, padded.
///
/// One width per column rather than per box, because boxes whose left edges do
/// not line up read as a ragged margin rather than as a column.
fn widths(g: &Graph, columns: &Columns, style: Style) -> Vec<i32> {
    if let Some(fixed) = style.box_width {
        return vec![fixed; columns.len()];
    }
    columns
        .iter()
        .map(|slots| {
            slots
                .iter()
                .filter_map(|slot| match slot {
                    Slot::Node(id) => g.node(*id),
                    Slot::Pass(_) => None,
                })
                .map(|node| {
                    let longest = std::iter::once(node.label())
                        .chain(node.lines().iter().map(String::as_str))
                        .map(|line| line.chars().count().min(style.cap))
                        .map(|len| i32::try_from(len).unwrap_or(i32::MAX))
                        .max()
                        .unwrap_or(0);
                    longest.saturating_add(PADDING)
                })
                .max()
                .unwrap_or(MIN_WIDTH)
                .max(MIN_WIDTH)
        })
        .collect()
}

/// The row every forward edge is on at each of its columns.
fn paths(
    g: &Graph,
    acyclic: &Acyclic,
    columns: &Columns,
    placed: &Placed,
    ports: &Ports,
) -> Vec<Path> {
    g.edge_ids()
        .filter(|id| !acyclic.is_back(*id))
        .filter_map(|id| {
            let edge = g.edge(id)?;
            let (from, to) = (edge.from(), edge.to());
            let first = column_of(columns, Slot::Node(from))?;
            let last = column_of(columns, Slot::Node(to))?;
            if last <= first {
                return None;
            }
            let rows = (first..=last)
                .map(|column| {
                    let slot = if column == first {
                        Slot::Node(from)
                    } else if column == last {
                        Slot::Node(to)
                    } else {
                        Slot::Pass(id)
                    };
                    let at = columns.get(column)?.iter().position(|s| *s == slot)?;
                    // A box's ends are its attach rows, one per tag set; a
                    // placeholder has only the row it was given.
                    Some(match (column == first, column == last) {
                        (true, _) => ports.exit(id).unwrap_or(placed.middle(column, at)),
                        (_, true) => ports.entry(id).unwrap_or(placed.middle(column, at)),
                        _ => placed.middle(column, at),
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Path {
                edge: id,
                first,
                rows,
                from,
                to,
            })
        })
        .collect()
}

/// Which column a slot is in.
fn column_of(columns: &Columns, slot: Slot) -> Option<usize> {
    columns.iter().position(|slots| slots.contains(&slot))
}

/// Every vertical run every path needs, in path order then gap order.
///
/// A hop that stays on its row needs none: it is drawn as one straight line and
/// nothing has to make room for it.
fn runs(g: &Graph, paths: &[Path], colours: &[Option<crate::colour::Colour>]) -> Vec<Run> {
    // The placeholders of one flow are one line, so its runs answer to one
    // slot: the tracks then hold them as a trunk with a branch each, where
    // keying them per edge gave them a track each and a crossing between.
    let rep = super::flow::representative(g);
    let shared = |slot: Slot| match slot {
        Slot::Pass(edge) => Slot::Pass(rep.get(edge.index()).copied().unwrap_or(edge)),
        Slot::Node(_) => slot,
    };
    let mut runs = Vec::new();
    for path in paths {
        for (step, pair) in path.rows.windows(2).enumerate() {
            let [from_row, to_row] = *pair else { continue };
            if from_row == to_row {
                continue;
            }
            let gap = path.first + step;
            runs.push(Run {
                edge: path.edge,
                gap,
                enter: from_row,
                leave: to_row,
                from: shared(path.slot(gap)),
                to: shared(path.slot(gap + 1)),
                ink: colours.get(path.edge.index()).copied().flatten(),
            });
        }
    }
    runs
}

/// How wide each gap has to be to hold its captions and its tracks.
fn gaps(tracks: &Tracks, captions: &Captions, columns: usize, style: Style) -> Vec<i32> {
    (0..columns.saturating_sub(1))
        .map(|gap| {
            let needed = i32::try_from(tracks.count(gap) + CLEARANCE).unwrap_or(style.gap);
            (needed + captions.room(gap)).max(style.gap)
        })
        .collect()
}

/// The left edge of each column.
fn lefts(widths: &[i32], gaps: &[i32], style: Style) -> Vec<i32> {
    let mut x = 0;
    widths
        .iter()
        .enumerate()
        .map(|(column, w)| {
            let at = x;
            x += w + gaps.get(column).copied().unwrap_or(style.gap);
            at
        })
        .collect()
}

fn boxes(columns: &Columns, placed: &Placed, widths: &[i32], lefts: &[i32]) -> Vec<Boxed> {
    let mut boxes = Vec::new();
    for (column, slots) in columns.iter().enumerate() {
        for (at, slot) in slots.iter().enumerate() {
            let Slot::Node(node) = slot else { continue };
            boxes.push(Boxed {
                node: *node,
                column,
                x: lefts.get(column).copied().unwrap_or(0),
                y: placed.top(column, at),
                w: widths.get(column).copied().unwrap_or(MIN_WIDTH),
                h: placed.height_of(column, at),
            });
        }
    }
    boxes
}

/// Joins the rows of a path, turning on the track each of its runs was given.
fn polyline(
    path: &Path,
    runs: &[Run],
    tracks: &Tracks,
    captions: &Captions,
    lefts: &[i32],
    widths: &[i32],
) -> Option<Vec<(i32, i32)>> {
    let last = path.first + path.rows.len() - 1;
    let exit = lefts
        .get(path.first)?
        .checked_add(*widths.get(path.first)?)?;
    let entry = lefts.get(last)?.checked_sub(1)?;

    let mut points = vec![(exit, *path.rows.first()?)];
    for (step, pair) in path.rows.windows(2).enumerate() {
        let [from_row, to_row] = *pair else { continue };
        if from_row == to_row {
            continue;
        }
        let gap = path.first + step;
        let track = runs
            .iter()
            .position(|run| run.edge == path.edge && run.gap == gap)
            .map_or(0, |run| tracks.of(run));
        let x = track_x(lefts, widths, gap, track) + captions.room(gap);
        points.push((x, from_row));
        points.push((x, to_row));
    }
    points.push((entry, *path.rows.last()?));
    Some(collapse(points))
}

/// Where one track of one gap sits: a cell clear of the box, then one per track.
fn track_x(lefts: &[i32], widths: &[i32], gap: usize, track: usize) -> i32 {
    let start =
        lefts.get(gap).copied().unwrap_or(0) + widths.get(gap).copied().unwrap_or(MIN_WIDTH);
    start + 1 + i32::try_from(track).unwrap_or(0)
}

/// Drops the points that do not turn.
fn collapse(points: Vec<(i32, i32)>) -> Vec<(i32, i32)> {
    let mut kept: Vec<(i32, i32)> = Vec::with_capacity(points.len());
    for point in points {
        if kept.last() == Some(&point) {
            continue;
        }
        if let [.., before, middle] = kept.as_slice() {
            let straight = (before.0 == middle.0 && middle.0 == point.0)
                || (before.1 == middle.1 && middle.1 == point.1);
            if straight {
                kept.pop();
            }
        }
        kept.push(point);
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collinear_points_are_dropped() {
        assert_eq!(collapse(vec![(0, 0), (3, 0), (7, 0)]), [(0, 0), (7, 0)]);
        assert_eq!(collapse(vec![(0, 0), (0, 4), (0, 9)]), [(0, 0), (0, 9)]);
    }

    #[test]
    fn a_repeated_point_is_dropped() {
        assert_eq!(collapse(vec![(0, 0), (0, 0), (5, 0)]), [(0, 0), (5, 0)]);
    }

    #[test]
    fn a_gap_is_never_narrower_than_the_minimum() {
        let none = Captions {
            text: Vec::new(),
            per_gap: Vec::new(),
        };
        assert_eq!(
            gaps(
                &Tracks::default(),
                &none,
                3,
                Style::natural(Options::default())
            ),
            [MIN_GAP, MIN_GAP]
        );
    }

    #[test]
    fn a_caption_widens_the_gap_it_sits_in() {
        let captions = Captions {
            text: Vec::new(),
            per_gap: vec![9, 0],
        };
        let widths = gaps(
            &Tracks::default(),
            &captions,
            3,
            Style::natural(Options::default()),
        );
        // No tracks here, so the gap is the clearance plus the caption, which
        // is already past the minimum.
        assert_eq!(widths[0], i32::try_from(CLEARANCE).unwrap() + 9);
        assert_eq!(widths[1], MIN_GAP);
    }

    #[test]
    fn a_track_sits_clear_of_the_box_it_leaves() {
        let (widths, gaps) = (vec![6, 6], vec![5]);
        let lefts = lefts(&widths, &gaps, Style::natural(Options::default()));
        assert_eq!(lefts, [0, 11]);
        // The box occupies 0..5, so 6 is the first free cell and the first
        // track is one clear of that.
        assert_eq!(track_x(&lefts, &widths, 0, 0), 7);
        assert_eq!(track_x(&lefts, &widths, 0, 1), 8);
    }
}
