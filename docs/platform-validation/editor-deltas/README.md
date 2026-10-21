# Small history deltas for external editor updates

`TextEditor::set_text` previously selected and replaced the whole document,
retaining the complete old and new strings for each undo entry. Streaming a
small token into a large editor therefore consumed the history byte budget
quickly and repeatedly copied unchanged text.

Replacement now removes a common prefix and suffix and records one UTF-8-safe
changed span. Append/truncate cases use optimized prefix comparisons. Undo/redo
retain exact text and prior selections even when the changed span touches a
combining sequence or ZWJ grapheme. Changed replacements still move the caret to
the document end and cancel preedit once; equal replacements remain untouched.
History entry and per-stack payload limits retain their existing meaning.

The widget model binding now borrows canonical signal text and uses a borrowed
replacement path. It releases the signal borrow before canonical writeback and
refresh callbacks. Accessibility value replacement uses that same path.

Tests cover Unicode shared bytes and grapheme joins, full replacement, insertion,
deletion, repeated text, selection/preedit behavior, history budgets, redo
branches and external model updates with keyboard undo/redo. A 1 MiB document
retains 100 two-byte appends with 200 bytes of edit payload; this excludes history
metadata and allocator overhead. Comparing documents is still linear in their
length, and multiple separated changes retain the intervening span.

The [headless benchmark](../../results/editor-streaming/README.md) compares exact
archived/current TextEditor modules using the same public owned-string API and
locked Unicode dependency. Five fresh-process runs per version measure 1,000
four-byte appends to a 256 KiB baseline with the same 100-entry/4 MiB limits.
Median elapsed time changes from 42.65 to 10.20 ms, live requested allocation
bytes from 4,775,648 to 1,060,788, and verified undo steps from 7 to 100. These
measurements include the model/editor strings and requested Rust allocations;
they are not renderer results or a GPUI/QuickGUI comparison. The borrowed widget
path is implemented and tested but not quantified by that microbenchmark.

Native editor model normalization, read-only clipboard/input and external-model
IME cancellation suites run against the archived integrated build. Linux native
checks use owned Xvfb/Openbox sessions and Mesa llvmpipe. Native macOS and hardware
GPU validation remain outstanding.

The integrated build passes **516 workspace tests** (two opt-in tests ignored),
**29 doctests**, strict Clippy, formatting and the macOS ARM64 cross-check.
All three native suites pass. All 158 source hashes match archive and
workspace after execution. Benchmark source hashes independently match this
implementation and the archived baseline; all ten benchmark correctness reports
pass.
