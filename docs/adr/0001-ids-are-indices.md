# 1. Use graph-branded dense indices

## Context

`NodeId` and `EdgeId` carry `u32` indices into the graph's node and edge vectors.
This provides direct lookup and lets layout phases store ranks, orderings and
rows in vectors indexed by the same IDs.

An index alone does not identify the graph that created it. `NodeId(0)` from one
graph is in range in any other graph containing a node, so range checks cannot
enforce ownership without another identity component.

Panicking on an invalid index would also conflict with the library's policy
against `unwrap`, `expect` and explicit panics outside tests.

## Decision

Keep the dense index and brand each ID with an opaque process-local graph
provenance allocated from an atomic counter. Equality includes both components,
while only the index is exposed. Cloning a graph preserves its provenance so
existing IDs address the structural clone; structural graph equality ignores
provenance.

`add_edge`, `add_tagged_edge` and `relabel` reject foreign and out-of-range IDs.
`Graph::node` and `Graph::edge` return `None` for either case. `Graph::validate`
checks that every stored edge endpoint has the graph's provenance and names a
stored node.

## Consequences

Mixing IDs from different graphs is detected at the graph boundary without a
map lookup. Layout storage remains vector-backed and indexes it with the dense
component.

Provenance is meaningful only within one process. It is not displayed or part
of a serialization contract.

Branding IDs with an invariant lifetime could enforce ownership at compile
time. That alternative was rejected because it would add lifetime parameters
to the public API to prevent a mistake considered uncommon in the intended
usage.
