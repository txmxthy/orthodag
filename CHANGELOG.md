# Changelog

Notable changes, newest first. Versions follow [semver](https://semver.org/).

## 0.1.0

- The graph model: vertices with a label and extra lines, edges with a canonical
  tag set.
- Cycle removal by depth-first three-colouring. Back edges are removed, not
  reversed.
- Layer assignment by longest path in Kahn order.
- Long edges cut into single-hop segments with a placeholder per column passed.
- Column ordering by alternating barycenter sweeps, keyed on exact rationals.
  A sweep proposes candidates; nothing chooses between them yet.
- Coordinates by alternating median sweeps, with each column pressed back into
  the height of the fullest one.
- A long edge holds one row for every column it passes, so it cannot bend in the
  middle.
- Orthogonal routes: straight, an L, or a Z between neighbours; four bends at
  most across a skip.
- The painter: four direction bits per cell, one glyph per combination, boxes
  and labels laid over the lines.
- `draw(&Graph) -> String`, the first thing here that produces output.
- The scorer: per-edge bends, forks, joins, crossings, overlaps and detour,
  counted on the drawn cells, in three tiers.
- `score(&Graph) -> Score`, an example that prints every fixture worst-first,
  and a ratchet test against a stored baseline.
- Track packing: every vertical run takes a channel of its own in the gap,
  except where two runs meet at an end and are one trunk. Tracks are ordered
  left to right by an exact search while there are few of them.
- Ports: one attach row per tag set, and boxes tall enough to hold them.
- Colour: `Colour`, `PALETTE`, `colour::of`, and `spans(&Graph) -> Vec<Vec<Span>>`
  for the drawing as runs of one colour.
- Back edges draw, through a lane under the boxes, with an arrowhead that points
  the way it is going.
- `Options`, and `draw_with` / `spans_with` / `score_with`. `labels` draws each
  edge's tags on it; `crossings` chooses between `Cross` and `Bridge`; `width`
  shrinks the drawing through a ladder of steps to fit a target.
- Mermaid in and out behind the `mermaid` feature, DOT out behind `dot`.
- A seeded generator, a loader for a private corpus, and a gallery page.

Everything the design set out to build exists. All nine committed fixtures have
zero vocabulary defects and are protected by a checked-in score baseline.
Generated and private corpora remain review evidence rather than a guarantee
for arbitrary graphs. See `docs/quality.md`.
