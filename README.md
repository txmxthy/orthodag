# orthodag

orthodag renders a directed graph as Unicode box-drawing text.

```
                                    ┌──────┐
                                ┌──▶│ even │─┐
┌────────┐     ┌──────────────┐ │   └──────┘ │   ┌──────┐
│ source │────▶│ split        │─┘            └──▶│ sink │
└────────┘     │ 2 partitions │─┐   ┌──────┐ ┌──▶│      │
               └──────────────┘ └──▶│ odd  │─┘   └──────┘
                                    └──────┘
```

Vertices become boxes, and edges become orthogonal lines that fit a target
width, against an explicit objective for what a good drawing looks like.

The default build carries no dependencies, and there is never a dependency
on a terminal library. The output is text and styled spans, and whether
those become escape codes, HTML or a widget buffer is the caller's
business.

> **Status: 0.1.0, early.** Everything documented below works, though
> whether the drawings are *good* is a separate question, asked and
> answered in [docs/quality.md](docs/quality.md): the small graphs are
> fine and the large ones are not yet.

## Start here

```toml
[dependencies]
orthodag = "0.1"
```

```rust
use orthodag::{Graph, Node};

let mut g = Graph::new();
let source = g.add_node(Node::new("source"));
let split = g.add_node(Node::new("split").line("2 partitions"));
let even = g.add_node(Node::new("even"));
let odd = g.add_node(Node::new("odd"));
let sink = g.add_node(Node::new("sink"));

g.add_edge(source, split);
g.add_tagged_edge(split, even, ["even"]);
g.add_tagged_edge(split, odd, ["odd"]);
g.add_tagged_edge(even, sink, ["even"]);
g.add_tagged_edge(odd, sink, ["odd"]);

print!("{}", orthodag::draw(&g));
```

That prints the drawing above. Nothing is parsed and nothing is inferred,
because a graph is exactly what the caller builds.

A vertex carries a headline and any number of further lines. An edge
carries a tag set, sorted and deduplicated on the way in, because tags
decide two things later: which edges are drawn as one line, and what
colour a flow keeps across the drawing.

## Cycles

A back edge is excluded from the layering and drawn on its own, keeping
its original direction: a reversed arrow reads as pointing the wrong way,
whatever the rest of the layout looks like. It gets a lane under the
boxes instead, with an arrowhead that points the way it is going.

```
┌────┐     ┌──────┐     ┌───────┐
│ in │────▶│ work │────▶│ retry │
└────┘     └──────┘     └───────┘
               ▲            │
               └────────────┘
```

## Colour

`spans` hands back the drawing as runs of one colour. The library does
not decide what a colour is: it returns a palette slot, and the caller
decides what that slot looks like.

```rust
for row in orthodag::spans(&g) {
    for span in row {
        match span.colour {
            Some(slot) => print!("\x1b[3{}m{}\x1b[0m", slot.slot() + 1, span.text),
            None => print!("{}", span.text),
        }
    }
    println!();
}
```

Slots are assigned by tag set, so every edge in that set shares one
flow's colour across the whole drawing. A box grows an interior row per
tag set so no two flows have to share an attach row and lose one of their
colours. There are six slots, and after that they wrap.

A span with no colour is a box, a label, or a cell where two flows met,
and the caller paints those in whatever its default ink is.

## Options

```rust
use orthodag::{Crossing, Options};

orthodag::draw_with(&g, Options::new().labels(true));                 // tags on the edges
orthodag::draw_with(&g, Options::new().crossings(Crossing::Bridge));  // ──╴│╶── not ───┼───
orthodag::draw_with(&g, Options::new().width(80));                    // fit 80 columns
```

```
┌────────┐     ┌──────────────┐      │  └──────┘      │  ┌──────┐
│ source │────▶│ split        │─even─┘                └─▶│ sink │
└────────┘     │ 2 partitions │─odd──┐  ┌──────┐      ┌─▶│      │
```

Asking for a width walks a fixed ladder — roomier gaps first, then
shorter box text — and stops at the widest rung that fits. If even the
last rung is too wide, that one comes back instead, because half a box
is worse than a wide one, so nothing is ever clipped.

## Scoring

`score` returns an explicit objective in three tiers: defects that are
categorical and must be zero, a scalar to push down, and numbers reported
because they are useful rather than because they are goals.

```
chain     bends_fwd 0 bends_skip 0 junctions 0 overlaps 0 ... total 0
ladder    bends_fwd 0 bends_skip 0 junctions 0 overlaps 4 ... total 110
```

An overlap is a cell where two unrelated edges are drawn as one line. The
ladder had thirty-two of them before the gaps were given tracks, and the
frame alone did not show that. [docs/quality.md](docs/quality.md) has the
rules and the loop.

## Features

| feature | gives you |
|---|---|
| *(none)* | the model, the layout, the painter, the scorer |
| `mermaid` | `io::mermaid_in::from_mermaid`, `io::mermaid_out::to_mermaid` |
| `dot` | `io::dot::to_dot` |
| `serde` | `Serialize`/`Deserialize` on the model and the score |

Round-tripping through Mermaid is tested: a graph written out and read
back comes back as the same graph.

## Development

```
just ci        # fmt --check, clippy pedantic as errors, the whole suite
just score     # every graph, worst first
just gallery   # every graph on one page, to be looked at
```

Everything runs offline with no configuration.

- [docs/design.md](docs/design.md) — what this is meant to be, written before
  any code.
- [docs/plan.md](docs/plan.md) — the order it was built in.
- [docs/painter.md](docs/painter.md) — how edges become glyphs.
- [docs/quality.md](docs/quality.md) — how a change to the layout is judged, and
  where it currently falls down.

## Licence

MIT or Apache-2.0, at your option.
