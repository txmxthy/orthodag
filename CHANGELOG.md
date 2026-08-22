# Changelog

Notable changes, newest first. Versions follow [semver](https://semver.org/);
nothing is released yet.

## Unreleased

- The graph model: vertices with a label and extra lines, edges with a canonical
  tag set.
- Cycle removal by depth-first three-colouring. Back edges are removed, not
  reversed.
- Layer assignment by longest path in Kahn order.

Nothing draws yet.
