# Working on layout quality

Read this before changing anything under `src/layout/`. It explains how a
layout change is judged.

## Allowed drawing shapes

A drawing may use only these shapes. They restate the rules in
[`design.md` §5](design.md) as the things the scorer counts.

- An edge is straight, an L with one bend, or a Z with two bends. Between
  neighbouring columns nothing else is allowed. An edge that skips columns
  may bend at each end, for four bends at most, regardless of how many
  columns it skips.
- An edge touches at most one fork run and one join run: one branch off its
  source's trunk, one merge onto its target's.
- Two unrelated edges may share a cell only as a crossing, with one passing
  straight through horizontally and the other straight through vertically.
  Any other shared cell between unrelated edges is an overlap, which can
  make edges that are not connected look like one line.
- Fan-outs and fan-ins are symmetric about the row they leave from.

## Score fields

Run `cargo run --example score` to print every fixture, worst first.

| Tier | Fields | Requirement |
|---|---|---|
| Vocabulary | `bends_fwd`, `bends_skip`, `junctions`, `overlaps` | Reach zero and remain there |
| Soft | `cross`, `asym`, `detour` | Reduce the aggregate cost |
| Reported | `jogs`, `ink`, `width`, `height` | Useful for investigating changes; not part of the search objective |

`total` is:

```text
10·(bends_fwd + bends_skip + junctions) + 5·overlaps + 3·cross + 2·asym + detour
```

When judging whether edges should merge, inspect `ink`. It counts drawn
cells once, even when several routes share them. Every other field counts
by edge, so two edges drawn as one line each pay for the full path, and a
merge can look like a regression on those fields. Ink is the fair measure
of a merge.

## Committed fixtures

Eight of the nine fixtures are inside the vocabulary. One is not:

| Fixture | Defect |
|---|---|
| `ladder` | 4 overlaps: two runs turning on each other's corner rows |

`ladder`'s edges carry no tags, so every edge at a box shares the one
attach row the empty tag set gets, and the rows that would separate the
two runs do not exist. Giving an untagged fan rows of its own is the
obvious next move. It is not in the design as written, and it needs an
argument first.

`tests/quality.rs` lists the fixtures known to be clean, in `CLEAN`. A
fixture that reaches zero is added to that list and never removed.

## Regression checks

The second test compares every number against a stored baseline:

```sh
cargo run --example score -- --record > target/quality/baseline.txt
```

The baseline file lives under `target/`, so it is not committed and CI
never sees it. Without a baseline, the test skips. It is a local tool that
supports a human working through the loop below. Nobody else has to
produce one before their build passes.

The vocabulary tier must not rise for any fixture. The soft tier may rise a
little for one fixture, as long as the total across every fixture falls.

The test does not decide this for you: the tiers are ordered, so a change
that clears a vocabulary defect can be worth taking even when the soft
tier rises, because a defect is categorical and a soft cost is not. Track
packing was exactly that trade: it took overlaps from 36 to 8, while the
soft tier rose by nine. Accept a trade like that deliberately, and record
it in the commit. Do not weaken the test so it stops asking the question.

Do not edit the scorer, the fixtures, the clean list, or the baseline
merely to make a change pass. If the objective itself is wrong, make that
its own change with its own argument.

## Review procedure

1. Start with the fixture that has the largest total, and its most
   expensive defect.
2. Change one thing.
3. Run `cargo test`. A moved character fails a snapshot and leaves
   `x.snap.new` beside `x.snap`. Read both, and accept with `mv` only when
   the new frame is better by the vocabulary above. Quote the change in
   the commit body. There is no accept-everything switch, on purpose.
4. Run `cargo run --example score`, then the regression check above.
5. Commit the accepted change with its deltas in the body, or discard it.
