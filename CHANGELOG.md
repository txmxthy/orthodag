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

Nothing draws yet.
