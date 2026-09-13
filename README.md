# orthodag

orthodag renders a directed graph as Unicode box-drawing text.

```
                        ┌────────┐
                    ┌──▶│ reduce │─┐
┌─────┐     ┌─────┐ │   └────────┘ │   ┌──────┐
│ src │────▶│ map │─┘              └──▶│ sink │
└─────┘     │     │─┐              ┌──▶│      │
            └─────┘ └──────────────┘   └──────┘
```

Vertices become boxes, and edges become orthogonal lines that fit a target width,
against an explicit objective for what a good drawing looks like.

The default build carries no dependencies, and there is never a dependency on a
terminal library. The output is text and styled spans, and whether those become
escape codes, HTML or a widget buffer is the caller's business.

> **Status: 0.1.0, early.** Everything documented here works and is tested,
> though whether the drawings are *good* is a separate question, asked and
> answered in [docs/quality.md](docs/quality.md): the small graphs are provably
> optimal, and the large ones are not yet.

## Motivation

The intended use is a terminal tool that displays a data pipeline: a few dozen
vertices, heavy fan-out, and edges tagged by condition. Existing options for
drawing such a graph fall into three groups.

Shelling out to Graphviz lays out beautifully and emits an image. A terminal
cannot show an image, the binary becomes a runtime dependency, and the process
boundary makes it awkward to colour one edge differently from another.

A layered-layout crate is sound as far as it goes, but it stops where the
literature stops, at coordinates. Turning coordinates into characters is left
to the caller, and on a character grid that last step is where most of the
difficulty lives.

Of these, a text renderer that goes all the way to characters comes closest. It
draws boxes and arrows correctly, then falls down on everything that happens
*between* them, which on a real pipeline is the entire drawing:

- **Ports.** Six edges leaving one box all left from the same row, so six flows
  became one line at the source, and the colours were lost before they started.
- **Routing.** Edges doubled back on themselves, or turned early and turned
  again, when a single bend would have done.
- **Crossings.** Two lines meeting drew a junction glyph, which says *these are
  one line*, so following a line through a dense drawing became guesswork.
- **Merging.** Three shards feeding one sink drew three parallel lines the
  width of the drawing, when they are one logical flow that should read as one.

None of this is a criticism of those tools, which mostly aim at smaller
diagrams. But for pipelines at this scale, the gap between a picture of a graph
and a picture that can be read was the whole problem, so this library exists to
close it.

## Approach

**One attach row per flow.** Edges are tagged, and edges carrying the same tags
are one logical flow. Each flow gets a row of its own on the box's edge, and the
box grows tall enough that no two flows share one, because a cell holds one
character and therefore one colour, and a shared row would lose a flow.

```
                             ┌────────────┐
                         ┌──▶│ warm-cache │
                         │   └────────────┘
               ┌───────┐ │
┌────────┐     │ route │─┘   ┌────────────┐
│ ingest │────▶│       │────▶│ cold-store │
└────────┘     │       │─┐   └────────────┘
               └───────┘ │
                         │   ┌────────────┐
                         └──▶│ audit      │
                             └────────────┘
```

**One flow, one line.** Edges with the same tags arriving at the same box read
as one line to a reader, so they are drawn as one trunk rather than as parallel
runs:

```
┌─────────┐
│ shard-0 │─┐
└─────────┘ │
            │
┌─────────┐ │   ┌─────────┐
│ shard-1 │─┼──▶│ collect │
└─────────┘ │   └─────────┘
            │
┌─────────┐ │
│ shard-2 │─┘
└─────────┘
```

**A crossing looks like a crossing.** Where two different flows genuinely have
to pass, the horizontal breaks so the vertical reads as going over. A junction
glyph is kept only where it holds true: two branches of *one* flow, meeting at
a box they share:

```
                      ┌────┐
           ┌────┐ ┌──▶│ a2 │─┐
┌────┐ ┌──▶│ a1 │╴│┐  └────┘ │
│ b0 │╴│┐  └────┘ ││         │
└────┘ ││         ││  ┌────┐ │   ┌─────┐
       ││  ┌────┐ ├┴─▶│ b2 │─┼──▶│ end │
┌────┐ ├┴─▶│ b1 │─┘   └────┘ │   └─────┘
│ a0 │─┤   └────┘            │
└────┘ │                     │
       │                     │
       └─────────────────────┘
```

**A bounded vocabulary.** An edge is straight, an **L**, or a **Z**: two bends
between neighbouring columns, four across a skip, and nothing else is
permitted. This is enforced as a hard tier of the objective, so a candidate
drawing that breaks it is refused whatever else it improves.

