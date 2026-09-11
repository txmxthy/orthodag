# orthodag design

orthodag renders a directed graph as Unicode box-drawing text. Boxes stand
for the vertices, and orthogonal lines carry the edges, sized to fit a
requested number of columns.

This document is written before the code. That way a decision that turns out
badly leaves a visible record instead of disappearing into the implementation
as an accident.

## 1. Motivation

Terminal programs that show a graph have three options today, and each one
falls short.

Shelling out to Graphviz lays out a graph beautifully, but it emits an
image. A terminal cannot show an image, the binary becomes a runtime
dependency, and the process boundary makes it awkward to give one edge a
different colour from another.

A layered-layout crate stops where the published algorithms stop, at
coordinates. Turning coordinates into characters is left to the caller, and
most of the difficulty of drawing on a character grid lives in that last
step.

Drawing a graph by hand works for a chain, but it becomes hopeless once
anything fans out.

orthodag goes all the way to characters. It treats the character grid as the
actual target of the layout and holds an opinion about what a good drawing
looks like.

The motivating case is a terminal UI showing data pipelines: a few dozen
vertices, heavy fan-out, tagged edges that need to be told apart by colour,
and occasional cycles. The graph redraws on every frame, so the layout has
to look deliberate as well as correct.

## 2. Scope

orthodag provides a graph model, layered layout, orthogonal routing and a
painter that turns routes into box-drawing characters. It fits a drawing to
a target width and scores how good a drawing is. An edge carries a palette
slot, and the output can be handed back as styled runs so the caller decides
what a slot looks like.

Undirected graphs, hypergraphs, nested clusters and ports on a named side are
outside the scope. Interactivity, hit testing, animation and pixel rendering
are left to other tools.

There is no dependency on a terminal library. The output is text and spans,
and it is the caller's choice whether those become escape codes, HTML or a
widget buffer.

## 3. The model

```rust
pub struct Graph { nodes: Vec<Node>, edges: Vec<Edge> }

pub struct Node {
    pub label: String,      // the headline, drawn bold
    pub lines: Vec<String>, // extra lines inside the box
}

pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub tags: Vec<String>,  // edges sharing a tag set are one logical line
}
```

Ids are newtypes over `u32` and index the two vectors. The graph is built by
the caller, and the library never parses anything.

Tags do a lot of work. Two edges carrying the same tags and ending at the
same vertex read as the same line to a reader, and the layout draws them as
one: where both edges are long enough to need placeholder rows, the pair
shares a single row instead of one each, so the merge is literal and they
arrive along a single trunk.

An untagged edge is not part of any flow. Two untagged edges arriving at
one vertex are two separate lines that happen to share a door, and nothing
in the graph declares them related, so each keeps its own row. Tags also
key the palette, so a logical flow keeps one colour across the whole
drawing.

## 4. The pipeline

The pipeline runs five classical phases, plus three more that the classical
presentation leaves to the renderer and that this library has to own.

### 4.1 Cycle removal

Layering needs an acyclic graph. A depth-first traversal colours vertices
white, grey and black, and an edge pointing at a grey vertex is a back edge.

Back edges are removed. Reversing one would point its arrow the wrong way in
the terminal, and that reads worse than the layout cost of taking the edge
out of the optimiser. A removed edge gets its own treatment: a lane row
under the boxes, one lane per source, with the shortest hops placed nearest
the graph.

### 4.2 Layer assignment

Layer assignment uses longest path in Kahn order: every vertex sits one
column past its latest predecessor. The pass runs once and stays tight,
which maximises the number of long edges. A vertex with one early
predecessor and one late predecessor hugs the early one, so the edge from
the late predecessor stretches across the drawing.

That stretch is a known weakness. Network simplex would pull such a vertex
to the right and remove the long edge. The plan is to start with longest
path, measure its effect, and revisit later, since the phase boundary keeps
a fancier ranking algorithm a drop-in replacement.

### 4.3 Crossing minimisation

An edge spanning several columns is split into single-hop segments with
dummy vertices between them, so crossings decompose into independent
adjacent-layer problems.

The ordering pass tries both a barycentre sweep and a median sweep from the
initial order. A barycentre is pulled off course by one neighbour far down
the column; a median cannot tell how far away a neighbour sits, so it treats
a small gap and a large one the same. Neither sweep wins on every graph.
Both use an exact rational key rather than a float, so ties break the same
way on every run.

