# Retained rendering and IME audit

The audit reproduced avoidable work and incorrect hidden rendering:

- Replacing an image source with different intrinsic dimensions triggered layout
  even when both bitmap dimensions were concrete. Fixed-size Fill replacement
  now repaints with zero layout nodes (the regression previously measured four).
  Intrinsic, percentage-only and one-axis sizing still invalidate layout; aspect
  fitting still updates bitmap geometry when its aspect ratio changes.
- Fully transparent images could upload textures and fail the visible-image
  budget. Transparent flat and isolated subtrees now skip uploads and layer
  preparation. Existing bounded resident caches are not eagerly purged on hide.
- A zero-opacity backdrop filter could still blur pixels behind it in the CPU
  renderer. Hidden paint items are filtered before blur detection and drawing.
  CPU and GPU tests cover hidden/reveal/hide, including isolated groups; hidden
  content leaves the backdrop unchanged and incremental pixels match full redraw.

The integrated source archive preserves compiled inputs and native smoke scripts.
Metadata records exact build/run commands, binary hashes and the source manifest.
Native image-fitting, AT-SPI and IBus/libpinyin smoke runs use owned X11 displays
and private services. Rendering uses Mesa llvmpipe, not hardware GPU evidence.

The desktop audit also covers accessibility-driven cancellation of an active
IME preedit. The host must release keyboard suppression when the editor cancels
composition, reject stale native events during session restart, and preserve
composition when an event handler prevents the selection action. Deterministic
policy tests exercise the restart event ordering; separate native IBus and AT-SPI
runs validate their existing end-to-end paths. Those separate runs do not prove
a combined live screen-reader/IME cancellation scenario or macOS IME behavior.

The IME restart guard waits for ordered winit `Disabled` then `Enabled`
notifications before accepting composition again, while ordinary keyboard input
is released immediately. This protects the queued reset interval, not an OS
session epoch: pinned winit's Wayland notifications are local, and its text-input
completion handler does not use the compositor serial to reject older commits.
A delayed server commit after the guard reopens therefore remains a Wayland
integration limitation. Focus changes retain the existing target-routing policy.

The final integrated run passes **448 tests** (two opt-in tests ignored),
**29 doctests**, strict workspace Clippy, formatting and the macOS ARM64
cross-check. All three native smoke suites pass on the archived build.
The 136 source inputs were independently verified against the workspace
and archive after execution. macOS cross-checking is not native runtime evidence.