## Start here

```toml
[dependencies]
orthodag = "0.1"
```

```rust
use orthodag::{Graph, Node};

let mut g = Graph::new();
let a = g.add_node(Node::new("read"));
let b = g.add_node(Node::new("parse"));
g.add_edge(a, b);

print!("{}", orthodag::draw(&g));
```

### Colour

The library does not decide what a colour *is*. It hands back runs of text
tagged with a palette slot and with whether the run is part of the drawing or a
line through it, and the caller decides what that looks like:

```rust
use orthodag::Part;

for row in orthodag::spans(&g) {
    for span in row {
        match (span.part, span.colour) {
            (Part::Flow, Some(slot)) => print!("\x1b[3{}m{}\x1b[0m", slot.slot() + 1, span.text),
            _ => print!("{}", span.text),
        }
    }
    println!();
}
```

Slots are keyed on the **tag set** that an edge carries, so one logical flow
keeps its colour across the whole drawing.

### Fitting to a width

```rust
use orthodag::Options;

print!("{}", orthodag::draw_with(&g, Options::new().width(80)));
```

A drawing has a natural width. Asking for less shrinks it through a fixed
ladder of steps down to a legibility floor, and nothing is ever clipped,
because half a box is worse than a wide one.

### Reading and writing other formats

| feature | what it adds |
|---|---|
| `mermaid` | read a graph from Mermaid flowchart source, and write one back |
| `dot` | write Graphviz DOT |
| `serde` | serialise the model and the score |

Off by default, so nobody pays for one they do not use.

## Scoring

Most of this library is a search, and a search needs an objective to descend.
The objective is written down, in three tiers ([docs/quality.md](docs/quality.md)):

**Vocabulary** — bends over budget, an edge touching more than one fork or join
run, two unrelated edges drawn as one line. These are categorical: a glyph
that reads wrong breaks the drawing regardless of its total score, and a
candidate raising one of these fields is refused whatever it does to the total.

**Total** — the scalar the search descends: crossings, asymmetry of fans, and
detour.

**Reported** — computed and deliberately excluded from the objective, because
they are diagnostics rather than goals: `ink`, `mixed`, `jogs`, `blends`.

Candidates are judged **by drawing them**. A sweep proposes an ordering; each
proposal is placed, routed, painted onto the cell grid and scored, and the best
drawing wins. That costs far more than counting inversions in the layered
graph, and it is the only way to see a crossing that a box hides or two runs
that merged into one apparent line.

Asking what is wrong with a drawing, not just how much, is also possible:

```
cargo run --features mermaid --example why -- graph.mmd
```

which prints the frame, the score, and then every defect together with the
pair of edges responsible and the phase that could have prevented it.

## Guarantees

- **Deterministic.** The same graph and options produce the same bytes: any
  first, nearest or median taken over a hash map sorts first, and tests assert
  this directly, using exact rational barycentres rather than floats for that
  reason.
- **No panics.** No `unwrap` or `expect` outside tests; clippy denies them.
- **Budgeted.** Every stage that scales with graph size carries an explicit
  budget expressed in edges.
- **Snapshot-tested.** The drawings are the specification: any change that
  moves a character shows up as a diff a human reads and accepts by hand, and
  there is deliberately no update-everything switch.

## Limits

- **Small graphs are provably optimal; large ones are not.** An exact solver
  checks this: on the committed fixtures, the layout sits at or within two
  points of the proven floor. On dense graphs — fifty nodes, a hundred edges —
  it is measurably worse, and the remaining defects are documented.
- **Back edges keep their original direction.** A cycle's edge is taken out of
  the layering and drawn through a lane under the boxes, because a reversed
  arrow reads as pointing the wrong way in a terminal, which costs more than
  the layout does.
- **Six palette slots.** Beyond six distinguishable flows they wrap, and a
  drawing that needs more than six has a bigger problem than the palette.
- **Not in scope:** undirected graphs, hypergraphs, nested clusters,
  interactivity, hit testing, animation, and any form of pixel rendering.

## Documentation

| | |
|---|---|
| [docs/design.md](docs/design.md) | what this is meant to be, written before the code |
| [docs/quality.md](docs/quality.md) | how a change to the layout is judged |
| [docs/painter.md](docs/painter.md) | how edges become glyphs |
| [docs/adr/](docs/adr/) | decisions, with their consequences |

## Licence

MIT or Apache-2.0, at your option.
