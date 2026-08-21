# Contributing

## Before a change

Read `docs/design.md` for what the library is and `docs/plan.md` for the order it
is being built in. A change that contradicts either is fine — say so in the commit
body and change the document in the same commit.

## The loop

```
just fmt     # rustfmt
just lint    # fmt --check, then clippy pedantic as errors
just test    # the whole suite
just ci      # what CI runs: lint, then test
```

Everything runs offline with no configuration.

## Drawings are the specification

Rendered frames are stored as snapshots. A change that moves a character fails a
snapshot test and leaves an `.snap.new` file beside the old one.

Read both frames. Accept the new one by moving it into place, and quote the frame in
the commit body so the change is legible from the log alone. There is deliberately no
accept-everything switch: a snapshot accepted without being read is worse than no
snapshot.

## The numbers

From the point the scorer exists, `cargo run --example score` prints every fixture's
score worst-first, and a test compares those numbers against a stored baseline.

The vocabulary tier — the categorical defects in `docs/design.md` §6 — may never rise.
The soft tier may rise for one graph only if the total across all of them falls. Never
edit the scorer, the gate, the fixtures or the baseline to make a change pass; if the
objective is wrong, that is its own commit with its own argument.

## Commits

Conventional commits (`feat:`, `fix:`, `docs:`, `test:`, `chore:`), subject in the
declarative present — "a fan leaves symmetrically", not "make fans symmetric". The body
says what moved and, once there are numbers, by how much.
