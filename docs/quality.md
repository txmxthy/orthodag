# Working on layout quality

Use this guide when changing `src/layout/`. A layout change needs both score
comparisons and visual review, because the objective does not capture every
aspect of readability.

## Allowed drawing shapes

The scorer checks the rules from [the design](design.md#5-drawing-rules):

- Between neighbouring columns, an edge may be straight or have one or two
  bends. An edge that skips columns may bend at each end, up to four times in
  total.
- An edge may touch at most one fork run from its source trunk and one join
  run onto its target trunk.
- Unrelated edges may share a cell only as a crossing, with one passing
  horizontally and the other vertically. Other shared cells are overlaps and
  can imply a connection that does not exist in the graph.
- Fan-outs and fan-ins should be symmetric about their attachment row.

## Score fields

Run `cargo run --example score` to print the fixtures in descending score order.

| Tier | Fields | Requirement |
|---|---|---|
| Vocabulary | `bends_fwd`, `bends_skip`, `junctions`, `overlaps` | Reach zero and remain there |
| Soft | `cross`, `asym`, `detour` | Reduce the aggregate cost |
| Diagnostic | `blends`, `mixed`, `jogs`, `ink`, `width`, `height` | Use to investigate changes; excluded from the search objective |

`blends` counts fork or join cells carrying different colours. These arise when
separate flows are bundled onto a shared trunk. `mixed` includes colour
conflicts at genuine crossings as well. A bridge helps make a crossing
readable, but the cell still cannot display both colours.

The search uses this weighted total:

```text
total = 10·(bends_fwd + bends_skip + junctions)
      + 5·overlaps + 3·cross + 2·asym + detour
```

The regression test uses the unweighted sum `cross + asym + detour` for its
aggregate soft check. This differs from the weighted search objective.

When evaluating bundling, inspect `ink`: it counts occupied edge cells once,
even if several routes share them. Per-edge measurements may count a shared
section several times and therefore do not directly measure the space saved.

## Committed fixtures

All nine fixtures are listed in `CLEAN` in `tests/quality.rs`, which requires
zero vocabulary defects for each of them.

The last fixture to reach zero was `ladder`, which had four overlaps caused by
untagged routes turning on each other's corner rows. The initial diagnosis
suggested that untagged fans needed separate attach rows. Placement descent
resolved the overlaps by moving the affected column, so the port change was
not needed for that fixture.

## Wider corpus results

To include twelve generated graphs and any files in `testdata/private`, run:

```sh
cargo run --features mermaid --example score -- --wide
```

The following results are retained from earlier measurements. They describe
those runs, including the private corpus available then; they are not a fresh
benchmark of the current checkout.

One wider run contained 71 graphs, of which seven generated graphs had vocabulary
defects. The generated cases included graphs with up to 50 nodes across a dozen
columns, with skips and cycles. One reported score was:

```text
seed-11  junctions 0 overlaps 0 cross 299 asym 1564 detour 554  total 4579
```

That run had no overlaps. Excess junctions remained on three generated graphs,
while crossings and asymmetry accounted for much of the remaining soft cost.
The small committed fixtures are useful for diagnosing specific behaviours but
do not establish quality on these larger graphs.

## Search-budget experiment

After drawing evaluation became roughly five times faster, an experiment raised
the ordering, placement-descent and swap-search budgets in `layout/mod.rs`.
The recorded comparison was:

| Measurement | Shipped budgets | Raised budgets |
|---|---|---|
| Private corpus metrics | Unchanged | Unchanged |
| Fixtures and generated graphs with vocabulary defects | 4 | 4 |
| Total score for fixtures and generated graphs | 22181 | 18788 |
| Wide-run wall time | 2.1 s | 7.1 s |

The private corpus had already converged: its searches stopped after a pass
without improvement, so extra budget made no difference. On generated graphs,
the larger budgets reduced the aggregate total by about 15% while taking about
3.4 times as long. They did not eliminate any vocabulary defects. All four
remaining cases had excess junctions, and three were already in the bracket
with the most generous search budget.

These results did not justify raising the budgets. They suggest trying different
moves for the remaining defects, particularly where the existing search has
already converged. The counts in this experiment belong to its own run and
should not be combined with the wider-corpus results above.

## Regression checks

Record an accepted set of scores with:

```sh
just baseline
```

This rewrites the committed `testdata/quality-baseline.txt` from the public
fixtures using the score example's name-sorted `--record` output. A missing,
malformed or incompatible baseline fails the quality test. Private corpus data
is never included.

`tests/quality.rs` applies these per-field allowances relative to the baseline:

| Fields | Maximum increase per fixture |
|---|---|
| Vocabulary fields | 0 |
| `width` | 4 |
| `height` | 2 |
| Other reported fields | 3 |

It also requires the aggregate soft score to stay at or below the baseline.
Diagnostic fields are therefore checked for regressions even though they do
not guide the layout search.

A vocabulary improvement can justify a soft-score increase, but accepting that
trade requires review. For example, an earlier track-packing change reduced
overlaps from 36 to 8 while increasing the soft score by nine. Record that kind
of decision and its measured deltas in the commit.

Do not edit the scorer, fixtures, `CLEAN` list or baseline merely to make a
change pass. If the objective needs correcting, make that a separate change
with its own rationale. Update the baseline only after accepting the layout
change.

## Visual review

```sh
just gallery            # committed fixtures
just gallery --wide     # also generated graphs and any private corpus
```

The output is `target/gallery/index.html`. Each graph has four frames: plain,
with labels, with bridges and fitted to 80 columns. Sort by name or score and
record observations beside the affected frames.

Notes are stored in the browser and are available only there. No server is
required. The gallery can include private corpus data, so keep the generated
page local.

## Review procedure

1. Start with the highest-scoring graph and inspect its largest defect in the
   gallery. Confirm what the score refers to before choosing a change.
2. Change one layout behaviour.
3. Run `cargo test --all-features`. A snapshot mismatch leaves an `x.snap.new`
   beside `x.snap`. Compare both frames and accept with `mv` only after review
   against the drawing rules. Include the relevant drawing change in the commit
   body.
4. Run `cargo run --example score` and the baseline regression test. Inspect both
   vocabulary and soft-score deltas.
5. Commit the accepted change with its measured deltas, or discard it.
