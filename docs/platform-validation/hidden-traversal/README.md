# Hidden subtree traversal and layer bounds

The retained-layer iterator now prunes zero-opacity subtrees before visiting
children or accumulating offscreen layer bounds. Previously, a visible 24×24
isolated parent with a hidden 8192×8192 descendant failed the GPU layer budget,
even though drawing correctly skipped that descendant. The regression now
renders the visible parent with less than 4 KiB of cached layer pixels.
Resizing and revealing the child updates layer geometry and produces matching
incremental/full-redraw pixels.

An explicitly requested layer root still excludes its own opacity from cached
content, so visibility remains an external composition property. Public
`paint_items()` and isolated-node enumeration preserve their existing liveness
semantics; hidden content remains mounted and its state remains available.
This change does not skip layout for hidden nodes or remove cached resources
solely because a view becomes transparent.

Hidden backdrop filters no longer expand damage caused by unrelated visible
updates. Effective ancestor opacity is included when deciding whether a filter
can affect output. Visible blur expansion remains conservative.

The source archive includes compiled inputs and smoke scripts; metadata records
commands and binary/source hashes. GPU tests use Mesa llvmpipe. Native image-fit
and hidden-window update smoke tests exercise the integrated build on owned
Xvfb/Openbox displays. Hidden-window checks establish map/restore behavior, not
opacity pruning by themselves; the core and GPU regressions cover pruning.

The final build passes **453 workspace tests** (two opt-in tests ignored),
**29 doctests**, strict all-target Clippy, formatting and the macOS ARM64
cross-check. Native image fitting passes all eight stages; the hidden-window
run verifies 40 model updates across hide/minimize and correct restored pixels.
All 136 archived source hashes match both the archive and workspace after execution.
