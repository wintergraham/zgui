# Native editor shaping reuse validation

The wrapped-editor and inherited-line-height examples pass native X11 input
checks with the focused-editor layout cache enabled. The wrapped test exercises
selection, editing, scrolling, a focused resize from 620 to 380 logical pixels,
click placement on a wrapped row, and content shrink. The line-height test
changes pitch through 24, 42, 12 and normal 28 pixels while preserving the model
and selection. Screenshots of the narrowed wrapped editor and 42-pixel pitch
were inspected.

- [Wrapped editor results](wrapped_editor/results.json)
- [Line-height results](line_height/results.json)
- [Build and binary provenance](metadata.json)
- [Source manifest](source-manifest.json) and [archive](source.tar.gz)

All 125 source inputs were unchanged across the build and both smoke tests.
The run used owned Xvfb/Openbox processes, Mesa llvmpipe, and the private fixed
Vulkan loader. The archive includes Rust sources, shaders, font, manifests,
compiled documentation inputs and smoke scripts.

The accompanying logs record 390 passing workspace tests (one opt-in GPU stress
test ignored), 26 documentation tests, strict Clippy, formatting and the macOS
ARM64 cross-check. Native GPU tests independently verify zero additional shaping
calls during unchanged navigation and matching damage/full-repaint pixels.
See [reuse measurements and limits](../../performance/editor-shaping/README.md).

These checks do not establish native macOS behavior, hardware GPU performance,
frame latency or a CPU/RSS reduction of a particular size.