Candidates are judged by drawing them: each distinct ordering a sweep
produces is laid out, painted in full and scored on the cell grid, and the
best-scoring drawing is kept. This is far more expensive than counting
inversions in the layered graph, but it is the only way to see a crossing
that a box hides, or two runs that merge into one apparent line. Because the
cost is real, the search budget scales with graph size.

### 4.4 Coordinates

Each vertex gets a row. Alternating median sweeps put a box on the median
row of its neighbours, and each column is then compacted back into the
height of the tallest column, moving each box as little as possible.

A long edge occupies one row for every column it passes through, so a dummy
chain cannot bend. Where the published algorithms keep dummy chains straight
through a separate alignment pass, which can still fail, this pipeline makes
straightness structural instead: a dummy chain simply has no way to bend.
The cost is that a dummy has no row of its own, which rules out adding an
independent alignment pass on top later.

The median sweep only ever pushes a box down, and compaction only pulls one
up when its column would not otherwise fit, so neither step can recentre a
drawing on its own. A budgeted local search, run as coordinate descent,
corrects this: each column is offered a shift of a row or two in either
direction, and a shift is kept only when the resulting drawing scores
better. Each candidate is judged by drawing it rather than by a cheap
geometric proxy, since a proxy could not see a crossing that a box hides.

### 4.5 Track packing and routing

This is a phase the classical presentation does not have.

In the gap between two columns, every edge that changes row needs a
vertical run. Runs are packed onto tracks, meaning vertical channels one
column wide, by first-fit packing over their row intervals. Two runs may
share a track when they meet at one end and carry the same colour, since a
fan out of one box should read as a single trunk with a branch to each
target instead of a comb of parallel lines.

Both conditions matter. Meeting at one end makes two runs eligible to merge
into a single line; carrying the same colour is what actually merges them.
Two different flows placed on one trunk become a line the reader cannot
separate again, the same failure an overlap causes, and the scorer counts
it as `blends`. Given a choice between a comb of separate runs and a trunk
that has swallowed a flow, the comb is the better drawing: it is less tidy
to look at, but every line in it can still be followed.

Tracks are then ordered left to right by an exact search over permutations
when the gap is small. The search first minimises horizontal segments
landing on another run's corner row, since that would read as one
continuous line, and only then minimises crossings.

The outermost branch of a fan turns first, so a fan nests inside itself
instead of interleaving with its own branches.

### 4.6 Ports

Where a box has several edges, each leaves on a different row of its right
edge, one row per tag set, and arrives the same way on the left edge of the
next box. A box grows tall enough that no two tag sets ever have to share a
row, because a cell holds one colour, and sharing a row would lose one of
them.

Branches are ordered by the row each one turns toward, which for a long edge
is the row it runs across on rather than the box it eventually reaches.
Ordering a fan by where its branches end instead, when a branch detours
somewhere else first, is exactly what makes a fan cross itself.

### 4.7 Painting

Layout produces orthogonal polylines in cell coordinates. The painter walks
each polyline and sets four direction bits per cell, one bit for each
direction a line leaves that cell, and ORs them into whatever bits are
already there. The glyph shown is a pure function of those bits: eleven
box-drawing characters plus a space.

Corners and junctions are never drawn on purpose; they simply appear. A
corner is whatever a horizontal run and a vertical run leave behind where
they meet, and a cross is whatever four runs leave behind. Routes can
therefore be painted in any order, and a fork and a crossing take the same
code path.

Two runs of different colours landing in one cell is a defect that the
layout must avoid, since a cell holds one character and therefore one
colour, and the painter has no way to fix that afterward. Where a genuine
crossing makes an overlap unavoidable, the horizontal run breaks for one
cell on either side so the vertical run reads as passing over it.

This break is the default behaviour, and ink decides when it applies rather
than colour on its own. A junction glyph asserts that two runs are one
line, which is true when they share an ink and false, invisibly, when they
do not, since the cell can only show one of the two. An untagged edge
counts as an ink of its own, drawn in whichever default colour the caller
uses, so a coloured run crossing a default one loses exactly as much as two
differently coloured runs would. A crossing between two flows therefore
breaks; a crossing within one flow keeps its `┼`. A caller who wants one
behaviour or the other at every crossing can still request it.

