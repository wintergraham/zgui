# Native inherited letter-spacing validation

The public `letter_spacing` example applies inherited tracking to a monospace
label and editor. Native X11 keys place the editor selection at byte three, then
buttons change tracking through 0, +3, −1 and 0 logical pixels. The text and
selection remain unchanged throughout.

| Tracking | Caret x | Label width |
| --- | ---: | ---: |
| 0 px | 64 | 72 |
| +3 px | 73 | 90 |
| −1 px | 61 | 66 |
| 0 px restored | 64 | 72 |

[Results](results.json), [native log](letter-spacing.log) and
[screenshot at +3 px](letter-spacing-2.png) record the four stages. The screenshot
was visually inspected. [Metadata](metadata.json), [source manifest](source-manifest.json)
and [source archive](source.tar.gz) preserve the build and binary provenance.
All 128 source inputs remained unchanged during build and smoke.
The run used owned Xvfb/Openbox processes, Mesa llvmpipe and the private fixed
Vulkan loader.

GPU tests separately verify matching damaged/full repaint pixels and zero new
glyph bitmap uploads when integer tracking changes. Shaping and software tests
cover pixel-to-em conversion at multiple font sizes, signed tracking, Unicode,
bidi/combining text, hit testing, selections and bounded extremes. Core tests cover
inheritance, slots, editor reflow, cache invalidation and normalized no-op updates.

Consolidated logs record 419 passing tests, one ignored opt-in GPU stress test,
28 documentation examples, strict Clippy, formatting and macOS ARM64
cross-compilation. Native macOS and hardware GPU behavior remain unverified.
See [the styling contract](../../styling.md#letter-spacing) for native glyph
tracking versus approximate fallback cells and the normalized spacing range.
