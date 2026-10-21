# Native percentage sizing validation

The public `percent_sizes` example renders a padded full-window column, a fixed
report button, and a growing/shrinking row containing a half-width wrapped editor
and a half-width text panel. Both children use full height.

The native X11 smoke test resized the actual client window from 640×420 to
440×340 and then 800×500. Content widths were 600, 400 and 760 pixels; each panel
received exactly half, and both followed the row's height. Five reports verify
that resize preserves editor text/selection and that subsequent pointer and
keyboard input inserts text at the expected place. Pixel samples verify both
panel extents. The narrow [screenshot](percent-2.png) was visually inspected.

[Results](results.json), [native log](percent-sizes.log), [metadata](metadata.json),
[source manifest](source-manifest.json) and [source archive](source.tar.gz) preserve
the evidence. All 126 inputs remained unchanged across build and smoke;
metadata records the executable and archive hashes. The test used owned
Xvfb/Openbox processes, Mesa llvmpipe and the private fixed Vulkan loader.

The test exposed a zero-redistribution flex case: when a growing row already
matched its allocation, percentage descendants did not receive a definite height.
A headless component regression and a core-layout test now cover the correction.
The responsive row explicitly enables shrink; `grow()` alone does not change
the existing shrink policy.

Consolidated logs record 405 passing tests, one ignored opt-in GPU stress test,
27 documentation examples, strict Clippy, formatting and macOS ARM64
cross-compilation. A GPU regression compares percentage-resize damage with full
repaint pixels. Native macOS and hardware GPU behavior remain unverified.