### 4.8 Width fitting

A drawing has a natural width. When the caller asks for something narrower,
box widths and gaps shrink through a fixed ladder of steps down to a
legibility floor, and the narrowest result found is kept even when nothing
in the ladder reaches the requested target.

## 5. Drawing rules

The scorer exists to measure these rules:

- Between neighbouring columns, an edge is either straight, an L shape with
  one bend, or a Z or S shape with two bends; nothing else is allowed there.
  An edge that skips columns may bend at each end, for four bends at most
  whatever the skip, and a back edge through a lane gets the same four-bend
  allowance.
- An edge touches at most one junction at each end: its fork on the source
  trunk and its join on the target trunk.
- Two unrelated edges may share a cell only as a crossing, one running
  straight through horizontally and the other straight through vertically.
  Anything else shared between unrelated edges is an overlap, and the scorer
  treats an overlap as a defect, since it draws two edges as one line and
  misleads the reader.
- Fan-outs and fan-ins are symmetric about the trunk row.
- There is one arrowhead per target per colour.

## 6. Scoring

The objective is organised into three tiers.

The vocabulary tier is hard: bends over budget, edges with more than one
fork or join run, and overlaps. These are categorical, so a glyph that reads
wrong counts as broken regardless of how small it looks, and a candidate
that raises any of these fields is refused whatever happens to its total.

The total tier is the scalar the search actually descends, a weighted sum of
crossings, fan asymmetry, and detour, meaning vertical travel beyond the row
difference the edge actually has to cover.

The reported tier is computed for diagnosis and deliberately left out of the
objective:

| metric | counts |
|---|---|
| `ink` | cells carrying any edge glyph |
| `mixed` | cells where two colours meet and the painter has to pick one |
| `jogs` | bends beyond the fewest an edge could have |
| `scatter` | blank rows between parallel runs crossing a column |

`ink` deserves a note. Every other metric is counted per edge, so when two
edges are drawn as one line, each pays for the whole path and a merge scores
as a regression. `ink` counts drawn cells instead, so a merge makes it go
down, which is why a merge should be judged by `ink`.

Crossings are counted in two senses that should not be confused: one charges
every pair of edges sharing a cell, and the other counts the cells
themselves. Two edges drawn as one line inflate the pair count while leaving
the cell count alone. The objective uses the cell count.

## 7. Public surface

```rust
pub fn layout(graph: &Graph, opts: Options) -> Layout;
pub fn draw(layout: &Layout) -> Canvas;
pub fn score(graph: &Graph, layout: &Layout) -> Score;
```

`Layout` holds boxes, gaps, tracks and polylines in cell coordinates, with
no character in it anywhere. `Canvas` is the painted grid: `to_string()`
returns plain text, and `runs()` returns a per-line list of styled spans,
each carrying the edge it belongs to, so a caller can colour one edge
without the library needing to know what a colour is.

`Options` carries the target width, how parallel runs are bundled, and how
crossings are drawn.

## 8. Quality requirements

The same graph and options must produce the same bytes every time. Any
choice of first, nearest or median taken over a hash map sorts its
candidates first, and tests assert this.

The search is expensive by design, so every stage that scales with graph
size carries an explicit budget expressed in edges.

Clippy denies `unwrap` and `expect` outside tests, and the code contains no
explicit panics either.

The drawings themselves are the specification: snapshot tests record them,
and any change that moves a character shows up as a diff for a human to
read.

## 9. Licence

The crate is offered under MIT or Apache-2.0, at the user's option. This is
the Rust convention, and it keeps the crate usable everywhere.

## 10. Milestones

The work starts with the graph model, ranks and layers, enough for a chain
to draw. Dummies and orderings follow, enough for a fan to draw, and then
coordinates and routing, at which point crossings appear and become
survivable. The painter comes next, producing real box-drawing output,
followed by the scorer and orderings that get judged by drawing them.

Later stages add tracks, meaning buses, exact ordering and nesting, then
colour, meaning one row per tag set and boxes grown tall enough to hold
them, then merging, bridges, fitting and labels. Documentation and examples
come last, ahead of publishing.
