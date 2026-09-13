# orthodag design

orthodag renders a directed graph as Unicode box-drawing text. Nodes appear in
boxes, and edges follow horizontal and vertical paths between them. The caller
can request a width and receive either plain text or styled spans.

These notes describe the layout decisions and the constraints of drawing on a
character grid. The implementation sequence is recorded in [the build
plan](plan.md).

## 1. Motivation

The intended use is a terminal UI displaying data pipelines with a few dozen
nodes. These graphs often have substantial fan-out, tagged edges that need
distinct colours, and occasional cycles. Layout must be fast enough for repeated
redraws and clear enough to follow individual paths.

A coordinate-based layout engine solves only part of this problem. On a
character grid, two routes can occupy the same cell, and a junction glyph can
make unrelated edges appear connected. Box sizes, attach rows and gaps all
affect whether the result is readable. This library handles those decisions
along with layout, so callers do not need to implement a renderer themselves.

## 2. Scope

The library provides a graph model, layered layout, orthogonal routing and a
Unicode painter. It supports tagged edges, colour slots, width fitting and
scoring of completed drawings. Mermaid import and export are available through
the optional I/O support.

Undirected graphs, hypergraphs, nested clusters and named ports are outside the
scope. The library also leaves interactivity, hit testing, animation and pixel
rendering to other tools.

There is no dependency on a terminal library. Callers decide how styled spans
become terminal escape sequences, HTML or widget content.

## 3. Graph model

`Graph` stores vectors of nodes and edges. Each node has a label and optional
additional lines of text. Each edge has a source, a target and a set of tags.
Callers build graphs with `add_node`, `add_edge` and `add_tagged_edge`; the core
model does not require a parser.

`NodeId` and `EdgeId` wrap `u32` indices into those vectors. Their limitations are
documented in [ADR 1](adr/0001-ids-are-indices.md).

Tags identify logical flows and determine palette slots. When tagged edges have
the same tag set and target, their long sections can share placeholder rows and
arrive along a common trunk. Untagged edges retain separate placeholder rows:
an empty tag set does not establish that they belong to one flow.

Sharing tags alone does not make every intersection a junction. Two edges with
the same tags can be unrelated elsewhere in the graph. The painter's default
crossing style also considers whether they share an endpoint.

## 4. Layout pipeline

The pipeline assigns columns, orders nodes within them, chooses rows and routes
edges. Additional passes handle ports, painting and width fitting.

### 4.1 Cycle removal

A depth-first traversal marks vertices as unvisited, active or finished. An
edge to an active vertex is a back edge and is excluded from forward layout.

Back edges retain their original direction. They are routed below the boxes,
with one lane per source and shorter hops placed nearer the graph. This keeps
cycles visible without reversing arrows to satisfy the ranking algorithm.

### 4.2 Layer assignment

Nodes are processed in Kahn topological order. A node's rank is one greater
than the maximum rank of its forward predecessors; nodes without predecessors
start at zero. Ranks become columns in the drawing.

This assignment puts each node as early as its dependencies permit. It does
not minimise total edge length. If a node has both an early and a late
predecessor, it sits after the late predecessor, leaving a long edge from the
early one. A more expensive rank optimiser could reduce edge length where the
graph allows nodes to move. The phase boundary allows that change without
replacing the rest of the pipeline.

### 4.3 Crossing minimisation

Long edges are split into adjacent-column segments using dummy vertices. This
allows ordering sweeps to compare neighbours one column at a time.

The ordering pass tries both barycentre and median sweeps from the initial
order. Barycentres account for every neighbour's position but are sensitive to
distant neighbours; medians respond differently to uneven distributions. Both
use exact rational keys to keep comparisons and tie-breaking deterministic.

Each distinct candidate ordering is placed, routed and scored on the cell grid.
Counting inversions alone would miss effects such as a box hiding a crossing
or two routes merging into one visible line. Evaluating drawings is more
expensive, so the number of candidates is limited according to graph size.

### 4.4 Coordinates

Alternating median sweeps assign rows using neighbouring positions. Each column
is then compacted to fit within the height of the tallest column, while moving
its boxes as little as possible.

All dummy vertices in a long-edge chain use one shared row. The middle section
therefore stays straight without a separate alignment pass. This representation
also means an individual dummy cannot move independently of its chain.

The sweeps and compaction can leave columns off-centre. A budgeted coordinate
descent tries shifting each column by a row or two in either direction. It
keeps a shift when the resulting drawing has a better score. As with ordering,
the evaluation includes routing and cell occupancy.

### 4.5 Track packing and routing

An edge that changes row between columns needs a vertical segment in the gap.
The router assigns these segments to one-cell-wide tracks using first-fit
interval packing.

Segments may share a track when they meet at an endpoint and carry the same
colour. This lets a fan use a common trunk with branches to its targets.
Combining different colours would make separate flows appear to be one line;
the scorer reports such cells as `blends`.

