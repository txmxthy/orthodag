# Build plan

This document records the implementation sequence and acceptance criteria for
the library described in [the design](design.md). Implementation has reached the
documentation stage; Cargo publication is still disabled with `publish = false`.
Ongoing layout work follows [the quality guide](quality.md).

## Implementation order

Building the layout pipeline strictly in phase order would delay visible output
until routing and painting were finished. The plan brings the painter forward
to step four so that subsequent changes can be reviewed as drawings.

Basic routing comes before track packing because it exposes the collisions
that packing must resolve. The scorer comes before track ordering so that the
search has an objective. Ports and colour follow the initial track work, adding
the row allocation needed to keep flows distinguishable.

Each milestone requires passing tests and lints. The README should describe the
features available at that point.

## Milestones

| # | Deliverable | Acceptance criteria |
|---|---|---|
| 1 | Graph model, cycle removal and layer assignment | A chain has ranks. Back edges are identified and excluded from forward layout. |
| 2 | Dummy vertices and barycentre ordering sweeps | A fan-out produces distinct candidate orderings, reproducibly. |
| 3 | Coordinates and orthogonal routes | Every edge has a polyline in cell coordinates. Graphs with crossings can be laid out without panicking. |
| 4 | Painter and canvas | A chain and a fan render as box-drawing text, covered by snapshots. |
| 5 | Scorer and quality checks | Every committed fixture has zero vocabulary defects, and tests detect score regressions. |
| 6 | Track packing, ordering and fan nesting | A fan can share a trunk with a branch per target. Soft scores improve without subsequent regressions. |
| 7 | Ports and colour | Each distinct tag set has an attach row, and boxes grow to accommodate them. |
| 8 | Merging, back-edge lanes, crossing styles and fitting | Width fitting reduces drawings where possible. Merging shared paths reduces `ink`. |
| 9 | Import, export and review gallery | Mermaid round-trips, and all fixtures can be reviewed on one HTML page. |
| 10 | Documentation, examples and publication | The README gives a new user enough information to add the crate and draw a graph. |

The first three steps were kept small to reach visible output quickly.
Completion of the milestones does not establish layout quality on larger or
more varied graphs; that requires continued corpus review.

## Tests

The test strategy combines snapshots, invariants and score comparisons. The
core suite runs with `cargo test`; `cargo test --all-features` also exercises
optional features. The tests do not require network access.

### Snapshots

Fixtures are built in code, rendered and compared with stored frames. A changed
character produces a diff for human review. Snapshot updates are accepted
individually so that a bulk update cannot hide an unwanted layout change.

### Invariants

These checks cover properties that should hold without a
stored frame: deterministic output, one route per edge, correct route
attachments and colour assignment consistent with tags. Search work is limited
by budgets based on graph size, with a test for the candidate-count limit.

### Score regression checks

Fixtures in the committed `CLEAN` list must retain zero vocabulary defects.
A separate local test compares scores with a baseline under `target/quality`.
It rejects any increase in vocabulary defects and limits regressions in other
fields. The aggregate soft score must not increase.

The baseline is recorded after a change has been reviewed and accepted. It is
not committed, and the baseline test skips when the file is absent. See
[the quality guide](quality.md#regression-checks) for commands and tolerances.

## Corpus

The committed fixtures are small synthetic graphs covering a chain, fan-out,
fan-in, diamond, skip, cycle, wide layer, ladder and shared target with different
tag sets. Names identify the affected shape in reviews and commit messages.
A seeded generator adds larger and denser graphs.

An optional private directory provides graphs from actual use that cannot be
published. Tests and tools that use it skip that input when it is absent. The
public suite remains runnable without it, although it then covers fewer graphs.
CI does not receive the private corpus.

## Review gallery

The gallery renders each graph in several configurations on a static HTML page.
It supports sorting by name or score and provides space for notes beside each
frame. This makes it possible to inspect changes that a numerical score does
not adequately describe.

Notes are stored in the browser. The gallery runs locally and includes whatever
corpus is present on the machine, so its output may contain private data and
should not be published. It is a review tool outside the library and CI.

## Completion requirements

- The same graph and options produce identical bytes.
- Search stages have explicit budgets based on graph size.
- Clippy rejects `unwrap`, `expect` and explicit panics outside tests.
- Changes to rendered fixtures receive visual review.

## Deferred work

Interactivity, hit testing, animation and terminal-library integration remain
outside the library's scope.

Solver-based layer assignment and a general routing constraint system were also
deferred. Either would need evidence of a layout improvement large enough to
justify its complexity and runtime cost.
