# orthodag

orthodag renders a directed graph as Unicode box-drawing text. Vertices
become boxes, and edges become orthogonal lines that fit a target width
against an explicit objective for what a good drawing looks like.

> **Status: early.** Nothing draws yet, but the model exists, and a graph
> can already be put into columns. [docs/design.md](docs/design.md) sets
> out what it is meant to be, and [docs/plan.md](docs/plan.md) sets out the
> order it gets built in.

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
| the painter, the scorer | not started |

Nothing above is public except the model: the phases stay internal until
there is a `layout()` to call them from.

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
