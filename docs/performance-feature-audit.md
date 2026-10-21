# Feature expansion: default costs and resource bounds

This source audit records checks made during the GPUI capability expansion. It does not replace the matched streaming/list benchmarks and does not claim universal performance superiority.

## Default paths

- Ordinary row/column/overlay nodes continue to use the original cached layout algorithm. Extended layout has an optional shared options block; default styles allocate no block. Only containers requiring the advanced algorithm allocate a retained Taffy tree. The tree retains immediate child IDs and removes obsolete generations.
- `Style` and `QuadStyle` are now `Clone`, rather than `Copy`, because optional extended data is shared. Default clones do not allocate an extended block. Extended options retain their shared allocation across unrelated dimension refinements; an integration regression checks pointer identity after a reactive width update. Reusing a canonical scene options allocation skips redundant normalization.
- Plain text retains its original shaping path. Display overflow/clamp options are compact default values; rich shaping is selected for nondefault display options or explicit rich text. Empty font fallbacks share one empty allocation, and default font features contain no allocation.
- Visibility observers are registered by consumers, not for every scene node. They run after geometry revision changes. Hidden animated images release their timer task. Hidden-focus ancestry checks run only after a layout/geometry change. Idle frame preparation publishes no observer updates.
- Explicit cursors maintain an exact count. When the last custom cursor is cleared or removed, `cursor_at` returns through the no-cursor fast path without hit-test allocation. A regression covers changing, clearing and subtree removal.
- A drag session exists only during an active gesture. Preview components are retained and translated, not rebuilt on pointer movement. Completion, cancellation, disabled/hidden source and removal dispose the preview. No drag timer is allocated.

## Raster caches

Canvas and SVG caches each enforce **32 MiB of raster pixels and 1,024 entries**, evicting least-recently-used entries before insertion. Entry count independently bounds metadata even for one-pixel rasters; tests fill each cache with 1,025 one-pixel entries and check count, bytes and oldest-entry eviction.

Canvas entries use `Weak<Canvas>`, so removed scene nodes release command and path storage even if a hidden host has not painted again. The weak control block can remain until cache eviction, but the source object's owned buffers are dropped. Generated decoration canvases need no strong retention after rasterization: unchanged detailed styles reuse the raster directly. Accepted cached decoration styles contain at most 1,024 gradient stops and 32 shadows. With 1,024 entries, their retained variable-size style arrays are bounded by 1,048,576 gradient-stop records and 32,768 shadow records, plus constant-size metadata. Shared style allocations are counted conservatively here; application-owned scene/style objects have their own lifetimes.

SVG entries retain a stable numeric source identity and tint, not XML bytes. Source identity survives clone, tint and affine refinement. A pure transform change reuses pixels even after the previous source wrapper is dropped. Unchanged transform hits also reuse the same `Arc<ImageData>` rather than allocating a new image wrapper. Source inputs are limited to 4 MiB each. Lifetime regressions remove sources before the next paint and confirm that cached rasters do not keep source payloads alive.

## Other bounded data

- Async image cache: at most 256 entries, configurable pixel budget capped at 64 MiB, 1,024-byte keys; abandoned pending requests are removed. Decoded animations permit at most 4,096 frames and 64 MiB of decoded pixels, with bounded nonzero frame durations.
- Canvas rasterization rejects more than 65,536 commands; paths reject more than 65,536 segments. Clip depth is bounded and mask storage has a separate 32 MiB check. These checks bound renderer work; caller-owned builders remain ordinary owned Rust data.
- Native grouped file transfers reject oversized complete batches rather than truncating them. X11/macOS assembly permits 1,024 paths and 1 MiB of encoded path bytes. Wayland additionally bounds URI transfer bytes and read duration. `FilesDropRejected` exposes `TooManyFiles`, `TooLarge`, `InvalidData` or `TimedOut`.
- Virtualized rows continue to own only the mounted range and overscan. Removing rows releases component registrations, tasks, visibility observers, previews and scene nodes. Renderer caches hold bounded pixels/metadata rather than keeping removed Canvas/XML source payloads alive until a later presentation.

The integrated regression suite and final benchmark packet record the tested revision. These bounds describe framework-owned storage; application models, active SVG/path sources, driver allocations and external font databases are not included in a cache's pixel counter.
