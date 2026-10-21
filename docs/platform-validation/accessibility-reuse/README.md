# Reusing unchanged native accessibility nodes

Native projection previously constructed each visible AccessKit node, copying
its full value, before comparing it with the cache. An unrelated button change
therefore copied unchanged editor documents even though no editor node was
emitted in the incremental update.

`Semantics::node_revision` now exposes an opaque per-node token. Equal updates
preserve it; changes affect only that node. A store identity prevents a new
Semantics instance from reusing coincident counters. Removal/reinsertion receives
a new token. Generic mutation callbacks publish a changed token even when a
caller catches a panic after a partial mutation, preserving cache invalidation.
This does not roll back the mutation.

Projection compares the semantic token plus scaled bounds, inherited disabled
state and native child sequence before rebuilding a node. It reuses the retained
AccessKit node's geometry/children, adding only a semantic-token map rather than
duplicate geometry or child-vector snapshots. Hidden/removed nodes prune the map;
reset clears it so reconnects receive complete snapshots. Reused text runs must
still have cached nodes and selection positions.

An independent allocator regression projects a 128 KiB editor beside a changing
sibling. Unrelated updates and idle projection allocate nothing as large as that
document, emit no editor/text-run nodes, and retain IDs and selection mapping.
The consumer subsequently accepts a real editor selection update. Other tests
cover inherited disabled state, translation, scaling, logical child ownership,
hide/show, reconnects, replacement semantic stores, Unicode text and read-only
actions. Existing core semantic allocation regressions remain intact.

This avoids unchanged-value copies, not all projection work: visible scene
metadata is still traversed, and changed nodes still need owned AccessKit values
and cached clones. Per-node semantic entries and projected tokens add bounded
metadata. No whole-framework timing or RSS improvement is claimed from these
allocation tests.

The live AT-SPI, editor model and native IME suites exercise the integrated build.
Linux native validation uses owned Xvfb/Openbox sessions and Mesa llvmpipe;
native macOS and hardware GPU behavior remain unverified. Sources, exact
commands, hashes and validation logs are archived alongside this record.

The integrated build passes **526 workspace tests** (two opt-in tests ignored),
**29 doctests**, strict Clippy, formatting and the macOS ARM64 cross-check.
All three native suites pass. All 160 source hashes match archive and
workspace after execution.