For small gaps, an exact permutation search chooses the track order. Its first
priority is avoiding horizontal segments that land on another segment's corner
row, where the paths would appear joined. It then minimises crossings. Fan
branches are arranged so that the outermost branch turns first, producing
nested routes.

### 4.6 Ports

Edges attach to the right or left side of a box on rows assigned by tag set.
Boxes grow tall enough to give each distinct tag set its own row. This avoids
forcing two colours into a single cell at an attachment.

Branches are ordered by the row of their first turn. For a long edge, this is
the row of its middle section. Ordering only by the final target would allow a
detouring branch to cross its siblings near the source.

### 4.7 Painting

The painter receives boxes and orthogonal polylines in cell coordinates. It
walks each route and accumulates four direction bits per cell. A lookup table
maps those bits to eleven box-drawing characters or a space.

Combining bits produces corners and junctions automatically, and the bit grid
is independent of route order. Box borders, text and arrowheads are applied as
a separate overlay. See [the painter documentation](painter.md) for the table
and rendering details.

A cell can display only one colour. At a crossing between distinct flows, the
default style breaks the horizontal line on either side of the vertical one.
This helps readers follow the vertical line through the intersection. A shared
colour and endpoint identify a junction within a flow, which keeps its junction
glyph. Untagged edges are treated as having distinct inks for this decision.
Callers can also request junction glyphs or bridges at every crossing.

### 4.8 Width fitting

When the natural drawing is wider than requested, layout tries a fixed sequence
of smaller box widths and gaps, stopping at a legibility floor. If no attempt
fits, it returns the narrowest result. The requested width is therefore a target,
not a guarantee. Text within a reduced box may be shortened with an ellipsis.

## 5. Drawing rules

The scorer measures compliance with these rules:

- Between adjacent columns, an edge may be straight or have one or two bends.
  An edge that skips columns may bend at both ends, for a maximum of four bends.
  A back edge routed through a lane also has a four-bend allowance.
- An edge may touch at most one fork run at its source and one join run at its
  target.
- Unrelated edges may share a cell only when one passes horizontally through
  it and the other passes vertically. Other shared cells are overlaps, which
  can make separate edges appear connected.
- Fan-outs and fan-ins should be symmetric about their trunk row.
- There should be one arrowhead per target per colour.

## 6. Scoring

The objective distinguishes structural defects from costs that can be traded
against one another.

The vocabulary fields count excess bends, excess fork or join runs, and overlaps.
A search candidate that increases any of these fields is rejected, even if its
total score falls.

Among acceptable candidates, the search minimises a weighted total. Crossings,
fan asymmetry and unnecessary vertical travel contribute to that total. The
formula and the local regression checks are documented in
[the quality guide](quality.md).

Other measurements help diagnose a drawing without contributing to the search
objective:

| Metric | Meaning |
|---|---|
| `ink` | Cells carrying edge glyphs |
| `mixed` | Cells in which colours conflict |
| `blends` | Fork or join cells carrying different colours |
| `jogs` | Bends beyond the minimum an edge could use |
| `width`, `height` | Drawing dimensions |

`ink` is useful when evaluating bundled edges. Per-edge measurements can count
a shared section once for each route, while `ink` counts its occupied cells
only once.

Crossings can likewise be counted by edge pair or by occupied cell. Several
edges sharing a path can inflate the pair count without adding visible
crossings. The objective uses the cell count.

## 7. Public API

The main entry points accept a graph and return text, spans or a score:

```rust
pub fn draw(graph: &Graph) -> String;
pub fn spans(graph: &Graph) -> Vec<Vec<Span>>;
pub fn score(graph: &Graph) -> Score;
```

Each has a `_with` variant accepting `Options`. Options control edge labels,
crossing style and target width.

A `Span` identifies both its palette slot and its part of the drawing. This
lets a caller style box borders differently from untagged edges even though
neither has a palette slot. The caller chooses the actual colours.

The internal `Layout` and `Canvas` types remain private. Callers can supply a
`Drawing` to `score_drawing` or `draw_drawing` when they already have box
positions and routes, so a drawing from any source is held to the same objective.

## 8. Quality requirements

The same graph and options must produce identical output. Iteration order and
tie-breaking must be deterministic wherever they affect the drawing.

Search stages have explicit budgets based on graph size. Performance work
should use measured phase costs and drawing counts to identify useful changes.

Clippy denies `unwrap`, `expect` and explicit panics outside tests. These rules
reduce avoidable failures; they are not a proof that every possible input is
panic-free.

Snapshots record the rendered fixtures. A change that moves characters requires
review of the resulting drawing as well as its score.

## 9. Licence

The crate is offered under MIT or Apache-2.0, at the user's option.

## 10. Implementation sequence

The work began with the graph model and ranking, followed by dummy vertices,
ordering, coordinates and basic routing. The painter then made it possible to
review actual drawings before adding the scorer and more elaborate searches.
Later stages added track ordering, colour-aware ports, merging, bridges, width
fitting and I/O. The [build plan](plan.md) records the acceptance criteria for
each stage.
