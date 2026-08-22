# 1. Use dense indices for graph IDs

## Context

`NodeId` and `EdgeId` wrap `u32` indices into the graph's node and edge vectors.
This provides direct lookup and lets layout phases store ranks, orderings and
rows in vectors indexed by the same IDs.

An index does not identify the graph that created it. An earlier version of
`add_edge` returned `Result<EdgeId, UnknownNode>` to reject invalid node IDs,
but it could only detect an out-of-range value. A test demonstrated the gap:
`NodeId(0)` from one graph is in range in any other graph containing a node.
The check could not enforce the promised ownership restriction.

Panicking on an invalid index would also conflict with the library's policy
against `unwrap`, `expect` and explicit panics outside tests.

## Decision

Keep IDs as dense indices. `add_edge` and `add_tagged_edge` return an `EdgeId`
without an error result. Callers must use IDs from the graph they are modifying;
the API documents this requirement but does not enforce it.

`Graph::node` and `Graph::edge` return `Option` for lookups. An out-of-range ID
therefore returns `None`. An in-range ID from another graph resolves to the local
entry at that index.

## Consequences

The API does not offer a validation result that could be mistaken for proof of
graph ownership. Layout storage remains simple, with no ownership token or map
needed for each lookup.

A caller that mixes IDs from different graphs may get an incorrect drawing
without an error. This is an accepted limitation of the representation, and
callers must keep their graph-specific IDs separate.

Branding IDs with an invariant lifetime could enforce ownership at compile
time. That alternative was rejected because it would add lifetime parameters
to the public API to prevent a mistake considered uncommon in the intended
usage.
