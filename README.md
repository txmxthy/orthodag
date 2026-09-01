# orthodag

orthodag renders a directed graph as Unicode box-drawing text. Vertices
become boxes, and edges become orthogonal lines that fit a target width
against an explicit objective for what a good drawing looks like.

> **Status: early.** It draws, though not yet well: there is no scorer, no
> track packing and no colour, so a fan crosses its own trunk and a long
> edge can land on another line. [docs/design.md](docs/design.md) sets out
> what it is meant to be, and [docs/plan.md](docs/plan.md) sets out the
> order it gets built in.

```
┌────┐     ┌─────┐     ┌─────┐
│ in │────▶│ cat │────▶│ out │
└────┘     └─────┘     └─────┘
```

## The model

A graph is vertices and edges, built by the caller. Nothing is parsed.

```rust
use orthodag::{Graph, Node};

let mut g = Graph::new();
let source = g.add_node(Node::new("source"));
let even = g.add_node(Node::new("even").line("2 partitions"));
let odd = g.add_node(Node::new("odd"));

g.add_tagged_edge(source, even, ["even"]);
g.add_tagged_edge(source, odd, ["odd"]);

print!("{}", orthodag::draw(&g));
```

```
               ┌──────────────┐
            ┌─▶│ even         │
┌────────┐  │  │ 2 partitions │
│ source │──┤  └──────────────┘
└────────┘  │
            │  ┌──────────────┐
            └─▶│ odd          │
               └──────────────┘
```

A vertex carries a headline and any number of further lines drawn under it.
An edge carries a tag set, sorted and deduplicated on the way in, because
tags decide two things later: which edges are drawn as one line, and what
colour a flow keeps across the drawing.

## Where it is

| phase | state |
|---|---|
| the model | done |
| cycle removal | done — back edges keep their original direction and are routed separately |
| layer assignment | done — longest path, one column past the latest predecessor |
| ordering, coordinates, routing | next |
| the painter | done — direction bits, one glyph per combination |
| tracks | done — a fan is one trunk with a branch each |
| ports, colour | next |
| the scorer and the quality gate | done — two of nine fixtures still fail it |

`draw` and `score` are the entry points so far, and `draw` returns a
`String`. The design calls for layout, drawing and scoring as three
separate calls, so a caller can colour one edge or ask what a drawing is
worth, though none of that exists yet.

Run `cargo run --example draw` to see the drawings above and a couple more,
and `cargo run --example score` to see what each of them is worth.

## Scoring

There is an explicit objective, in three tiers: defects that are
categorical and must be zero, a scalar to push down, and numbers reported
because they are useful to know rather than because they are goals.

```
chain     bends_fwd 0 bends_skip 0 junctions 0 overlaps 0 ... total 0
ladder    bends_fwd 0 bends_skip 0 junctions 0 overlaps 4 ... total 110
```

An overlap is a cell where two unrelated edges are drawn as one line. The
ladder had thirty-two of them before the gaps were given tracks, and the
frame alone did not show that. [docs/quality.md](docs/quality.md) has the
rules and the loop.

## Building

```
just ci      # fmt --check, clippy pedantic as errors, the test suite
```

Or without [`just`](https://github.com/casey/just):

```
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Everything runs offline. There are no required features, and no optional
dependency is on by default.

## Licence

MIT or Apache-2.0, at your option.
