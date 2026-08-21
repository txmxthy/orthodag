# orthodag

orthodag renders a directed graph as Unicode box-drawing text. Vertices
become boxes, and edges become orthogonal lines that fit a target width
against an explicit objective for what a good drawing looks like.

> **Status: early.** Nothing draws yet. [docs/design.md](docs/design.md)
> sets out what it is meant to be, and [docs/plan.md](docs/plan.md) sets out
> the order it gets built in.

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
