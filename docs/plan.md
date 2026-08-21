# orthodag — the build plan

[`design.md`](design.md) says what the library is. This page says how it gets
built: in what order, what has to be true before each step counts as done, and
what the tests look like that decide.

The plan was written before the code, for the same reason the design document
was written first: to decide the build order in advance.

## Implementation order

A layered drawing is built as a pipeline of phases: model, ranks, orderings,
coordinates, routes, characters. Building them in that order, front to back,
delays all visible output until the last phase lands, so five phases would be
written before there is any feedback.

The painter is moved earlier instead, to step four of ten, before anything
that needs visual judgement. Every step after it changes a drawing that can be
looked at, diffed and scored. The remaining steps are ordered by what each one
depends on: routing before tracks, because tracks only matter once routes
collide; the scorer before track ordering, because track ordering is a search
and a search needs an objective; colour after tracks, because a colour needs a
row of its own, and rows are what tracks allocate.

Each step ends with the test suite passing, the lints clean, and a README that
describes only what actually works.

## Milestones

| # | delivers | done when |
|---|---|---|
| 1 | The graph model, cycle removal, layer assignment. | A chain has ranks. Back edges are identified and removed. |
| 2 | Dummy vertices, barycenter sweeps, candidate orderings. | A fan-out produces several distinct orderings; the same graph produces the same ones every run. |
| 3 | Coordinates and orthogonal routes. | Every edge is a polyline in cell coordinates. Crossings exist and nothing panics. |
| 4 | The painter: direction bits, the glyph table, the canvas. | A chain and a fan draw as box-drawing text. First snapshots land. |
| 5 | The scorer and the quality gate. | Every committed fixture is at zero on the vocabulary tier, and a regression in the numbers fails a test. |
| 6 | Tracks: packing, ordering, fan nesting. | A fan reads as one trunk with a branch per target. The soft tier falls and stays down. |
| 7 | Ports and colour. | No two tag sets share an attach row. Boxes grow to hold the rows they need. |
| 8 | Merging, back-edge lanes, crossing styles, fitting. | A drawing fits a target width. Two edges drawn as one line reduce `ink`. |
| 9 | Import, export, and the review gallery. | Mermaid round-trips. Every fixture renders onto one HTML page. |
| 10 | Documentation, examples, publication. | A stranger can add the crate and draw a graph from the README alone. |

All ten milestones are complete. Whether the drawings are actually good is a
separate question, covered in `docs/quality.md`.

Steps 1–3 produce no output a human would look at, so they are kept short:
their purpose is to reach step 4.

## Tests

The tests run in three layers, ordered from cheapest to most expensive. All
three run with `cargo test`, without configuration or network access.

**Goldens.** Graphs are built in code, rendered, and compared against a stored
frame. The drawings serve as the specification, so any change that moves a
character produces a diff for a human to read and accept by hand. There is no
switch to accept every diff at once, because an accepted change that nobody
read would defeat the purpose of the check.

**Invariants.** These are properties that must hold for every graph in the
corpus without a stored expectation: the layout is byte-identical when run
twice, every edge has exactly one route, a route starts and ends on its boxes,
an edge is coloured if and only if it carries tags, and the whole thing
finishes inside a time budget expressed in edges.

**The ratchet.** Every fixture is scored and the numbers compared against a
stored baseline. The vocabulary tier — the categorical defects from
[`design.md` §6](design.md) — may never rise for any graph, and zero is
absorbing. The soft tier may rise slightly for one graph if the total across
all of them falls. The baseline is written by hand, after a change has been
accepted, and never adjusted to make a change pass.

## Corpus

There are two sets: one committed and one not.

The committed set is synthetic: hand-written graphs, one for each shape that
needs its own test — a chain, a fan-out, a fan-in, a diamond, a skip, a cycle,
a wide layer, a graph where two tag sets share a target — plus a seeded
generator for breadth. They are small enough to read, and named so a commit
message can identify which one moved.

The second set is a private directory that the tests look for and skip when
it is absent. It exists because the library can only be judged honestly
against graphs someone actually uses, and those graphs cannot be published.
Every test that depends on it degrades to a skip, so the suite runs the same
with or without it, and CI never receives it.

## Review gallery

The scorer reduces a drawing to a single number, and a number cannot show
everything a drawing can be checked for by eye.

There is one more tool, outside the library and outside CI: a single static
HTML page holding every fixture rendered at every width and option worth
comparing, one tab at a time, sorted by name or by score. Its purpose is to
be looked at directly. A local helper stores notes against each frame and
clears them when the frame is regenerated, so notes do not carry over between
rounds of review.

The gallery runs locally only, and renders whatever corpus is on the machine.

## Completion requirements

These come from [`design.md` §8](design.md), restated as what a test asserts:

- **Deterministic.** The same graph and options produce the same bytes.
- **Budgeted.** Every stage that scales with graph size has an explicit budget.
- **No panics.** No `unwrap` or `expect` outside tests; the lint denies them.
- **Snapshot-tested.** Every change that moves a character is read by a human.

## Deferred work

Interactivity, hit testing, animation, and any dependency on a terminal
library are out of scope in the design and out of scope here. A solver-based
layer assignment and a general constraint system for routing are also
excluded, though both are tempting. Both may become the right answer
eventually, but only once there are numbers showing what they would buy.
