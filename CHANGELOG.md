# Changelog

Notable changes, newest first. Versions follow [semver](https://semver.org/);
nothing is released yet.

## Unreleased

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

Nothing draws yet.
